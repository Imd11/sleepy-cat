use std::sync::atomic::{AtomicBool, Ordering};

#[cfg(target_os = "macos")]
use block2::RcBlock;
#[cfg(target_os = "macos")]
use objc2::rc::Retained;
#[cfg(target_os = "macos")]
use objc2::runtime::AnyObject;
#[cfg(target_os = "macos")]
use objc2_app_kit::{NSEvent, NSEventMask};
use tauri::{Emitter, Manager, WebviewUrl, WebviewWindowBuilder};

pub const BUTTON_WINDOW_LABEL: &str = "prompt-button";
pub const POPOVER_WINDOW_LABEL: &str = "prompt-popover";

pub const BUTTON_WIDTH: f64 = 132.0;
pub const BUTTON_HEIGHT: f64 = 132.0;
pub const BUTTON_WINDOW_TRANSPARENT: bool = true;
pub const POPOVER_WIDTH: f64 = 280.0;
pub const POPOVER_HEIGHT: f64 = 388.0;
pub const POPOVER_GAP: f64 = 4.0;

static OUTSIDE_CLICK_MONITOR_ACTIVE: AtomicBool = AtomicBool::new(false);
#[cfg(target_os = "macos")]
static OUTSIDE_CLICK_MONITOR_INSTALLED: AtomicBool = AtomicBool::new(false);

#[derive(Clone, Copy)]
struct MonitorBounds {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
}

#[derive(Clone, Copy)]
struct WindowRect {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
}

fn point_inside_rect(point: (f64, f64), rect: WindowRect) -> bool {
    point.0 >= rect.x
        && point.0 <= rect.x + rect.width
        && point.1 >= rect.y
        && point.1 <= rect.y + rect.height
}

fn should_dismiss_popover_for_click(
    point: (f64, f64),
    button: Option<WindowRect>,
    popover: Option<WindowRect>,
) -> bool {
    if popover.is_none() {
        return false;
    }
    if button.is_some_and(|rect| point_inside_rect(point, rect)) {
        return false;
    }
    if popover.is_some_and(|rect| point_inside_rect(point, rect)) {
        return false;
    }
    true
}

fn bottom_left_to_top_left_point(point: (f64, f64), bounds: MonitorBounds) -> (f64, f64) {
    (point.0, bounds.y + bounds.height - point.1)
}

fn prompt_button_window_position(x: f64, y: f64) -> tauri::Position {
    tauri::Position::Logical(tauri::LogicalPosition { x, y })
}

fn monitor_bounds(monitor: &tauri::Monitor) -> MonitorBounds {
    let scale = monitor.scale_factor();
    MonitorBounds {
        x: monitor.position().x as f64 / scale,
        y: monitor.position().y as f64 / scale,
        width: monitor.size().width as f64 / scale,
        height: monitor.size().height as f64 / scale,
    }
}

fn clamp_button_position_in_bounds(x: f64, y: f64, bounds: Option<MonitorBounds>) -> (f64, f64) {
    let Some(bounds) = bounds else {
        return (x, y);
    };

    let margin = 8.0;
    let min_x = bounds.x + margin;
    let min_y = bounds.y + margin;
    let max_x = bounds.x + bounds.width - BUTTON_WIDTH - margin;
    let max_y = bounds.y + bounds.height - BUTTON_HEIGHT - margin;

    (x.clamp(min_x, max_x), y.clamp(min_y, max_y))
}

fn clamp_button_position_for_monitor(
    x: f64,
    y: f64,
    monitor: Option<&tauri::Monitor>,
) -> (f64, f64) {
    clamp_button_position_in_bounds(x, y, monitor.map(monitor_bounds))
}

fn positions_are_close(a: (f64, f64), b: (f64, f64)) -> bool {
    (a.0 - b.0).abs() < 0.5 && (a.1 - b.1).abs() < 0.5
}

fn popover_mode_state() -> &'static std::sync::Mutex<Option<String>> {
    static STATE: std::sync::OnceLock<std::sync::Mutex<Option<String>>> =
        std::sync::OnceLock::new();
    STATE.get_or_init(|| std::sync::Mutex::new(None))
}

fn current_popover_mode() -> Option<String> {
    popover_mode_state()
        .lock()
        .expect("popover mode lock poisoned")
        .clone()
}

fn set_popover_mode(mode: Option<&str>) {
    *popover_mode_state()
        .lock()
        .expect("popover mode lock poisoned") = mode.map(str::to_string);
}

fn set_outside_click_monitor_active(active: bool) {
    OUTSIDE_CLICK_MONITOR_ACTIVE.store(active, Ordering::SeqCst);
}

fn should_reuse_popover(existing_mode: Option<&str>, requested_mode: &str) -> bool {
    existing_mode == Some(requested_mode)
}

fn should_close_prompt_popover_on_toggle(existing_mode: Option<&str>, is_visible: bool) -> bool {
    existing_mode == Some("popover") && is_visible
}

fn emit_popover_opened(app: &tauri::AppHandle, mode: &str) {
    let _ = app.emit_to(POPOVER_WINDOW_LABEL, "prompt-popover-opened", mode);
}

fn emit_popover_dismissed(app: &tauri::AppHandle) {
    let _ = app.emit("prompt-popover-dismissed", ());
}

fn window_rect(app: &tauri::AppHandle, label: &str) -> Option<WindowRect> {
    let window = app.get_webview_window(label)?;
    if !window.is_visible().ok()? {
        return None;
    }

    let position = window.outer_position().ok()?;
    let size = window.outer_size().ok()?;
    let scale = window.scale_factor().unwrap_or(1.0);

    Some(WindowRect {
        x: position.x as f64 / scale,
        y: position.y as f64 / scale,
        width: size.width as f64 / scale,
        height: size.height as f64 / scale,
    })
}

#[cfg(target_os = "macos")]
fn current_mouse_point_for_popover(app: &tauri::AppHandle) -> Option<(f64, f64)> {
    let popover = app.get_webview_window(POPOVER_WINDOW_LABEL)?;
    let monitor = popover
        .current_monitor()
        .ok()
        .flatten()
        .or_else(|| app.primary_monitor().ok().flatten())?;
    let bounds = monitor_bounds(&monitor);
    let point = NSEvent::mouseLocation();

    Some(bottom_left_to_top_left_point((point.x, point.y), bounds))
}

#[cfg(target_os = "macos")]
fn handle_prompt_popover_outside_click(app: tauri::AppHandle) {
    if !OUTSIDE_CLICK_MONITOR_ACTIVE.load(Ordering::SeqCst) {
        return;
    }
    if current_popover_mode().as_deref() != Some("popover") {
        set_outside_click_monitor_active(false);
        return;
    }

    let Some(popover_window) = app.get_webview_window(POPOVER_WINDOW_LABEL) else {
        set_outside_click_monitor_active(false);
        return;
    };
    if !popover_window.is_visible().unwrap_or(false) {
        set_outside_click_monitor_active(false);
        return;
    }

    let Some(point) = current_mouse_point_for_popover(&app) else {
        return;
    };
    if should_dismiss_popover_for_click(
        point,
        window_rect(&app, BUTTON_WINDOW_LABEL),
        window_rect(&app, POPOVER_WINDOW_LABEL),
    ) {
        let _ = popover_window.hide();
        set_outside_click_monitor_active(false);
        emit_popover_dismissed(&app);
    }
}

#[cfg(target_os = "macos")]
fn retain_event_monitor(monitor: Retained<AnyObject>) {
    let _ = Retained::into_raw(monitor);
}

#[cfg(target_os = "macos")]
fn install_prompt_popover_outside_click_monitor(app: &tauri::AppHandle) {
    if OUTSIDE_CLICK_MONITOR_INSTALLED
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        return;
    }

    let mask =
        NSEventMask::LeftMouseDown | NSEventMask::RightMouseDown | NSEventMask::OtherMouseDown;

    let global_app = app.clone();
    let global_block = RcBlock::new(move |_event: std::ptr::NonNull<NSEvent>| {
        handle_prompt_popover_outside_click(global_app.clone());
    });
    if let Some(monitor) =
        NSEvent::addGlobalMonitorForEventsMatchingMask_handler(mask, &global_block)
    {
        retain_event_monitor(monitor);
    }

    let local_app = app.clone();
    let local_block = RcBlock::new(move |event: std::ptr::NonNull<NSEvent>| -> *mut NSEvent {
        handle_prompt_popover_outside_click(local_app.clone());
        event.as_ptr()
    });
    if let Some(monitor) =
        unsafe { NSEvent::addLocalMonitorForEventsMatchingMask_handler(mask, &local_block) }
    {
        retain_event_monitor(monitor);
    }
}

#[cfg(not(target_os = "macos"))]
fn install_prompt_popover_outside_click_monitor(_app: &tauri::AppHandle) {}

fn enable_prompt_popover_outside_click_monitor(app: &tauri::AppHandle) {
    install_prompt_popover_outside_click_monitor(app);
    set_outside_click_monitor_active(true);
}

#[derive(serde::Serialize)]
pub struct PromptButtonPosition {
    pub x: f64,
    pub y: f64,
}

#[derive(serde::Serialize)]
pub struct PromptPopoverToggleOutcome {
    pub opened: bool,
}

#[tauri::command]
pub fn prompt_button_position_cmd(
    app: tauri::AppHandle,
) -> Result<Option<PromptButtonPosition>, String> {
    let Some(window) = app.get_webview_window(BUTTON_WINDOW_LABEL) else {
        return Ok(None);
    };
    let position = window.outer_position().map_err(|e| e.to_string())?;
    let scale = window.scale_factor().unwrap_or(1.0);
    Ok(Some(PromptButtonPosition {
        x: position.x as f64 / scale,
        y: position.y as f64 / scale,
    }))
}

#[tauri::command]
pub fn move_prompt_button_to(x: f64, y: f64, app: tauri::AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window(BUTTON_WINDOW_LABEL) {
        let monitor = window
            .current_monitor()
            .map_err(|e| e.to_string())?
            .or(app.primary_monitor().map_err(|e| e.to_string())?);
        let (x, y) = clamp_button_position_for_monitor(x, y, monitor.as_ref());
        window
            .set_position(tauri::Position::Logical(tauri::LogicalPosition { x, y }))
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn show_popover_mode(x: f64, y: f64, mode: &str, app: &tauri::AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window(POPOVER_WINDOW_LABEL) {
        let existing_mode = current_popover_mode();
        if should_reuse_popover(existing_mode.as_deref(), mode) {
            window
                .set_position(prompt_button_window_position(x, y))
                .map_err(|e| e.to_string())?;
            if mode == "popover" {
                crate::macos_panels::configure_transparent_webview_window(&window)?;
                enable_prompt_popover_outside_click_monitor(app);
            } else {
                set_outside_click_monitor_active(false);
            }
            window.show().map_err(|e| e.to_string())?;
            crate::macos_panels::configure_non_activating_panel(&window)?;
            emit_popover_opened(app, mode);
            return Ok(());
        }
        window.close().map_err(|e| e.to_string())?;
        set_popover_mode(None);
    }

    let url = format!("index.html?mode={}", mode);
    let window = WebviewWindowBuilder::new(app, POPOVER_WINDOW_LABEL, WebviewUrl::App(url.into()))
        .title("Prompt Picker")
        .inner_size(POPOVER_WIDTH, POPOVER_HEIGHT)
        .resizable(false)
        .decorations(false)
        .always_on_top(true)
        .accept_first_mouse(true)
        .skip_taskbar(true)
        .position(x, y)
        .build()
        .map_err(|e| e.to_string())?;
    if mode == "popover" {
        crate::macos_panels::configure_transparent_webview_window(&window)?;
        enable_prompt_popover_outside_click_monitor(app);
    } else {
        set_outside_click_monitor_active(false);
    }
    crate::macos_panels::configure_non_activating_panel(&window)?;
    set_popover_mode(Some(mode));
    emit_popover_opened(app, mode);
    Ok(())
}

#[tauri::command]
pub fn show_prompt_button(x: f64, y: f64, app: tauri::AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window(BUTTON_WINDOW_LABEL) {
        let monitor = window
            .current_monitor()
            .map_err(|e| e.to_string())?
            .or(app.primary_monitor().map_err(|e| e.to_string())?);
        let (x, y) = clamp_button_position_for_monitor(x, y, monitor.as_ref());
        let position = window.outer_position().map_err(|e| e.to_string())?;
        let scale = window.scale_factor().unwrap_or(1.0);
        let current = (position.x as f64 / scale, position.y as f64 / scale);
        let visible = window.is_visible().unwrap_or(false);

        if !positions_are_close(current, (x, y)) {
            window
                .set_position(prompt_button_window_position(x, y))
                .map_err(|e| e.to_string())?;
        }
        if !visible {
            window.show().map_err(|e| e.to_string())?;
            if BUTTON_WINDOW_TRANSPARENT {
                crate::macos_panels::configure_transparent_webview_window(&window)?;
            }
            crate::macos_panels::configure_non_activating_panel(&window)?;
        }
        Ok(())
    } else {
        let monitor = app.primary_monitor().map_err(|e| e.to_string())?;
        let (x, y) = clamp_button_position_for_monitor(x, y, monitor.as_ref());
        let window = WebviewWindowBuilder::new(
            &app,
            BUTTON_WINDOW_LABEL,
            WebviewUrl::App("overlay.html".into()),
        )
        .title("Prompt Button")
        .inner_size(BUTTON_WIDTH, BUTTON_HEIGHT)
        .resizable(false)
        .decorations(false)
        .always_on_top(true)
        .accept_first_mouse(true)
        .skip_taskbar(true)
        .position(x, y)
        .build()
        .map_err(|e| e.to_string())?;
        if BUTTON_WINDOW_TRANSPARENT {
            crate::macos_panels::configure_transparent_webview_window(&window)?;
        }
        crate::macos_panels::configure_non_activating_panel(&window)?;
        Ok(())
    }
}

#[tauri::command]
pub fn hide_prompt_button(app: tauri::AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window(BUTTON_WINDOW_LABEL) {
        window.hide().map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
pub fn show_prompt_popover(x: f64, y: f64, app: tauri::AppHandle) -> Result<(), String> {
    show_popover_mode(x, y, "popover", &app)
}

#[tauri::command]
pub fn hide_prompt_popover(app: tauri::AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window(POPOVER_WINDOW_LABEL) {
        window.hide().map_err(|e| e.to_string())?;
        set_outside_click_monitor_active(false);
    }
    Ok(())
}

#[tauri::command]
pub fn show_prompt_popover_from_button(
    session_id: u64,
    session_state: tauri::State<crate::PromptPickSessionState>,
    app: tauri::AppHandle,
) -> Result<(), String> {
    session_state.begin(session_id);
    let position = button_relative_popover_position(&app, BUTTON_WIDTH, BUTTON_HEIGHT);
    show_popover_mode(position.0, position.1, "popover", &app)
}

#[tauri::command]
pub fn toggle_prompt_popover_from_button(
    session_id: u64,
    session_state: tauri::State<crate::PromptPickSessionState>,
    app: tauri::AppHandle,
) -> Result<PromptPopoverToggleOutcome, String> {
    if let Some(window) = app.get_webview_window(POPOVER_WINDOW_LABEL) {
        let visible = window.is_visible().unwrap_or(false);
        if should_close_prompt_popover_on_toggle(current_popover_mode().as_deref(), visible) {
            session_state.begin(session_id);
            window.hide().map_err(|e| e.to_string())?;
            set_outside_click_monitor_active(false);
            emit_popover_dismissed(&app);
            return Ok(PromptPopoverToggleOutcome { opened: false });
        }
    }

    session_state.begin(session_id);
    let position = button_relative_popover_position(&app, BUTTON_WIDTH, BUTTON_HEIGHT);
    show_popover_mode(position.0, position.1, "popover", &app)?;
    Ok(PromptPopoverToggleOutcome { opened: true })
}

#[tauri::command]
pub fn show_prompt_button_controls_from_button(app: tauri::AppHandle) -> Result<(), String> {
    let position = button_relative_popover_position(&app, BUTTON_WIDTH, BUTTON_HEIGHT);
    show_popover_mode(position.0, position.1, "button-controls", &app)
}

fn button_relative_popover_position(
    app: &tauri::AppHandle,
    button_width: f64,
    button_height: f64,
) -> (f64, f64) {
    app.get_webview_window(BUTTON_WINDOW_LABEL)
        .and_then(|window| {
            let position = window.outer_position().ok()?;
            let scale = window.scale_factor().unwrap_or(1.0);
            let button_x = position.x as f64 / scale;
            let button_y = position.y as f64 / scale;
            let monitor = window.current_monitor().ok().flatten();
            Some(clamp_popover_position(
                button_x,
                button_y,
                button_width,
                button_height,
                monitor.as_ref(),
            ))
        })
        .unwrap_or((100.0, 100.0))
}

fn clamp_popover_position(
    button_x: f64,
    button_y: f64,
    button_width: f64,
    button_height: f64,
    monitor: Option<&tauri::Monitor>,
) -> (f64, f64) {
    let Some(monitor) = monitor else {
        return clamp_popover_position_in_bounds(
            button_x,
            button_y,
            button_width,
            button_height,
            None,
        );
    };

    let scale = monitor.scale_factor();
    let bounds = MonitorBounds {
        x: monitor.position().x as f64 / scale,
        y: monitor.position().y as f64 / scale,
        width: monitor.size().width as f64 / scale,
        height: monitor.size().height as f64 / scale,
    };

    clamp_popover_position_in_bounds(
        button_x,
        button_y,
        button_width,
        button_height,
        Some(bounds),
    )
}

fn clamp_popover_position_in_bounds(
    button_x: f64,
    button_y: f64,
    button_width: f64,
    button_height: f64,
    bounds: Option<MonitorBounds>,
) -> (f64, f64) {
    let centered_x = button_x + (button_width / 2.0) - (POPOVER_WIDTH / 2.0);
    let above_y = button_y - POPOVER_HEIGHT - POPOVER_GAP;
    let below_y = button_y + button_height + POPOVER_GAP;

    let Some(bounds) = bounds else {
        return (centered_x, above_y);
    };

    let margin = 8.0;
    let monitor_right = bounds.x + bounds.width;
    let monitor_bottom = bounds.y + bounds.height;
    let min_x = bounds.x + margin;
    let max_x = monitor_right - POPOVER_WIDTH - margin;
    let min_y = bounds.y + margin;
    let max_y = monitor_bottom - POPOVER_HEIGHT - margin;

    let x = centered_x.clamp(min_x, max_x);
    let y = if above_y >= min_y {
        above_y
    } else {
        below_y.clamp(min_y, max_y)
    };

    (x, y)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn places_popover_above_centered_on_calico_when_there_is_room() {
        let bounds = MonitorBounds {
            x: 0.0,
            y: 0.0,
            width: 1440.0,
            height: 900.0,
        };
        let position = clamp_popover_position_in_bounds(
            500.0,
            400.0,
            BUTTON_WIDTH,
            BUTTON_HEIGHT,
            Some(bounds),
        );

        let expected_x = 500.0 + (BUTTON_WIDTH / 2.0) - (POPOVER_WIDTH / 2.0);
        let expected_y = 400.0 - POPOVER_HEIGHT - POPOVER_GAP;
        assert_eq!(position, (expected_x, expected_y));
    }

    #[test]
    fn prompt_popover_height_supports_taller_prompt_list() {
        assert_eq!(POPOVER_HEIGHT, 388.0);
    }

    #[test]
    fn prompt_popover_gap_keeps_list_close_to_calico() {
        assert_eq!(POPOVER_GAP, 4.0);
    }

    #[test]
    fn places_popover_below_calico_when_top_has_no_room() {
        let bounds = MonitorBounds {
            x: 0.0,
            y: 0.0,
            width: 1440.0,
            height: 900.0,
        };
        let position = clamp_popover_position_in_bounds(
            500.0,
            20.0,
            BUTTON_WIDTH,
            BUTTON_HEIGHT,
            Some(bounds),
        );

        assert_eq!(position.1, 20.0 + BUTTON_HEIGHT + POPOVER_GAP);
    }

    #[test]
    fn clamps_popover_horizontally_inside_monitor() {
        let bounds = MonitorBounds {
            x: 0.0,
            y: 0.0,
            width: 1440.0,
            height: 900.0,
        };
        let left =
            clamp_popover_position_in_bounds(4.0, 400.0, BUTTON_WIDTH, BUTTON_HEIGHT, Some(bounds));
        let right = clamp_popover_position_in_bounds(
            1390.0,
            400.0,
            BUTTON_WIDTH,
            BUTTON_HEIGHT,
            Some(bounds),
        );

        assert_eq!(left.0, 8.0);
        assert_eq!(right.0, 1440.0 - POPOVER_WIDTH - 8.0);
    }

    #[test]
    fn calico_button_window_uses_square_character_size() {
        assert_eq!(BUTTON_WIDTH, 132.0);
        assert_eq!(BUTTON_HEIGHT, 132.0);
    }

    #[test]
    fn calico_button_window_uses_native_transparency() {
        assert!(BUTTON_WINDOW_TRANSPARENT);
    }

    #[test]
    fn clamps_calico_button_inside_monitor() {
        let bounds = MonitorBounds {
            x: 0.0,
            y: 0.0,
            width: 1440.0,
            height: 900.0,
        };

        let position = clamp_button_position_in_bounds(2000.0, -100.0, Some(bounds));

        assert_eq!(position, (1440.0 - BUTTON_WIDTH - 8.0, 8.0));
    }

    #[test]
    fn prompt_button_set_position_uses_logical_coordinates() {
        let position = prompt_button_window_position(320.0, 240.0);

        match position {
            tauri::Position::Logical(logical) => {
                assert_eq!(logical.x, 320.0);
                assert_eq!(logical.y, 240.0);
            }
            _ => panic!("prompt button position must use logical coordinates"),
        }
    }

    #[test]
    fn outside_click_dismisses_when_point_is_outside_button_and_popover() {
        let button = WindowRect {
            x: 100.0,
            y: 500.0,
            width: BUTTON_WIDTH,
            height: BUTTON_HEIGHT,
        };
        let popover = WindowRect {
            x: 20.0,
            y: 108.0,
            width: POPOVER_WIDTH,
            height: POPOVER_HEIGHT,
        };

        assert!(should_dismiss_popover_for_click(
            (900.0, 900.0),
            Some(button),
            Some(popover)
        ));
    }

    #[test]
    fn outside_click_keeps_popover_when_point_is_inside_popover_or_button() {
        let button = WindowRect {
            x: 100.0,
            y: 500.0,
            width: BUTTON_WIDTH,
            height: BUTTON_HEIGHT,
        };
        let popover = WindowRect {
            x: 20.0,
            y: 108.0,
            width: POPOVER_WIDTH,
            height: POPOVER_HEIGHT,
        };

        assert!(!should_dismiss_popover_for_click(
            (40.0, 120.0),
            Some(button),
            Some(popover)
        ));
        assert!(!should_dismiss_popover_for_click(
            (120.0, 520.0),
            Some(button),
            Some(popover)
        ));
    }

    #[test]
    fn outside_click_does_nothing_without_visible_popover_rect() {
        let button = WindowRect {
            x: 100.0,
            y: 500.0,
            width: BUTTON_WIDTH,
            height: BUTTON_HEIGHT,
        };

        assert!(!should_dismiss_popover_for_click(
            (900.0, 900.0),
            Some(button),
            None
        ));
    }

    #[test]
    fn converts_bottom_left_screen_point_to_top_left_logical_point() {
        let bounds = MonitorBounds {
            x: 0.0,
            y: 0.0,
            width: 1440.0,
            height: 900.0,
        };

        assert_eq!(
            bottom_left_to_top_left_point((100.0, 200.0), bounds),
            (100.0, 700.0)
        );
    }

    #[test]
    fn prompt_list_mode_enables_outside_click_monitor() {
        let source = include_str!("windows.rs");
        let start = source
            .find("fn show_popover_mode")
            .expect("popover mode function should exist");
        let end = source[start..]
            .find("#[tauri::command]")
            .expect("first popover command should follow show_popover_mode");
        let show_source = &source[start..start + end];

        assert!(show_source.contains("if mode == \"popover\""));
        assert!(show_source.contains("enable_prompt_popover_outside_click_monitor(app);"));
        assert!(show_source.contains("set_outside_click_monitor_active(false);"));
    }

    #[test]
    fn outside_click_monitor_uses_global_and_local_mouse_down_events() {
        let source = include_str!("windows.rs");
        assert!(source.contains("addGlobalMonitorForEventsMatchingMask_handler"));
        assert!(source.contains("addLocalMonitorForEventsMatchingMask_handler"));
        assert!(source.contains("NSEventMask::LeftMouseDown"));
        assert!(source.contains("NSEventMask::RightMouseDown"));
        assert!(source.contains("NSEventMask::OtherMouseDown"));
    }

    #[test]
    fn reuses_popover_only_for_the_same_mode() {
        assert!(should_reuse_popover(Some("popover"), "popover"));
        assert!(should_reuse_popover(
            Some("button-controls"),
            "button-controls"
        ));
        assert!(!should_reuse_popover(Some("button-controls"), "popover"));
        assert!(!should_reuse_popover(None, "popover"));
    }

    #[test]
    fn prompt_popover_toggle_closes_only_visible_prompt_lists() {
        assert!(should_close_prompt_popover_on_toggle(Some("popover"), true));
        assert!(!should_close_prompt_popover_on_toggle(
            Some("popover"),
            false
        ));
        assert!(!should_close_prompt_popover_on_toggle(
            Some("button-controls"),
            true
        ));
        assert!(!should_close_prompt_popover_on_toggle(None, true));
    }

    #[test]
    fn prompt_popover_toggle_invalidates_session_and_emits_dismissal_when_closing() {
        let source = include_str!("windows.rs");
        let start = source
            .find("pub fn toggle_prompt_popover_from_button")
            .expect("toggle command should exist");
        let end = source[start..]
            .find("pub fn show_prompt_button_controls_from_button")
            .expect("button controls command should follow toggle command");
        let command_source = &source[start..start + end];

        assert!(command_source.contains("should_close_prompt_popover_on_toggle"));
        assert!(command_source.contains("session_state.begin(session_id);"));
        assert!(command_source.contains("window.hide().map_err"));
        assert!(command_source.contains("emit_popover_dismissed(&app)"));
        assert!(command_source.contains("PromptPopoverToggleOutcome { opened: false }"));
        assert!(command_source.contains("PromptPopoverToggleOutcome { opened: true }"));
    }

    #[test]
    fn reused_popover_is_repositioned_shown_and_announced() {
        let source = include_str!("windows.rs");
        let start = source
            .find("fn show_popover_mode")
            .expect("show_popover_mode should exist");
        let end = source[start..]
            .find("let url = format!")
            .expect("show_popover_mode should build a fresh popover after reuse branch");
        let reuse_source = &source[start..start + end];

        assert!(reuse_source.contains("should_reuse_popover("));
        assert!(reuse_source.contains("set_position(prompt_button_window_position(x, y))"));
        assert!(reuse_source.contains("window.show().map_err"));
        assert!(reuse_source.contains("emit_popover_opened(app, mode)"));
    }

    #[test]
    fn prompt_list_popover_uses_transparent_native_window() {
        let source = include_str!("windows.rs");
        let start = source
            .find("fn show_popover_mode")
            .expect("show_popover_mode should exist");
        let end = source[start..]
            .find("#[tauri::command]\npub fn show_prompt_button")
            .expect("show_prompt_button should follow show_popover_mode");
        let command_source = &source[start..start + end];

        assert!(command_source.contains("if mode == \"popover\""));
        assert!(command_source.contains("configure_transparent_webview_window(&window)?"));
        assert!(command_source.contains("configure_non_activating_panel(&window)?"));
    }
}
