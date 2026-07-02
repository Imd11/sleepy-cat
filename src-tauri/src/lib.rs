use tauri::{Manager, WindowEvent};

mod platform;
pub use platform::{
    accessibility_status, frontmost_app, AccessibilityStatus, CandidateInput, FrontmostApp,
};
mod overlay_position;
pub use overlay_position::{prompt_button_position, OverlayPoint};
mod windows;
pub use windows::{
    hide_prompt_button, hide_prompt_popover, move_prompt_button_to, prompt_button_position_cmd,
    show_prompt_button, show_prompt_button_controls_from_button, show_prompt_popover,
    show_prompt_popover_from_button,
};
mod macos_panels;
pub use macos_panels::{activate_main_window, configure_non_activating_panel};

#[tauri::command]
fn accessibility_status_cmd() -> AccessibilityStatus {
    accessibility_status()
}

#[tauri::command]
fn frontmost_app_cmd() -> Option<FrontmostApp> {
    frontmost_app()
}

#[tauri::command]
fn current_input_target(
    state: tauri::State<LastInputTargetState>,
) -> Option<platform::InputTarget> {
    let target = platform::macos::current_input_target()?;
    record_last_input_target_if_valid(state.inner(), &target);
    Some(target)
}

#[tauri::command]
fn paste_prompt(body: String) -> Result<(), String> {
    platform::macos::paste_prompt(&body)
}

#[tauri::command]
fn paste_prompt_to_app(body: String, bundle_id: String) -> Result<(), String> {
    platform::macos::paste_prompt_to_app(&body, &bundle_id)
}

#[tauri::command]
fn paste_prompt_to_last_target(
    body: String,
    state: tauri::State<LastInputTargetState>,
) -> Result<(), String> {
    paste_prompt_to_last_target_impl(&body, state.inner())
}

fn paste_prompt_to_last_target_impl(
    body: &str,
    state: &LastInputTargetState,
) -> Result<(), String> {
    let Some(target) = state.get() else {
        return Err("Click into a text field first, then choose a prompt.".to_string());
    };
    platform::macos::paste_prompt_to_app(body, &target.app.bundle_id)
}

#[tauri::command]
fn open_main_window(app: tauri::AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("main") {
        window.show().map_err(|e| e.to_string())?;
        activate_main_window(&window)?;
        window.set_focus().map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[derive(Clone, Debug, serde::Serialize)]
pub struct LastInputTarget {
    pub app: FrontmostApp,
    pub observed_at_ms: u128,
}

#[derive(Default)]
pub struct LastInputTargetState(std::sync::Mutex<Option<LastInputTarget>>);

impl LastInputTargetState {
    pub fn set(&self, target: LastInputTarget) {
        *self.0.lock().expect("last input target lock poisoned") = Some(target);
    }

    pub fn get(&self) -> Option<LastInputTarget> {
        self.0
            .lock()
            .expect("last input target lock poisoned")
            .clone()
    }
}

fn record_last_input_target_if_valid(state: &LastInputTargetState, target: &platform::InputTarget) {
    let Some(app) = target.app.clone() else {
        return;
    };
    if app.bundle_id == "local.promptpicker.dev" || app.name == "Prompt Picker" {
        return;
    }
    state.set(LastInputTarget {
        app,
        observed_at_ms: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis(),
    });
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(LastInputTargetState::default())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_shell::init())
        .invoke_handler(tauri::generate_handler![
            accessibility_status_cmd,
            frontmost_app_cmd,
            current_input_target,
            paste_prompt,
            paste_prompt_to_app,
            paste_prompt_to_last_target,
            show_prompt_button,
            hide_prompt_button,
            show_prompt_popover,
            hide_prompt_popover,
            show_prompt_popover_from_button,
            show_prompt_button_controls_from_button,
            prompt_button_position_cmd,
            move_prompt_button_to,
            open_main_window
        ])
        .setup(|app| {
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            let window = app.get_webview_window("main").unwrap();
            window.set_title("Prompt Picker").unwrap();
            let main_window = window.clone();
            window.on_window_event(move |event| {
                if let WindowEvent::CloseRequested { api, .. } = event {
                    api.prevent_close();
                    let _ = main_window.hide();
                }
            });
            let _ = show_prompt_button(960.0, 700.0, app.handle().clone());
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod last_input_target_tests {
    use super::*;

    #[test]
    fn stores_and_reads_last_input_target() {
        let state = LastInputTargetState::default();
        let target = LastInputTarget {
            app: FrontmostApp {
                name: "Notes".to_string(),
                bundle_id: "com.apple.Notes".to_string(),
            },
            observed_at_ms: 123,
        };

        state.set(target);

        assert_eq!(state.get().unwrap().app.bundle_id, "com.apple.Notes");
    }

    #[test]
    fn records_non_prompt_picker_input_target() {
        let state = LastInputTargetState::default();
        let target = platform::InputTarget {
            frame: CandidateInput {
                x: 0.0,
                y: 0.0,
                width: 10.0,
                height: 10.0,
            },
            window_frame: CandidateInput {
                x: 0.0,
                y: 0.0,
                width: 100.0,
                height: 100.0,
            },
            button_position: (10.0, 10.0),
            app: Some(FrontmostApp {
                name: "Notes".to_string(),
                bundle_id: "com.apple.Notes".to_string(),
            }),
        };

        record_last_input_target_if_valid(&state, &target);

        assert_eq!(state.get().unwrap().app.bundle_id, "com.apple.Notes");
    }

    #[test]
    fn skips_prompt_picker_as_last_input_target() {
        let state = LastInputTargetState::default();
        let target = platform::InputTarget {
            frame: CandidateInput {
                x: 0.0,
                y: 0.0,
                width: 10.0,
                height: 10.0,
            },
            window_frame: CandidateInput {
                x: 0.0,
                y: 0.0,
                width: 100.0,
                height: 100.0,
            },
            button_position: (10.0, 10.0),
            app: Some(FrontmostApp {
                name: "Prompt Picker".to_string(),
                bundle_id: "local.promptpicker.dev".to_string(),
            }),
        };

        record_last_input_target_if_valid(&state, &target);

        assert!(state.get().is_none());
    }

    #[test]
    fn missing_last_target_returns_clear_error() {
        let state = LastInputTargetState::default();
        let result = paste_prompt_to_last_target_impl("hello", &state);

        assert_eq!(
            result.unwrap_err(),
            "Click into a text field first, then choose a prompt."
        );
    }
}
