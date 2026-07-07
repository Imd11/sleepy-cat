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

pub const BUTTON_VISUAL_WIDTH: f64 = 132.0;
pub const BUTTON_VISUAL_HEIGHT: f64 = 132.0;
pub const BUTTON_WINDOW_WIDTH: f64 = 288.0;
pub const BUTTON_WINDOW_HEIGHT: f64 = 288.0;
pub const BUTTON_WINDOW_PADDING_X: f64 = (BUTTON_WINDOW_WIDTH - BUTTON_VISUAL_WIDTH) / 2.0;
pub const BUTTON_WINDOW_PADDING_Y: f64 = (BUTTON_WINDOW_HEIGHT - BUTTON_VISUAL_HEIGHT) / 2.0;
pub const BUTTON_WINDOW_TRANSPARENT: bool = true;
pub const POPOVER_WIDTH: f64 = 280.0;
pub const POPOVER_HEIGHT: f64 = 432.0;
pub const POPOVER_WINDOW_PADDING: f64 = 0.0;
pub const POPOVER_WINDOW_WIDTH: f64 = POPOVER_WIDTH;
pub const POPOVER_WINDOW_HEIGHT: f64 = POPOVER_HEIGHT;
pub const BUTTON_CONTROLS_WIDTH: f64 = 156.0;
pub const BUTTON_CONTROLS_HEIGHT: f64 = 72.0;
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

#[derive(Clone, Copy)]
struct PopoverSize {
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

fn logical_position(x: f64, y: f64) -> tauri::Position {
    tauri::Position::Logical(tauri::LogicalPosition { x, y })
}

fn prompt_button_visual_to_window_position(x: f64, y: f64) -> (f64, f64) {
    (x - BUTTON_WINDOW_PADDING_X, y - BUTTON_WINDOW_PADDING_Y)
}

fn prompt_button_window_to_visual_position(x: f64, y: f64) -> (f64, f64) {
    (x + BUTTON_WINDOW_PADDING_X, y + BUTTON_WINDOW_PADDING_Y)
}

fn prompt_button_position_from_visual(x: f64, y: f64) -> tauri::Position {
    let (window_x, window_y) = prompt_button_visual_to_window_position(x, y);
    logical_position(window_x, window_y)
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
    let max_x = bounds.x + bounds.width - BUTTON_VISUAL_WIDTH - margin;
    let max_y = bounds.y + bounds.height - BUTTON_VISUAL_HEIGHT - margin;

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

fn popover_size_for_mode(mode: &str) -> PopoverSize {
    if mode == "button-controls" {
        return PopoverSize {
            width: BUTTON_CONTROLS_WIDTH,
            height: BUTTON_CONTROLS_HEIGHT,
        };
    }

    PopoverSize {
        width: POPOVER_WIDTH,
        height: POPOVER_HEIGHT,
    }
}

fn popover_window_padding_for_mode(mode: &str) -> f64 {
    if mode == "popover" {
        POPOVER_WINDOW_PADDING
    } else {
        0.0
    }
}

fn popover_window_size_for_mode(mode: &str) -> PopoverSize {
    if mode == "popover" {
        return PopoverSize {
            width: POPOVER_WINDOW_WIDTH,
            height: POPOVER_WINDOW_HEIGHT,
        };
    }

    popover_size_for_mode(mode)
}

fn popover_window_position_from_visual_position(x: f64, y: f64, mode: &str) -> (f64, f64) {
    let padding = popover_window_padding_for_mode(mode);
    (x - padding, y - padding)
}

fn visual_popover_rect_from_window_rect(rect: WindowRect, mode: &str) -> WindowRect {
    let padding = popover_window_padding_for_mode(mode);
    WindowRect {
        x: rect.x + padding,
        y: rect.y + padding,
        width: rect.width - (padding * 2.0),
        height: rect.height - (padding * 2.0),
    }
}

fn should_use_transparent_popover_window(mode: Option<&str>) -> bool {
    matches!(mode, Some("popover" | "button-controls"))
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

fn visual_button_rect_from_window_rect(rect: WindowRect) -> WindowRect {
    let (x, y) = prompt_button_window_to_visual_position(rect.x, rect.y);
    WindowRect {
        x,
        y,
        width: BUTTON_VISUAL_WIDTH,
        height: BUTTON_VISUAL_HEIGHT,
    }
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
    let current_mode = current_popover_mode();
    if !should_use_transparent_popover_window(current_mode.as_deref()) {
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
    let button = window_rect(&app, BUTTON_WINDOW_LABEL).map(visual_button_rect_from_window_rect);
    let popover = window_rect(&app, POPOVER_WINDOW_LABEL).map(|rect| {
        visual_popover_rect_from_window_rect(rect, current_mode.as_deref().unwrap_or_default())
    });
    if should_dismiss_popover_for_click(point, button, popover) {
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
    let (x, y) = prompt_button_window_to_visual_position(
        position.x as f64 / scale,
        position.y as f64 / scale,
    );
    Ok(Some(PromptButtonPosition { x, y }))
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
            .set_position(prompt_button_position_from_visual(x, y))
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn show_popover_mode(x: f64, y: f64, mode: &str, app: &tauri::AppHandle) -> Result<(), String> {
    let popover_size = popover_window_size_for_mode(mode);
    let (window_x, window_y) = popover_window_position_from_visual_position(x, y, mode);
    if let Some(window) = app.get_webview_window(POPOVER_WINDOW_LABEL) {
        let existing_mode = current_popover_mode();
        if should_reuse_popover(existing_mode.as_deref(), mode) {
            window
                .set_position(logical_position(window_x, window_y))
                .map_err(|e| e.to_string())?;
            window
                .set_size(tauri::Size::Logical(tauri::LogicalSize {
                    width: popover_size.width,
                    height: popover_size.height,
                }))
                .map_err(|e| e.to_string())?;
            if should_use_transparent_popover_window(Some(mode)) {
                crate::macos_panels::configure_transparent_webview_window(&window)?;
                enable_prompt_popover_outside_click_monitor(app);
            } else {
                set_outside_click_monitor_active(false);
            }
            show_non_activating_overlay_window(&window)?;
            emit_popover_opened(app, mode);
            return Ok(());
        }
        window.close().map_err(|e| e.to_string())?;
        set_popover_mode(None);
    }

    let url = format!("index.html?mode={}", mode);
    let window = WebviewWindowBuilder::new(app, POPOVER_WINDOW_LABEL, WebviewUrl::App(url.into()))
        .title("Prompt Picker")
        .inner_size(popover_size.width, popover_size.height)
        .resizable(false)
        .decorations(false)
        .always_on_top(true)
        .accept_first_mouse(true)
        .skip_taskbar(true)
        .focusable(false)
        .visible(false)
        .position(window_x, window_y)
        .build()
        .map_err(|e| e.to_string())?;
    if should_use_transparent_popover_window(Some(mode)) {
        crate::macos_panels::configure_transparent_webview_window(&window)?;
        enable_prompt_popover_outside_click_monitor(app);
    } else {
        set_outside_click_monitor_active(false);
    }
    show_non_activating_overlay_window(&window)?;
    set_popover_mode(Some(mode));
    emit_popover_opened(app, mode);
    Ok(())
}

fn show_non_activating_overlay_window(window: &tauri::WebviewWindow) -> Result<(), String> {
    window.set_focusable(false).map_err(|e| e.to_string())?;
    crate::macos_panels::configure_non_activating_panel(window)?;

    if !window.is_visible().unwrap_or(false) {
        window.show().map_err(|e| e.to_string())?;
        window.set_focusable(false).map_err(|e| e.to_string())?;
        crate::macos_panels::configure_non_activating_panel(window)?;
    }

    if !window.is_visible().unwrap_or(false) {
        return Err("Overlay window did not become visible.".to_string());
    }

    Ok(())
}

fn build_prompt_button_window(
    app: &tauri::AppHandle,
    x: f64,
    y: f64,
) -> Result<tauri::WebviewWindow, String> {
    let monitor = app.primary_monitor().map_err(|e| e.to_string())?;
    let (x, y) = clamp_button_position_for_monitor(x, y, monitor.as_ref());
    let (window_x, window_y) = prompt_button_visual_to_window_position(x, y);
    let window = WebviewWindowBuilder::new(
        app,
        BUTTON_WINDOW_LABEL,
        WebviewUrl::App("overlay.html".into()),
    )
    .title("Prompt Button")
    .inner_size(BUTTON_WINDOW_WIDTH, BUTTON_WINDOW_HEIGHT)
    .resizable(false)
    .decorations(false)
    .always_on_top(true)
    .accept_first_mouse(true)
    .skip_taskbar(true)
    .focusable(false)
    .visible(false)
    .position(window_x, window_y)
    .build()
    .map_err(|e| e.to_string())?;

    if BUTTON_WINDOW_TRANSPARENT {
        crate::macos_panels::configure_transparent_webview_window(&window)?;
    }
    show_non_activating_overlay_window(&window)?;
    Ok(window)
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
        let current = prompt_button_window_to_visual_position(
            position.x as f64 / scale,
            position.y as f64 / scale,
        );
        let visible = window.is_visible().unwrap_or(false);

        window
            .set_size(tauri::Size::Logical(tauri::LogicalSize {
                width: BUTTON_WINDOW_WIDTH,
                height: BUTTON_WINDOW_HEIGHT,
            }))
            .map_err(|e| e.to_string())?;
        if !positions_are_close(current, (x, y)) {
            window
                .set_position(prompt_button_position_from_visual(x, y))
                .map_err(|e| e.to_string())?;
        }
        if !visible {
            if BUTTON_WINDOW_TRANSPARENT {
                crate::macos_panels::configure_transparent_webview_window(&window)?;
            }
            show_non_activating_overlay_window(&window)?;
        }
        Ok(())
    } else {
        build_prompt_button_window(&app, x, y)?;
        Ok(())
    }
}

pub fn rebuild_prompt_button_window(app: &tauri::AppHandle) -> Result<(), String> {
    let position = prompt_button_position_cmd(app.clone())?
        .map(|point| (point.x, point.y))
        .unwrap_or((960.0, 700.0));

    if let Some(window) = app.get_webview_window(BUTTON_WINDOW_LABEL) {
        let _ = window.close();
    }

    build_prompt_button_window(app, position.0, position.1)?;
    Ok(())
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
        let was_visible = window.is_visible().unwrap_or(false);
        window.hide().map_err(|e| e.to_string())?;
        set_outside_click_monitor_active(false);
        if was_visible {
            emit_popover_dismissed(&app);
        }
    }
    Ok(())
}

#[tauri::command]
pub fn show_prompt_popover_from_button(
    session_id: u64,
    session_state: tauri::State<crate::PromptPickSessionState>,
    app: tauri::AppHandle,
) -> Result<(), String> {
    session_state.begin_if_new(session_id);
    let position = button_relative_popover_position(
        &app,
        BUTTON_VISUAL_WIDTH,
        BUTTON_VISUAL_HEIGHT,
        "popover",
    );
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

    session_state.begin_if_new(session_id);
    let position = button_relative_popover_position(
        &app,
        BUTTON_VISUAL_WIDTH,
        BUTTON_VISUAL_HEIGHT,
        "popover",
    );
    show_popover_mode(position.0, position.1, "popover", &app)?;
    Ok(PromptPopoverToggleOutcome { opened: true })
}

#[tauri::command]
pub fn show_prompt_button_controls_from_button(app: tauri::AppHandle) -> Result<(), String> {
    let position = button_relative_popover_position(
        &app,
        BUTTON_VISUAL_WIDTH,
        BUTTON_VISUAL_HEIGHT,
        "button-controls",
    );
    show_popover_mode(position.0, position.1, "button-controls", &app)
}

fn button_relative_popover_position(
    app: &tauri::AppHandle,
    button_width: f64,
    button_height: f64,
    mode: &str,
) -> (f64, f64) {
    let popover_size = popover_size_for_mode(mode);
    let window_padding = popover_window_padding_for_mode(mode);
    app.get_webview_window(BUTTON_WINDOW_LABEL)
        .and_then(|window| {
            let position = window.outer_position().ok()?;
            let scale = window.scale_factor().unwrap_or(1.0);
            let native_x = position.x as f64 / scale;
            let native_y = position.y as f64 / scale;
            let (button_x, button_y) = prompt_button_window_to_visual_position(native_x, native_y);
            let monitor = window.current_monitor().ok().flatten();
            Some(clamp_popover_position_for_size(
                button_x,
                button_y,
                button_width,
                button_height,
                popover_size,
                window_padding,
                monitor.as_ref(),
            ))
        })
        .unwrap_or((100.0, 100.0))
}

fn clamp_popover_position_for_size(
    button_x: f64,
    button_y: f64,
    button_width: f64,
    button_height: f64,
    popover_size: PopoverSize,
    window_padding: f64,
    monitor: Option<&tauri::Monitor>,
) -> (f64, f64) {
    let Some(monitor) = monitor else {
        return clamp_popover_position_in_bounds_for_size(
            button_x,
            button_y,
            button_width,
            button_height,
            popover_size,
            window_padding,
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

    clamp_popover_position_in_bounds_for_size(
        button_x,
        button_y,
        button_width,
        button_height,
        popover_size,
        window_padding,
        Some(bounds),
    )
}

#[cfg(test)]
fn clamp_popover_position_in_bounds(
    button_x: f64,
    button_y: f64,
    button_width: f64,
    button_height: f64,
    bounds: Option<MonitorBounds>,
) -> (f64, f64) {
    clamp_popover_position_in_bounds_for_size(
        button_x,
        button_y,
        button_width,
        button_height,
        popover_size_for_mode("popover"),
        popover_window_padding_for_mode("popover"),
        bounds,
    )
}

fn clamp_popover_position_in_bounds_for_size(
    button_x: f64,
    button_y: f64,
    button_width: f64,
    button_height: f64,
    popover_size: PopoverSize,
    window_padding: f64,
    bounds: Option<MonitorBounds>,
) -> (f64, f64) {
    let centered_x = button_x + (button_width / 2.0) - (popover_size.width / 2.0);
    let above_y = button_y - popover_size.height - POPOVER_GAP;
    let below_y = button_y + button_height + POPOVER_GAP;

    let Some(bounds) = bounds else {
        return (centered_x, above_y);
    };

    let margin = 8.0;
    let monitor_right = bounds.x + bounds.width;
    let monitor_bottom = bounds.y + bounds.height;
    let min_x = bounds.x + margin + window_padding;
    let max_x = monitor_right - popover_size.width - margin - window_padding;
    let min_y = bounds.y + margin + window_padding;
    let max_y = monitor_bottom - popover_size.height - margin - window_padding;

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
            600.0,
            BUTTON_VISUAL_WIDTH,
            BUTTON_VISUAL_HEIGHT,
            Some(bounds),
        );

        let expected_x = 500.0 + (BUTTON_VISUAL_WIDTH / 2.0) - (POPOVER_WIDTH / 2.0);
        let expected_y = 600.0 - POPOVER_HEIGHT - POPOVER_GAP;
        assert_eq!(position, (expected_x, expected_y));
    }

    #[test]
    fn prompt_popover_height_supports_category_tabs_and_prompt_list() {
        assert_eq!(POPOVER_HEIGHT, 432.0);
    }

    #[test]
    fn prompt_popover_native_window_matches_rounded_panel_size() {
        let visual_size = popover_size_for_mode("popover");
        let window_size = popover_window_size_for_mode("popover");

        assert_eq!(POPOVER_WINDOW_PADDING, 0.0);
        assert_eq!(visual_size.width, POPOVER_WIDTH);
        assert_eq!(visual_size.height, POPOVER_HEIGHT);
        assert_eq!(window_size.width, POPOVER_WINDOW_WIDTH);
        assert_eq!(window_size.height, POPOVER_WINDOW_HEIGHT);
        assert_eq!(window_size.width, POPOVER_WIDTH);
        assert_eq!(window_size.height, POPOVER_HEIGHT);
    }

    #[test]
    fn prompt_popover_visual_rect_matches_native_window_rect() {
        let rect = WindowRect {
            x: 120.0,
            y: 240.0,
            width: POPOVER_WINDOW_WIDTH,
            height: POPOVER_WINDOW_HEIGHT,
        };
        let visual = visual_popover_rect_from_window_rect(rect, "popover");

        assert_eq!(visual.x, rect.x);
        assert_eq!(visual.y, rect.y);
        assert_eq!(visual.width, rect.width);
        assert_eq!(visual.height, rect.height);
    }

    #[test]
    fn button_controls_uses_compact_popover_size() {
        let size = popover_size_for_mode("button-controls");
        let window_size = popover_window_size_for_mode("button-controls");

        assert_eq!(size.width, BUTTON_CONTROLS_WIDTH);
        assert_eq!(size.height, BUTTON_CONTROLS_HEIGHT);
        assert_eq!(window_size.width, BUTTON_CONTROLS_WIDTH);
        assert_eq!(window_size.height, BUTTON_CONTROLS_HEIGHT);
        assert_eq!(BUTTON_CONTROLS_WIDTH, 156.0);
        assert_eq!(BUTTON_CONTROLS_HEIGHT, 72.0);
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
            BUTTON_VISUAL_WIDTH,
            BUTTON_VISUAL_HEIGHT,
            Some(bounds),
        );

        assert_eq!(position.1, 20.0 + BUTTON_VISUAL_HEIGHT + POPOVER_GAP);
    }

    #[test]
    fn taller_popover_stays_inside_monitor_near_bottom_edge() {
        let bounds = MonitorBounds {
            x: 0.0,
            y: 0.0,
            width: 1440.0,
            height: 900.0,
        };
        let position = clamp_popover_position_in_bounds(
            500.0,
            760.0,
            BUTTON_VISUAL_WIDTH,
            BUTTON_VISUAL_HEIGHT,
            Some(bounds),
        );

        assert!(position.1 >= 8.0);
        assert!(position.1 + POPOVER_HEIGHT <= 892.0);
    }

    #[test]
    fn clamps_popover_horizontally_inside_monitor() {
        let bounds = MonitorBounds {
            x: 0.0,
            y: 0.0,
            width: 1440.0,
            height: 900.0,
        };
        let left = clamp_popover_position_in_bounds(
            4.0,
            400.0,
            BUTTON_VISUAL_WIDTH,
            BUTTON_VISUAL_HEIGHT,
            Some(bounds),
        );
        let right = clamp_popover_position_in_bounds(
            1390.0,
            400.0,
            BUTTON_VISUAL_WIDTH,
            BUTTON_VISUAL_HEIGHT,
            Some(bounds),
        );

        assert_eq!(left.0, 8.0);
        assert_eq!(right.0, 1440.0 - POPOVER_WIDTH - 8.0);
    }

    #[test]
    fn button_controls_position_uses_compact_size() {
        let bounds = MonitorBounds {
            x: 0.0,
            y: 0.0,
            width: 1440.0,
            height: 900.0,
        };
        let position = clamp_popover_position_in_bounds_for_size(
            500.0,
            400.0,
            BUTTON_VISUAL_WIDTH,
            BUTTON_VISUAL_HEIGHT,
            popover_size_for_mode("button-controls"),
            popover_window_padding_for_mode("button-controls"),
            Some(bounds),
        );

        assert_eq!(
            position,
            (
                500.0 + (BUTTON_VISUAL_WIDTH / 2.0) - (BUTTON_CONTROLS_WIDTH / 2.0),
                400.0 - BUTTON_CONTROLS_HEIGHT - POPOVER_GAP
            )
        );
    }

    #[test]
    fn calico_button_window_uses_square_character_size() {
        assert_eq!(BUTTON_VISUAL_WIDTH, 132.0);
        assert_eq!(BUTTON_VISUAL_HEIGHT, 132.0);
    }

    #[test]
    fn calico_button_window_has_animation_padding() {
        assert_eq!(BUTTON_VISUAL_WIDTH, 132.0);
        assert_eq!(BUTTON_VISUAL_HEIGHT, 132.0);
        assert_eq!(BUTTON_WINDOW_WIDTH, 288.0);
        assert_eq!(BUTTON_WINDOW_HEIGHT, 288.0);
        assert_eq!(BUTTON_WINDOW_PADDING_X, 78.0);
        assert_eq!(BUTTON_WINDOW_PADDING_Y, 78.0);
    }

    #[test]
    fn calico_button_window_uses_larger_native_transparent_size() {
        assert!(BUTTON_WINDOW_WIDTH > BUTTON_VISUAL_WIDTH);
        assert!(BUTTON_WINDOW_HEIGHT > BUTTON_VISUAL_HEIGHT);
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

        assert_eq!(position, (1440.0 - BUTTON_VISUAL_WIDTH - 8.0, 8.0));
    }

    #[test]
    fn calico_visual_and_window_positions_round_trip() {
        let visual = (320.0, 240.0);
        let window = prompt_button_visual_to_window_position(visual.0, visual.1);

        assert_eq!(window, (242.0, 162.0));
        assert_eq!(
            prompt_button_window_to_visual_position(window.0, window.1),
            visual
        );
    }

    #[test]
    fn calico_button_position_from_visual_subtracts_padding_without_affecting_generic_windows() {
        let button_position = prompt_button_position_from_visual(320.0, 240.0);
        let generic_position = logical_position(320.0, 240.0);

        match button_position {
            tauri::Position::Logical(logical) => {
                assert_eq!(logical.x, 242.0);
                assert_eq!(logical.y, 162.0);
            }
            _ => panic!("prompt button position must use logical coordinates"),
        }

        match generic_position {
            tauri::Position::Logical(logical) => {
                assert_eq!(logical.x, 320.0);
                assert_eq!(logical.y, 240.0);
            }
            _ => panic!("generic position must use logical coordinates"),
        }
    }

    #[test]
    fn outside_click_uses_visual_button_rect_not_transparent_window_padding() {
        let native_button = WindowRect {
            x: 242.0,
            y: 162.0,
            width: BUTTON_WINDOW_WIDTH,
            height: BUTTON_WINDOW_HEIGHT,
        };
        let visual_button = visual_button_rect_from_window_rect(native_button);
        let popover = WindowRect {
            x: 800.0,
            y: 20.0,
            width: POPOVER_WIDTH,
            height: POPOVER_HEIGHT,
        };

        assert_eq!(visual_button.x, 320.0);
        assert_eq!(visual_button.y, 240.0);
        assert_eq!(visual_button.width, BUTTON_VISUAL_WIDTH);
        assert_eq!(visual_button.height, BUTTON_VISUAL_HEIGHT);
        assert!(should_dismiss_popover_for_click(
            (300.0, 220.0),
            Some(visual_button),
            Some(popover)
        ));
        assert!(!should_dismiss_popover_for_click(
            (340.0, 260.0),
            Some(visual_button),
            Some(popover)
        ));
    }

    #[test]
    fn outside_click_uses_popover_panel_rect_without_shadow_padding() {
        let native_popover = WindowRect {
            x: 800.0,
            y: 20.0,
            width: POPOVER_WINDOW_WIDTH,
            height: POPOVER_WINDOW_HEIGHT,
        };
        let visual_popover = visual_popover_rect_from_window_rect(native_popover, "popover");

        assert_eq!(visual_popover.x, native_popover.x);
        assert_eq!(visual_popover.y, native_popover.y);
        assert_eq!(visual_popover.width, native_popover.width);
        assert_eq!(visual_popover.height, native_popover.height);
        assert!(should_dismiss_popover_for_click(
            (790.0, 12.0),
            None,
            Some(visual_popover)
        ));
        assert!(!should_dismiss_popover_for_click(
            (810.0, 30.0),
            None,
            Some(visual_popover)
        ));
    }

    #[test]
    fn outside_click_dismisses_when_point_is_outside_button_and_popover() {
        let button = WindowRect {
            x: 100.0,
            y: 500.0,
            width: BUTTON_VISUAL_WIDTH,
            height: BUTTON_VISUAL_HEIGHT,
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
            width: BUTTON_VISUAL_WIDTH,
            height: BUTTON_VISUAL_HEIGHT,
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
            width: BUTTON_VISUAL_WIDTH,
            height: BUTTON_VISUAL_HEIGHT,
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
    fn transparent_popover_modes_enable_outside_click_monitor() {
        let source = include_str!("windows.rs");
        let start = source
            .find("fn show_popover_mode")
            .expect("popover mode function should exist");
        let end = source[start..]
            .find("#[tauri::command]")
            .expect("first popover command should follow show_popover_mode");
        let show_source = &source[start..start + end];

        assert!(should_use_transparent_popover_window(Some("popover")));
        assert!(should_use_transparent_popover_window(Some(
            "button-controls"
        )));
        assert!(show_source.contains("should_use_transparent_popover_window(Some(mode))"));
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
    fn prompt_popover_open_preserves_existing_session_capture() {
        let source = include_str!("windows.rs");
        let start = source
            .find("pub fn toggle_prompt_popover_from_button")
            .expect("toggle command should exist");
        let end = source[start..]
            .find("pub fn show_prompt_button_controls_from_button")
            .expect("next command should follow toggle");
        let command_source = &source[start..start + end];

        assert!(command_source.contains("session_state.begin_if_new(session_id);"));
        assert!(command_source.contains("session_state.begin(session_id);"));
    }

    #[test]
    fn hide_prompt_popover_emits_dismissal_after_hiding_visible_window() {
        let source = include_str!("windows.rs");
        let start = source
            .find("pub fn hide_prompt_popover")
            .expect("hide_prompt_popover command should exist");
        let end = source[start..]
            .find("#[tauri::command]\npub fn show_prompt_popover_from_button")
            .expect("show_prompt_popover_from_button should follow hide_prompt_popover");
        let command_source = &source[start..start + end];

        assert!(command_source.contains("let was_visible = window.is_visible().unwrap_or(false);"));
        assert!(command_source.contains("if was_visible {"));
        assert!(command_source.contains("emit_popover_dismissed(&app);"));
        assert!(
            command_source
                .find("window.hide().map_err")
                .expect("hide should happen")
                < command_source
                    .find("emit_popover_dismissed(&app);")
                    .expect("dismissal should be emitted")
        );
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
        assert!(reuse_source.contains("set_position(logical_position(window_x, window_y))"));
        assert!(reuse_source.contains("show_non_activating_overlay_window(&window)?"));
        assert!(reuse_source.contains("emit_popover_opened(app, mode)"));
    }

    #[test]
    fn popover_builder_creates_hidden_non_focusable_window_before_panel_configuration() {
        let source = include_str!("windows.rs");
        let start = source
            .find("let window = WebviewWindowBuilder::new(app, POPOVER_WINDOW_LABEL")
            .expect("popover builder should exist");
        let end = source[start..]
            .find("set_popover_mode(Some(mode));")
            .expect("popover builder block should set mode");
        let block = &source[start..start + end];

        assert!(block.contains(".visible(false)"));
        assert!(block.contains(".focusable(false)"));
        assert!(block.contains("show_non_activating_overlay_window(&window)?"));
    }

    #[test]
    fn prompt_button_builder_creates_hidden_non_focusable_window() {
        let source = include_str!("windows.rs");
        let start = source
            .find("fn build_prompt_button_window")
            .expect("prompt button builder should exist");
        let end = source[start..]
            .find("#[tauri::command]\npub fn show_prompt_button")
            .expect("show_prompt_button should follow builder");
        let block = &source[start..start + end];

        assert!(block.contains(".visible(false)"));
        assert!(block.contains(".focusable(false)"));
        assert!(block.contains("show_non_activating_overlay_window(&window)?"));
    }

    #[test]
    fn reused_popover_configures_panel_before_any_show_call() {
        let source = include_str!("windows.rs");
        let start = source
            .find("if should_reuse_popover")
            .expect("reused popover branch should exist");
        let end = source[start..]
            .find("return Ok(());")
            .expect("reused popover branch should return");
        let block = &source[start..start + end];

        assert!(block.contains("show_non_activating_overlay_window(&window)?"));
        assert!(!block.contains("window.show().map_err"));
    }

    #[test]
    fn overlay_visibility_uses_single_non_activating_show_helper() {
        let source = include_str!("windows.rs");

        assert!(source.contains("fn show_non_activating_overlay_window"));
        assert!(source.contains("window.set_focusable(false)"));
        assert!(source.contains("configure_non_activating_panel"));
        assert!(source.contains("window.is_visible().unwrap_or(false)"));
        assert!(source.contains("Overlay window did not become visible."));
    }

    #[test]
    fn popover_toggle_still_uses_visible_state_to_close_open_prompt_list() {
        let source = include_str!("windows.rs");
        let start = source
            .find("pub fn toggle_prompt_popover_from_button")
            .expect("toggle command should exist");
        let end = source[start..]
            .find("pub fn show_prompt_button_controls_from_button")
            .expect("button controls command should follow toggle command");
        let block = &source[start..start + end];

        assert!(block.contains("window.is_visible().unwrap_or(false)"));
        assert!(block.contains("should_close_prompt_popover_on_toggle"));
        assert!(block.contains("window.hide().map_err"));
    }

    #[test]
    fn outside_click_paths_keep_using_visible_state() {
        let source = include_str!("windows.rs");

        assert!(source.contains("fn handle_prompt_popover_outside_click"));
        assert!(source.contains("popover_window.is_visible().unwrap_or(false)"));
        assert!(source.contains("window_rect(&app, POPOVER_WINDOW_LABEL)"));
    }

    #[test]
    fn popover_modes_use_transparent_native_window() {
        let source = include_str!("windows.rs");
        let start = source
            .find("fn show_popover_mode")
            .expect("show_popover_mode should exist");
        let end = source[start..]
            .find("#[tauri::command]\npub fn show_prompt_button")
            .expect("show_prompt_button should follow show_popover_mode");
        let command_source = &source[start..start + end];

        assert!(command_source.contains("should_use_transparent_popover_window(Some(mode))"));
        assert!(command_source.contains("configure_transparent_webview_window(&window)?"));
        assert!(command_source.contains("show_non_activating_overlay_window(&window)?"));
    }

    #[test]
    fn prompt_button_rebuild_closes_existing_window_and_rebuilds_at_same_position() {
        let source = include_str!("windows.rs");

        assert!(source.contains("pub fn rebuild_prompt_button_window"));
        assert!(source.contains("prompt_button_position_cmd(app.clone())"));
        assert!(source.contains("window.close()"));
        assert!(source.contains("build_prompt_button_window(app"));
    }
}
