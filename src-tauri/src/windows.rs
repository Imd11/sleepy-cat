use tauri::{Emitter, Manager, WebviewUrl, WebviewWindowBuilder};

pub const BUTTON_WINDOW_LABEL: &str = "prompt-button";
pub const POPOVER_WINDOW_LABEL: &str = "prompt-popover";
pub const PAPER_FLIGHT_WINDOW_LABEL: &str = "paper-plane-flight";

pub const BUTTON_WIDTH: f64 = 132.0;
pub const BUTTON_HEIGHT: f64 = 132.0;
pub const BUTTON_WINDOW_TRANSPARENT: bool = true;
pub const POPOVER_WIDTH: f64 = 280.0;
pub const POPOVER_HEIGHT: f64 = 388.0;
pub const POPOVER_GAP: f64 = 4.0;

#[derive(Clone, Copy)]
struct MonitorBounds {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
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

fn paper_flight_points(
    monitor_width: f64,
    monitor_height: f64,
    button_x: f64,
    button_y: f64,
    monitor_x: f64,
    monitor_y: f64,
) -> (f64, f64, f64, f64) {
    let start_x = button_x + 102.0 - monitor_x;
    let start_y = button_y + 45.0 - monitor_y;
    let end_x = (start_x - 460.0).clamp(48.0, monitor_width - 48.0);
    let end_y = (start_y - 120.0).clamp(48.0, monitor_height - 48.0);
    (start_x, start_y, end_x, end_y)
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

#[tauri::command]
pub fn show_paper_plane_flight_from_button(app: tauri::AppHandle) -> Result<(), String> {
    let Some(button) = app.get_webview_window(BUTTON_WINDOW_LABEL) else {
        return Ok(());
    };
    let monitor = button
        .current_monitor()
        .map_err(|e| e.to_string())?
        .or(app.primary_monitor().map_err(|e| e.to_string())?);
    let Some(monitor) = monitor else {
        return Ok(());
    };

    let scale = monitor.scale_factor();
    let monitor_x = monitor.position().x as f64 / scale;
    let monitor_y = monitor.position().y as f64 / scale;
    let monitor_w = monitor.size().width as f64 / scale;
    let monitor_h = monitor.size().height as f64 / scale;

    let position = button.outer_position().map_err(|e| e.to_string())?;
    let button_scale = button.scale_factor().unwrap_or(1.0);
    let button_x = position.x as f64 / button_scale;
    let button_y = position.y as f64 / button_scale;
    let (start_x, start_y, end_x, end_y) = paper_flight_points(
        monitor_w, monitor_h, button_x, button_y, monitor_x, monitor_y,
    );

    if let Some(window) = app.get_webview_window(PAPER_FLIGHT_WINDOW_LABEL) {
        let _ = window.close();
    }

    let url = format!(
        "paper-flight.html?startX={:.0}&startY={:.0}&endX={:.0}&endY={:.0}",
        start_x, start_y, end_x, end_y
    );
    let window =
        WebviewWindowBuilder::new(&app, PAPER_FLIGHT_WINDOW_LABEL, WebviewUrl::App(url.into()))
            .title("Paper Plane")
            .inner_size(monitor_w, monitor_h)
            .resizable(false)
            .decorations(false)
            .always_on_top(true)
            .skip_taskbar(true)
            .visible(false)
            .focused(false)
            .focusable(false)
            .position(monitor_x, monitor_y)
            .build()
            .map_err(|e| e.to_string())?;

    crate::macos_panels::configure_transparent_webview_window(&window)?;
    crate::macos_panels::configure_non_activating_panel(&window)?;
    crate::macos_panels::configure_ignores_mouse_events(&window, true)?;
    window.show().map_err(|e| e.to_string())?;

    let app_for_close = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(1200));
        if let Some(window) = app_for_close.get_webview_window(PAPER_FLIGHT_WINDOW_LABEL) {
            let _ = window.close();
        }
    });

    Ok(())
}

#[tauri::command]
pub fn hide_paper_plane_flight(app: tauri::AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window(PAPER_FLIGHT_WINDOW_LABEL) {
        window.close().map_err(|e| e.to_string())?;
    }
    Ok(())
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
    fn paper_flight_points_move_left_and_up_when_space_allows() {
        let (sx, sy, ex, ey) = paper_flight_points(1440.0, 900.0, 1000.0, 600.0, 0.0, 0.0);
        assert_eq!((sx, sy), (1102.0, 645.0));
        assert!(ex < sx);
        assert!(ey < sy);
    }

    #[test]
    fn paper_flight_points_stay_inside_monitor_bounds() {
        let (_sx, _sy, ex, ey) = paper_flight_points(500.0, 320.0, 40.0, 30.0, 0.0, 0.0);
        assert!((48.0..=452.0).contains(&ex));
        assert!((48.0..=272.0).contains(&ey));
    }

    #[test]
    fn paper_flight_window_has_backend_close_fallback() {
        let source = include_str!("windows.rs");
        assert!(source.contains("PAPER_FLIGHT_WINDOW_LABEL"));
        assert!(source.contains("Duration::from_millis(1200)"));
    }

    #[test]
    fn paper_flight_window_is_configured_before_showing() {
        let source = include_str!("windows.rs");
        let start = source
            .find("pub fn show_paper_plane_flight_from_button")
            .expect("paper flight command should exist");
        let end = source[start..]
            .find("let app_for_close = app.clone();")
            .expect("paper flight backend close fallback should exist");
        let command_source = &source[start..start + end];
        assert!(command_source.contains(".visible(false)"));
        assert!(command_source.contains(".focused(false)"));
        assert!(command_source.contains(".focusable(false)"));
        assert!(command_source.contains("configure_ignores_mouse_events(&window, true)?;"));
        assert!(command_source.contains("window.show().map_err"));
        assert!(
            command_source.find("configure_ignores_mouse_events(&window, true)?;")
                < command_source.find("window.show().map_err")
        );
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
