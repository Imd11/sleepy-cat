use tauri::{Manager, WebviewUrl, WebviewWindowBuilder};

pub const BUTTON_WINDOW_LABEL: &str = "prompt-button";
pub const POPOVER_WINDOW_LABEL: &str = "prompt-popover";

pub const BUTTON_WIDTH: f64 = 112.0;
pub const BUTTON_HEIGHT: f64 = 40.0;
pub const POPOVER_WIDTH: f64 = 280.0;
pub const POPOVER_HEIGHT: f64 = 240.0;
pub const POPOVER_GAP: f64 = 8.0;

#[derive(serde::Serialize)]
pub struct PromptButtonPosition {
    pub x: f64,
    pub y: f64,
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
        window
            .set_position(tauri::Position::Logical(tauri::LogicalPosition { x, y }))
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn show_popover_mode(x: f64, y: f64, mode: &str, app: &tauri::AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window(POPOVER_WINDOW_LABEL) {
        window.close().map_err(|e| e.to_string())?;
    }

    let url = format!("index.html?mode={}", mode);
    let window = WebviewWindowBuilder::new(app, POPOVER_WINDOW_LABEL, WebviewUrl::App(url.into()))
        .title("Prompt Picker")
        .inner_size(POPOVER_WIDTH, POPOVER_HEIGHT)
        .resizable(false)
        .decorations(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .position(x, y)
        .build()
        .map_err(|e| e.to_string())?;
    crate::macos_panels::configure_non_activating_panel(&window)?;
    Ok(())
}

#[tauri::command]
pub fn show_prompt_button(x: f64, y: f64, app: tauri::AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window(BUTTON_WINDOW_LABEL) {
        window
            .set_position(tauri::Position::Physical(tauri::PhysicalPosition {
                x: x as i32,
                y: y as i32,
            }))
            .map_err(|e| e.to_string())?;
        window.show().map_err(|e| e.to_string())?;
        crate::macos_panels::configure_non_activating_panel(&window)?;
        Ok(())
    } else {
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
        .skip_taskbar(true)
        .position(x, y)
        .build()
        .map_err(|e| e.to_string())?;
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
pub fn show_prompt_popover_from_button(app: tauri::AppHandle) -> Result<(), String> {
    let position = button_relative_popover_position(&app, BUTTON_WIDTH);
    show_popover_mode(position.0, position.1, "popover", &app)
}

#[tauri::command]
pub fn show_prompt_button_controls_from_button(app: tauri::AppHandle) -> Result<(), String> {
    let position = button_relative_popover_position(&app, BUTTON_WIDTH);
    show_popover_mode(position.0, position.1, "button-controls", &app)
}

fn button_relative_popover_position(app: &tauri::AppHandle, button_side_width: f64) -> (f64, f64) {
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
                button_side_width,
                monitor.as_ref(),
            ))
        })
        .unwrap_or((100.0, 100.0))
}

fn clamp_popover_position(
    button_x: f64,
    button_y: f64,
    button_side_width: f64,
    monitor: Option<&tauri::Monitor>,
) -> (f64, f64) {
    let right_x = button_x + button_side_width + POPOVER_GAP;
    let left_x = button_x - POPOVER_WIDTH - POPOVER_GAP;
    let Some(monitor) = monitor else {
        return (right_x, button_y);
    };

    let scale = monitor.scale_factor();
    let monitor_x = monitor.position().x as f64 / scale;
    let monitor_y = monitor.position().y as f64 / scale;
    let monitor_width = monitor.size().width as f64 / scale;
    let monitor_height = monitor.size().height as f64 / scale;
    let monitor_right = monitor_x + monitor_width;
    let monitor_bottom = monitor_y + monitor_height;
    let margin = 8.0;

    let x = if right_x + POPOVER_WIDTH + margin <= monitor_right {
        right_x
    } else {
        left_x
    }
    .clamp(monitor_x + margin, monitor_right - POPOVER_WIDTH - margin);

    let y = button_y.clamp(monitor_y + margin, monitor_bottom - POPOVER_HEIGHT - margin);
    (x, y)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_popover_to_right_when_there_is_room() {
        let position = clamp_popover_position(100.0, 200.0, BUTTON_WIDTH, None);
        assert_eq!(position, (220.0, 200.0));
    }
}
