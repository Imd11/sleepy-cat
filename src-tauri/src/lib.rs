use tauri::{
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::TrayIconBuilder,
    Manager, WindowEvent,
};

mod platform;
pub use platform::{
    accessibility_status, frontmost_app, AccessibilityStatus, AutosendOutcome, CandidateInput,
    FrontmostApp,
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
fn open_accessibility_settings() -> Result<(), String> {
    platform::macos::open_accessibility_settings()
}

#[tauri::command]
fn frontmost_app_cmd() -> Option<FrontmostApp> {
    frontmost_app()
}

#[tauri::command]
fn current_input_target(
    state: tauri::State<LastInputTargetState>,
) -> Option<platform::InputTarget> {
    if let Some(target) = platform::macos::current_input_target() {
        record_last_input_target_if_valid(state.inner(), &target);
        return Some(target);
    }

    if let Some(app) = frontmost_app() {
        record_last_app_if_valid(state.inner(), app);
    }

    None
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
fn paste_prompt_and_submit_to_last_target(
    body: String,
    state: tauri::State<LastInputTargetState>,
) -> Result<AutosendOutcome, String> {
    paste_prompt_and_submit_to_last_target_impl(&body, state.inner())
}

fn paste_prompt_and_submit_to_last_target_impl(
    body: &str,
    state: &LastInputTargetState,
) -> Result<AutosendOutcome, String> {
    paste_prompt_and_submit_to_last_target_with_sender(
        body,
        state,
        platform::macos::paste_prompt_and_submit_to_foreground,
    )
}

fn paste_prompt_and_submit_to_last_target_with_sender<F>(
    body: &str,
    _state: &LastInputTargetState,
    sender: F,
) -> Result<AutosendOutcome, String>
where
    F: FnOnce(&str) -> Result<AutosendOutcome, String>,
{
    sender(body)
}

#[cfg(test)]
fn last_target_bundle_id(state: &LastInputTargetState) -> Result<String, String> {
    let Some(target) = state.get() else {
        return Err("Click into a text field first, then choose a prompt.".to_string());
    };
    Ok(target.app.bundle_id)
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
    pub click_point: Option<TargetClickPoint>,
}

#[derive(Clone, Copy, Debug, serde::Serialize)]
pub struct TargetClickPoint {
    pub x: f64,
    pub y: f64,
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
    if is_prompt_picker_app(&app) {
        return;
    }
    state.set(LastInputTarget {
        app,
        observed_at_ms: now_ms(),
        click_point: Some(TargetClickPoint {
            x: target.click_point.0,
            y: target.click_point.1,
        }),
    });
}

fn record_last_app_if_valid(state: &LastInputTargetState, app: FrontmostApp) {
    if is_prompt_picker_app(&app) {
        return;
    }
    state.set(LastInputTarget {
        app,
        observed_at_ms: now_ms(),
        click_point: None,
    });
}

fn is_prompt_picker_app(app: &FrontmostApp) -> bool {
    app.bundle_id == "local.promptpicker.dev" || app.name == "Prompt Picker"
}

fn now_ms() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}

const TRAY_ID: &str = "prompt-picker-tray";
const TRAY_OPEN_MAIN_ID: &str = "open-main-window";
const TRAY_SHOW_BUTTON_ID: &str = "show-floating-button";
const TRAY_HIDE_BUTTON_ID: &str = "hide-floating-button";
const TRAY_QUIT_ID: &str = "quit";

#[derive(Debug, PartialEq, Eq)]
enum TrayMenuAction {
    OpenMainWindow,
    ShowFloatingButton,
    HideFloatingButton,
    Quit,
    Unknown,
}

fn tray_menu_action(id: &str) -> TrayMenuAction {
    match id {
        TRAY_OPEN_MAIN_ID => TrayMenuAction::OpenMainWindow,
        TRAY_SHOW_BUTTON_ID => TrayMenuAction::ShowFloatingButton,
        TRAY_HIDE_BUTTON_ID => TrayMenuAction::HideFloatingButton,
        TRAY_QUIT_ID => TrayMenuAction::Quit,
        _ => TrayMenuAction::Unknown,
    }
}

fn setup_menu_bar_app(app_handle: &tauri::AppHandle) -> Result<(), String> {
    let open_main = MenuItem::with_id(
        app_handle,
        TRAY_OPEN_MAIN_ID,
        "Open Prompt Picker",
        true,
        None::<&str>,
    )
    .map_err(|e| e.to_string())?;
    let show_button = MenuItem::with_id(
        app_handle,
        TRAY_SHOW_BUTTON_ID,
        "Show Calico",
        true,
        None::<&str>,
    )
    .map_err(|e| e.to_string())?;
    let hide_button = MenuItem::with_id(
        app_handle,
        TRAY_HIDE_BUTTON_ID,
        "Hide Calico",
        true,
        None::<&str>,
    )
    .map_err(|e| e.to_string())?;
    let separator = PredefinedMenuItem::separator(app_handle).map_err(|e| e.to_string())?;
    let quit = MenuItem::with_id(
        app_handle,
        TRAY_QUIT_ID,
        "Quit Prompt Picker",
        true,
        None::<&str>,
    )
    .map_err(|e| e.to_string())?;
    let menu = Menu::with_items(
        app_handle,
        &[&open_main, &show_button, &hide_button, &separator, &quit],
    )
    .map_err(|e| e.to_string())?;

    let mut tray_builder = TrayIconBuilder::with_id(TRAY_ID)
        .menu(&menu)
        .tooltip("Prompt Picker")
        .show_menu_on_left_click(true)
        .icon_as_template(true)
        .on_menu_event(|app, event| match tray_menu_action(event.id().as_ref()) {
            TrayMenuAction::OpenMainWindow => {
                let _ = open_main_window(app.clone());
            }
            TrayMenuAction::ShowFloatingButton => {
                let position = prompt_button_position_cmd(app.clone()).ok().flatten();
                let (x, y) = position
                    .map(|point| (point.x, point.y))
                    .unwrap_or((960.0, 700.0));
                let _ = show_prompt_button(x, y, app.clone());
            }
            TrayMenuAction::HideFloatingButton => {
                let _ = hide_prompt_popover(app.clone());
                let _ = hide_prompt_button(app.clone());
            }
            TrayMenuAction::Quit => app.exit(0),
            TrayMenuAction::Unknown => {}
        });

    if let Some(icon) = app_handle.default_window_icon().cloned() {
        tray_builder = tray_builder.icon(icon);
    }

    tray_builder.build(app_handle).map_err(|e| e.to_string())?;
    Ok(())
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
            open_accessibility_settings,
            frontmost_app_cmd,
            current_input_target,
            paste_prompt,
            paste_prompt_to_app,
            paste_prompt_to_last_target,
            paste_prompt_and_submit_to_last_target,
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

            setup_menu_bar_app(app.handle())?;

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
            click_point: None,
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
            click_point: (6.0, 5.0),
            app: Some(FrontmostApp {
                name: "Notes".to_string(),
                bundle_id: "com.apple.Notes".to_string(),
            }),
        };

        record_last_input_target_if_valid(&state, &target);

        assert_eq!(state.get().unwrap().app.bundle_id, "com.apple.Notes");
        assert_eq!(state.get().unwrap().click_point.unwrap().x, 6.0);
    }

    #[test]
    fn records_input_click_point_separately_from_button_position() {
        let state = LastInputTargetState::default();
        let target = platform::InputTarget {
            frame: CandidateInput {
                x: 700.0,
                y: 680.0,
                width: 500.0,
                height: 96.0,
            },
            window_frame: CandidateInput {
                x: 10.0,
                y: 20.0,
                width: 1200.0,
                height: 800.0,
            },
            button_position: (1200.0, 776.0),
            click_point: (760.0, 728.0),
            app: Some(FrontmostApp {
                name: "Codex".to_string(),
                bundle_id: "com.openai.codex".to_string(),
            }),
        };

        record_last_input_target_if_valid(&state, &target);

        let stored = state.get().unwrap();
        let click_point = stored.click_point.unwrap();
        assert_eq!(click_point.x, 760.0);
        assert_eq!(click_point.y, 728.0);
        assert_ne!(click_point.x, target.button_position.0);
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
            click_point: (6.0, 5.0),
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

    #[test]
    fn autosend_attempts_foreground_sender_without_last_target() {
        let state = LastInputTargetState::default();
        let result = paste_prompt_and_submit_to_last_target_with_sender("hello", &state, |body| {
            assert_eq!(body, "hello");
            Ok(AutosendOutcome::sent())
        });

        let outcome = result.unwrap();
        assert!(outcome.copied);
        assert!(outcome.sent);
    }

    #[test]
    fn autosend_propagates_foreground_sender_errors() {
        let state = LastInputTargetState::default();
        let result = paste_prompt_and_submit_to_last_target_with_sender("hello", &state, |_| {
            Err("foreground keyboard failed".to_string())
        });

        assert_eq!(result.unwrap_err(), "foreground keyboard failed");
    }

    #[test]
    fn autosend_returns_foreground_outcome_without_last_target() {
        let state = LastInputTargetState::default();
        let result = paste_prompt_and_submit_to_last_target_with_sender("hello", &state, |body| {
            assert_eq!(body, "hello");
            Ok(AutosendOutcome::keyboard_failed(
                "System Events denied".to_string(),
            ))
        });

        let outcome = result.unwrap();
        assert!(outcome.copied);
        assert!(!outcome.sent);
        assert_eq!(outcome.error.as_deref(), Some("System Events denied"));
    }

    #[test]
    fn accepts_non_codex_target_for_autosend() {
        let state = LastInputTargetState::default();
        state.set(LastInputTarget {
            app: FrontmostApp {
                name: "WeChat".to_string(),
                bundle_id: "com.tencent.xinWeChat".to_string(),
            },
            observed_at_ms: 123,
            click_point: None,
        });

        assert_eq!(
            last_target_bundle_id(&state).unwrap(),
            "com.tencent.xinWeChat"
        );
    }

    #[test]
    fn last_target_for_wechat_does_not_need_click_point() {
        let state = LastInputTargetState::default();
        state.set(LastInputTarget {
            app: FrontmostApp {
                name: "WeChat".to_string(),
                bundle_id: "com.tencent.xinWeChat".to_string(),
            },
            observed_at_ms: 123,
            click_point: None,
        });

        assert_eq!(
            last_target_bundle_id(&state).unwrap(),
            "com.tencent.xinWeChat"
        );
    }

    #[test]
    fn accepts_codex_target_for_autosend() {
        let state = LastInputTargetState::default();
        state.set(LastInputTarget {
            app: FrontmostApp {
                name: "Codex".to_string(),
                bundle_id: "com.openai.codex".to_string(),
            },
            observed_at_ms: 123,
            click_point: None,
        });

        assert_eq!(last_target_bundle_id(&state).unwrap(), "com.openai.codex");
    }

    #[test]
    fn autosend_does_not_require_click_point_for_codex() {
        let state = LastInputTargetState::default();
        state.set(LastInputTarget {
            app: FrontmostApp {
                name: "Codex".to_string(),
                bundle_id: "com.openai.codex".to_string(),
            },
            observed_at_ms: 123,
            click_point: None,
        });

        assert_eq!(last_target_bundle_id(&state).unwrap(), "com.openai.codex");
    }

    #[test]
    fn records_frontmost_app_as_last_target_fallback() {
        let state = LastInputTargetState::default();
        record_last_app_if_valid(
            &state,
            FrontmostApp {
                name: "WeChat".to_string(),
                bundle_id: "com.tencent.xinWeChat".to_string(),
            },
        );

        assert_eq!(state.get().unwrap().app.bundle_id, "com.tencent.xinWeChat");
    }

    #[test]
    fn skips_prompt_picker_as_frontmost_app_fallback() {
        let state = LastInputTargetState::default();
        record_last_app_if_valid(
            &state,
            FrontmostApp {
                name: "Prompt Picker".to_string(),
                bundle_id: "local.promptpicker.dev".to_string(),
            },
        );

        assert!(state.get().is_none());
    }
}

#[cfg(test)]
mod menu_bar_app_tests {
    use super::*;

    #[test]
    fn maps_tray_menu_item_ids_to_actions() {
        assert_eq!(
            tray_menu_action(TRAY_OPEN_MAIN_ID),
            TrayMenuAction::OpenMainWindow
        );
        assert_eq!(
            tray_menu_action(TRAY_SHOW_BUTTON_ID),
            TrayMenuAction::ShowFloatingButton
        );
        assert_eq!(
            tray_menu_action(TRAY_HIDE_BUTTON_ID),
            TrayMenuAction::HideFloatingButton
        );
        assert_eq!(tray_menu_action(TRAY_QUIT_ID), TrayMenuAction::Quit);
    }

    #[test]
    fn ignores_unknown_tray_menu_item_ids() {
        assert_eq!(tray_menu_action("unknown"), TrayMenuAction::Unknown);
    }

    #[test]
    fn macos_info_plist_marks_app_as_menu_bar_app() {
        let info_plist = include_str!("../Info.plist");

        assert!(info_plist.contains("<key>LSUIElement</key>"));
        assert!(info_plist.contains("<true/>"));
    }

    #[test]
    fn macos_info_plist_declares_apple_events_usage() {
        let info_plist = include_str!("../Info.plist");

        assert!(info_plist.contains("<key>NSAppleEventsUsageDescription</key>"));
        assert!(info_plist.contains("send keyboard events"));
    }

    #[test]
    fn tauri_capabilities_do_not_allow_blocking_message_dialogs() {
        let capabilities = include_str!("../capabilities/default.json");

        assert!(!capabilities.contains("\"dialog:allow-message\""));
    }
}
