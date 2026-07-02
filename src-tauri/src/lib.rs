use tauri::{
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::TrayIconBuilder,
    Manager, WindowEvent,
};

mod platform;
pub use platform::{
    accessibility_status, frontmost_app, request_accessibility_permission, AccessibilityStatus,
    AutosendOutcome, CandidateInput, FrontmostApp,
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
fn request_accessibility_permission_cmd() -> AccessibilityStatus {
    request_accessibility_permission()
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
fn begin_prompt_pick_session(
    session_state: tauri::State<PromptPickSessionState>,
    recent_state: tauri::State<LastInputTargetState>,
) -> Option<FrontmostApp> {
    let target = prompt_pick_session_target(
        frontmost_app(),
        platform::macos::visible_apps(),
        recent_state.inner().get(),
    )?;
    record_prompt_pick_session_target_if_valid(session_state.inner(), target)
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
    session_state: tauri::State<PromptPickSessionState>,
) -> Result<AutosendOutcome, String> {
    paste_prompt_and_submit_to_last_target_impl(&body, session_state.inner())
}

#[derive(Clone, Debug, serde::Serialize)]
pub struct AutosendSequenceOutcome {
    pub copied: bool,
    pub sent: bool,
    pub sent_count: usize,
    pub failed_index: Option<usize>,
    pub error: Option<String>,
    pub reason: Option<platform::macos::AutosendFailureReason>,
}

impl AutosendSequenceOutcome {
    fn sent_all(count: usize) -> Self {
        Self {
            copied: true,
            sent: true,
            sent_count: count,
            failed_index: None,
            error: None,
            reason: None,
        }
    }

    fn from_failure(outcome: AutosendOutcome, sent_count: usize, failed_index: usize) -> Self {
        Self {
            copied: outcome.copied,
            sent: false,
            sent_count,
            failed_index: Some(failed_index),
            error: outcome.error,
            reason: outcome.reason,
        }
    }
}

#[tauri::command]
fn paste_prompt_sequence_and_submit_to_last_target(
    bodies: Vec<String>,
    interval_ms: u64,
    session_state: tauri::State<PromptPickSessionState>,
) -> Result<AutosendSequenceOutcome, String> {
    paste_prompt_sequence_and_submit_to_last_target_impl(
        &bodies,
        interval_ms,
        session_state.inner(),
    )
}

fn paste_prompt_sequence_and_submit_to_last_target_impl(
    bodies: &[String],
    interval_ms: u64,
    state: &PromptPickSessionState,
) -> Result<AutosendSequenceOutcome, String> {
    paste_prompt_sequence_and_submit_to_session_target_with_senders(
        bodies,
        interval_ms,
        state,
        |body, bundle_id, click_point| {
            platform::macos::paste_prompt_and_submit_to_app_clipboard(
                body,
                bundle_id,
                click_point.map(|point| (point.x, point.y)),
            )
        },
        platform::macos::copy_prompt_to_clipboard,
        |delay_ms| std::thread::sleep(std::time::Duration::from_millis(delay_ms)),
    )
}

fn paste_prompt_and_submit_to_last_target_impl(
    body: &str,
    state: &PromptPickSessionState,
) -> Result<AutosendOutcome, String> {
    paste_prompt_and_submit_to_session_target_with_senders(
        body,
        state,
        |body, bundle_id, click_point| {
            platform::macos::paste_prompt_and_submit_to_app_clipboard(
                body,
                bundle_id,
                click_point.map(|point| (point.x, point.y)),
            )
        },
        platform::macos::copy_prompt_to_clipboard,
    )
}

const MIN_SEQUENCE_INTERVAL_MS: u64 = 200;
const MAX_SEQUENCE_INTERVAL_MS: u64 = 3_000;

fn clamp_sequence_interval_ms(interval_ms: u64) -> u64 {
    interval_ms.clamp(MIN_SEQUENCE_INTERVAL_MS, MAX_SEQUENCE_INTERVAL_MS)
}

fn paste_prompt_and_submit_to_session_target_with_senders<A, C>(
    body: &str,
    state: &PromptPickSessionState,
    app_sender: A,
    copy_sender: C,
) -> Result<AutosendOutcome, String>
where
    A: FnOnce(&str, &str, Option<TargetClickPoint>) -> AutosendOutcome,
    C: FnOnce(&str) -> Result<(), String>,
{
    let Some(target) = state.take() else {
        return Ok(copy_without_sending(
            body,
            copy_sender,
            "No prompt pick target app was recorded for autosend.",
        ));
    };

    if is_unsafe_autosend_target(&target.app) {
        return Ok(copy_without_sending(
            body,
            copy_sender,
            "Target app is not safe for autosend.",
        ));
    }

    if !allows_app_only_autosend(&target.app) {
        return Ok(copy_without_sending(
            body,
            copy_sender,
            "Target app is not safe for app-only autosend.",
        ));
    }

    Ok(app_sender(body, &target.app.bundle_id, target.click_point))
}

fn paste_prompt_sequence_and_submit_to_session_target_with_senders<A, C, S>(
    bodies: &[String],
    interval_ms: u64,
    state: &PromptPickSessionState,
    mut app_sender: A,
    copy_sender: C,
    mut sleeper: S,
) -> Result<AutosendSequenceOutcome, String>
where
    A: FnMut(&str, &str, Option<TargetClickPoint>) -> AutosendOutcome,
    C: FnOnce(&str) -> Result<(), String>,
    S: FnMut(u64),
{
    let clean_bodies: Vec<String> = bodies
        .iter()
        .map(|body| body.trim().to_string())
        .filter(|body| !body.is_empty())
        .collect();
    let Some(first_body) = clean_bodies.first() else {
        return Ok(AutosendSequenceOutcome::from_failure(
            AutosendOutcome::copy_failed("Prompt group is empty.".to_string()),
            0,
            1,
        ));
    };

    let Some(target) = state.take() else {
        let outcome = copy_without_sending(
            first_body,
            copy_sender,
            "No prompt pick target app was recorded for autosend.",
        );
        return Ok(AutosendSequenceOutcome::from_failure(outcome, 0, 1));
    };

    if is_unsafe_autosend_target(&target.app) || !allows_app_only_autosend(&target.app) {
        let outcome = copy_without_sending(
            first_body,
            copy_sender,
            "Target app is not safe for app-only autosend.",
        );
        return Ok(AutosendSequenceOutcome::from_failure(outcome, 0, 1));
    }

    let delay_ms = clamp_sequence_interval_ms(interval_ms);
    for (index, body) in clean_bodies.iter().enumerate() {
        let outcome = app_sender(body, &target.app.bundle_id, target.click_point);
        if !outcome.sent {
            return Ok(AutosendSequenceOutcome::from_failure(outcome, index, index + 1));
        }
        if index + 1 < clean_bodies.len() {
            sleeper(delay_ms);
        }
    }

    Ok(AutosendSequenceOutcome::sent_all(clean_bodies.len()))
}

fn copy_without_sending<C>(body: &str, copy_sender: C, message: &str) -> AutosendOutcome
where
    C: FnOnce(&str) -> Result<(), String>,
{
    match copy_sender(body) {
        Ok(()) => AutosendOutcome::copied_without_send(message.to_string()),
        Err(error) => AutosendOutcome::copy_failed(error),
    }
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

#[tauri::command]
fn quit_prompt_picker(app: tauri::AppHandle) {
    app.exit(0);
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

    pub fn clear(&self) {
        *self.0.lock().expect("last input target lock poisoned") = None;
    }

    pub fn get(&self) -> Option<LastInputTarget> {
        self.0
            .lock()
            .expect("last input target lock poisoned")
            .clone()
    }
}

#[derive(Clone, Debug, serde::Serialize)]
pub struct PromptPickSessionTarget {
    pub app: FrontmostApp,
    pub observed_at_ms: u128,
    pub click_point: Option<TargetClickPoint>,
}

#[derive(Default)]
pub struct PromptPickSessionState(std::sync::Mutex<Option<PromptPickSessionTarget>>);

impl PromptPickSessionState {
    pub fn set(&self, target: PromptPickSessionTarget) {
        *self
            .0
            .lock()
            .expect("prompt pick session lock poisoned") = Some(target);
    }

    pub fn clear(&self) {
        *self
            .0
            .lock()
            .expect("prompt pick session lock poisoned") = None;
    }

    pub fn get(&self) -> Option<PromptPickSessionTarget> {
        self.0
            .lock()
            .expect("prompt pick session lock poisoned")
            .clone()
    }

    pub fn take(&self) -> Option<PromptPickSessionTarget> {
        self.0
            .lock()
            .expect("prompt pick session lock poisoned")
            .take()
    }
}

fn record_prompt_pick_session_target_if_valid(
    state: &PromptPickSessionState,
    target: PromptPickSessionTarget,
) -> Option<FrontmostApp> {
    if !is_usable_autosend_app(&target.app) {
        state.clear();
        return None;
    }

    let app = target.app.clone();
    state.set(target);
    Some(app)
}

fn prompt_pick_session_target(
    frontmost: Option<FrontmostApp>,
    visible_apps: Vec<FrontmostApp>,
    recent_target: Option<LastInputTarget>,
) -> Option<PromptPickSessionTarget> {
    let frontmost = frontmost?;
    if is_usable_autosend_app(&frontmost) {
        let click_point = recent_target
            .as_ref()
            .filter(|target| target.app.bundle_id == frontmost.bundle_id)
            .and_then(|target| target.click_point);
        return Some(PromptPickSessionTarget {
            app: frontmost,
            observed_at_ms: now_ms(),
            click_point,
        });
    }

    if !is_prompt_picker_app(&frontmost) {
        return None;
    }

    if let Some(target) = recent_target
        .as_ref()
        .filter(|target| is_recent_prompt_target(target))
        .filter(|target| is_usable_autosend_app(&target.app))
    {
        return Some(PromptPickSessionTarget {
            app: target.app.clone(),
            observed_at_ms: now_ms(),
            click_point: target.click_point,
        });
    }

    if let Some(app) = visible_apps
        .into_iter()
        .find(|app| !is_prompt_picker_app(app))
    {
        if is_usable_autosend_app(&app) {
            return Some(PromptPickSessionTarget {
                app,
                observed_at_ms: now_ms(),
                click_point: None,
            });
        }
        return None;
    }

    recent_target
        .filter(is_recent_prompt_target)
        .filter(|target| is_usable_autosend_app(&target.app))
        .map(|target| PromptPickSessionTarget {
            app: target.app,
            observed_at_ms: now_ms(),
            click_point: target.click_point,
        })
}

fn record_last_input_target_if_valid(state: &LastInputTargetState, target: &platform::InputTarget) {
    let Some(app) = target.app.clone() else {
        return;
    };
    if is_prompt_picker_app(&app) {
        return;
    }
    if is_unsafe_autosend_target(&app) {
        state.clear();
        return;
    }
    let click_point = if has_focused_input_frame(&target.frame) || allows_fallback_click_point(&app)
    {
        Some(TargetClickPoint {
            x: target.click_point.0,
            y: target.click_point.1,
        })
    } else if allows_app_only_autosend(&app) {
        None
    } else {
        state.clear();
        return;
    };

    state.set(LastInputTarget {
        app,
        observed_at_ms: now_ms(),
        click_point,
    });
}

fn record_last_app_if_valid(state: &LastInputTargetState, app: FrontmostApp) {
    if is_prompt_picker_app(&app) {
        return;
    }
    if is_unsafe_autosend_target(&app) {
        state.clear();
        return;
    }
    if !allows_app_only_autosend(&app) {
        state.clear();
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

fn is_usable_autosend_app(app: &FrontmostApp) -> bool {
    !is_prompt_picker_app(app) && allows_app_only_autosend(app)
}

const PROMPT_PICK_RECENT_TARGET_MAX_AGE_MS: u128 = 5_000;

fn is_recent_prompt_target(target: &LastInputTarget) -> bool {
    now_ms().saturating_sub(target.observed_at_ms) <= PROMPT_PICK_RECENT_TARGET_MAX_AGE_MS
}

fn has_focused_input_frame(frame: &CandidateInput) -> bool {
    frame.width > 1.0 && frame.height > 1.0
}

fn allows_fallback_click_point(app: &FrontmostApp) -> bool {
    app.bundle_id == "com.openai.codex" || app.name == "Codex"
}

fn allows_app_only_autosend(app: &FrontmostApp) -> bool {
    !is_unsafe_autosend_target(app)
}

fn is_unsafe_autosend_target(app: &FrontmostApp) -> bool {
    matches!(
        app.bundle_id.as_str(),
        "com.apple.finder"
            | "com.apple.systempreferences"
            | "com.apple.SystemSettings"
            | "com.apple.dock"
            | "com.apple.controlcenter"
            | "com.apple.notificationcenterui"
    ) || matches!(
        app.name.as_str(),
        "Finder" | "Desktop" | "System Settings" | "System Preferences" | "Dock" | "Control Center"
    )
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
const TRAY_OPEN_ACCESSIBILITY_ID: &str = "open-accessibility-settings";
const TRAY_QUIT_ID: &str = "quit";

#[derive(Debug, PartialEq, Eq)]
enum TrayMenuAction {
    OpenMainWindow,
    ShowFloatingButton,
    HideFloatingButton,
    OpenAccessibilitySettings,
    Quit,
    Unknown,
}

fn tray_menu_action(id: &str) -> TrayMenuAction {
    match id {
        TRAY_OPEN_MAIN_ID => TrayMenuAction::OpenMainWindow,
        TRAY_SHOW_BUTTON_ID => TrayMenuAction::ShowFloatingButton,
        TRAY_HIDE_BUTTON_ID => TrayMenuAction::HideFloatingButton,
        TRAY_OPEN_ACCESSIBILITY_ID => TrayMenuAction::OpenAccessibilitySettings,
        TRAY_QUIT_ID => TrayMenuAction::Quit,
        _ => TrayMenuAction::Unknown,
    }
}

fn parse_saved_button_position(contents: &str) -> Option<(f64, f64)> {
    let settings: serde_json::Value = serde_json::from_str(contents).ok()?;
    let position = settings.pointer("/overlayPlacement/buttonPosition")?;
    let x = position.get("x")?.as_f64()?;
    let y = position.get("y")?.as_f64()?;
    if !x.is_finite() || !y.is_finite() {
        return None;
    }
    Some((x, y))
}

fn default_settings_value() -> serde_json::Value {
    serde_json::json!({
        "version": 1,
        "blacklistedApps": [],
        "overlayPlacement": {
            "buttonOffset": null,
            "buttonPosition": null
        },
        "floatingButton": {
            "visible": true
        }
    })
}

fn settings_path(app: &tauri::AppHandle) -> Option<std::path::PathBuf> {
    app.path().app_data_dir().ok().map(|dir| dir.join("settings.json"))
}

fn read_settings_value(app: &tauri::AppHandle) -> serde_json::Value {
    let Some(path) = settings_path(app) else {
        return default_settings_value();
    };
    let Ok(contents) = std::fs::read_to_string(path) else {
        return default_settings_value();
    };
    serde_json::from_str(&contents).unwrap_or_else(|_| default_settings_value())
}

fn write_settings_value(app: &tauri::AppHandle, settings: &serde_json::Value) -> Result<(), String> {
    let Some(path) = settings_path(app) else {
        return Err("Could not resolve settings path.".to_string());
    };
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let contents = serde_json::to_string_pretty(settings).map_err(|e| e.to_string())?;
    std::fs::write(path, contents).map_err(|e| e.to_string())
}

fn set_saved_floating_button_visible(
    app: &tauri::AppHandle,
    visible: bool,
) -> Result<(), String> {
    let mut settings = read_settings_value(app);
    if !settings.is_object() {
        settings = default_settings_value();
    }
    if settings.get("floatingButton").is_none() || !settings["floatingButton"].is_object() {
        settings["floatingButton"] = serde_json::json!({});
    }
    settings["floatingButton"]["visible"] = serde_json::Value::Bool(visible);
    write_settings_value(app, &settings)
}

fn startup_prompt_button_position(app: &tauri::AppHandle) -> (f64, f64) {
    let fallback = (960.0, 700.0);
    let Ok(app_data_dir) = app.path().app_data_dir() else {
        return fallback;
    };
    let settings_path = app_data_dir.join("settings.json");
    let Ok(contents) = std::fs::read_to_string(settings_path) else {
        return fallback;
    };
    parse_saved_button_position(&contents).unwrap_or(fallback)
}

fn setup_menu_bar_app(app_handle: &tauri::AppHandle) -> Result<(), String> {
    let open_main = MenuItem::with_id(
        app_handle,
        TRAY_OPEN_MAIN_ID,
        "Manage Prompts...",
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
    let open_accessibility = MenuItem::with_id(
        app_handle,
        TRAY_OPEN_ACCESSIBILITY_ID,
        "Open Accessibility Settings",
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
        &[
            &open_main,
            &show_button,
            &hide_button,
            &open_accessibility,
            &separator,
            &quit,
        ],
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
                let _ = set_saved_floating_button_visible(app, true);
                let position = prompt_button_position_cmd(app.clone()).ok().flatten();
                let (x, y) = position
                    .map(|point| (point.x, point.y))
                    .unwrap_or_else(|| startup_prompt_button_position(app));
                let _ = show_prompt_button(x, y, app.clone());
            }
            TrayMenuAction::HideFloatingButton => {
                let _ = set_saved_floating_button_visible(app, false);
                let _ = hide_prompt_popover(app.clone());
                let _ = hide_prompt_button(app.clone());
            }
            TrayMenuAction::OpenAccessibilitySettings => {
                let _ = platform::macos::open_accessibility_settings();
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
        .manage(PromptPickSessionState::default())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_shell::init())
        .invoke_handler(tauri::generate_handler![
            accessibility_status_cmd,
            request_accessibility_permission_cmd,
            open_accessibility_settings,
            frontmost_app_cmd,
            current_input_target,
            begin_prompt_pick_session,
            paste_prompt,
            paste_prompt_to_app,
            paste_prompt_to_last_target,
            paste_prompt_and_submit_to_last_target,
            paste_prompt_sequence_and_submit_to_last_target,
            show_prompt_button,
            hide_prompt_button,
            show_prompt_popover,
            hide_prompt_popover,
            show_prompt_popover_from_button,
            show_prompt_button_controls_from_button,
            prompt_button_position_cmd,
            move_prompt_button_to,
            open_main_window,
            quit_prompt_picker
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
            let (x, y) = startup_prompt_button_position(app.handle());
            let _ = show_prompt_button(x, y, app.handle().clone());
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod last_input_target_tests {
    use super::*;
    use crate::platform::macos::AutosendFailureReason;

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
    fn parses_saved_startup_button_position() {
        let settings = r#"{
            "version": 1,
            "overlayPlacement": {
                "buttonOffset": null,
                "buttonPosition": { "x": 1765, "y": 419 }
            },
            "floatingButton": { "visible": true }
        }"#;

        assert_eq!(parse_saved_button_position(settings), Some((1765.0, 419.0)));
    }

    #[test]
    fn ignores_invalid_saved_startup_button_position() {
        let settings = r#"{
            "version": 1,
            "overlayPlacement": {
                "buttonPosition": { "x": "bad", "y": 419 }
            }
        }"#;

        assert_eq!(parse_saved_button_position(settings), None);
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
    fn records_app_only_target_without_focused_input_frame() {
        let state = LastInputTargetState::default();
        let target = platform::InputTarget {
            frame: CandidateInput {
                x: 0.0,
                y: 0.0,
                width: 0.0,
                height: 0.0,
            },
            window_frame: CandidateInput {
                x: 0.0,
                y: 0.0,
                width: 100.0,
                height: 100.0,
            },
            button_position: (90.0, 90.0),
            click_point: (50.0, 50.0),
            app: Some(FrontmostApp {
                name: "WeChat".to_string(),
                bundle_id: "com.tencent.xinWeChat".to_string(),
            }),
        };

        record_last_input_target_if_valid(&state, &target);

        let stored = state.get().unwrap();
        assert_eq!(stored.app.bundle_id, "com.tencent.xinWeChat");
        assert!(stored.click_point.is_none());
    }

    #[test]
    fn clears_unsafe_finder_target_without_focused_input_frame() {
        let state = LastInputTargetState::default();
        state.set(LastInputTarget {
            app: FrontmostApp {
                name: "Codex".to_string(),
                bundle_id: "com.openai.codex".to_string(),
            },
            observed_at_ms: 123,
            click_point: Some(TargetClickPoint { x: 640.0, y: 720.0 }),
        });
        let target = platform::InputTarget {
            frame: CandidateInput {
                x: 0.0,
                y: 0.0,
                width: 0.0,
                height: 0.0,
            },
            window_frame: CandidateInput {
                x: 0.0,
                y: 0.0,
                width: 100.0,
                height: 100.0,
            },
            button_position: (90.0, 90.0),
            click_point: (50.0, 50.0),
            app: Some(FrontmostApp {
                name: "Finder".to_string(),
                bundle_id: "com.apple.finder".to_string(),
            }),
        };

        record_last_input_target_if_valid(&state, &target);

        assert!(state.get().is_none());
    }

    #[test]
    fn records_codex_fallback_target_without_focused_input_frame() {
        let state = LastInputTargetState::default();
        let target = platform::InputTarget {
            frame: CandidateInput {
                x: 0.0,
                y: 0.0,
                width: 0.0,
                height: 0.0,
            },
            window_frame: CandidateInput {
                x: 100.0,
                y: 200.0,
                width: 800.0,
                height: 600.0,
            },
            button_position: (876.0, 776.0),
            click_point: (500.0, 735.0),
            app: Some(FrontmostApp {
                name: "Codex".to_string(),
                bundle_id: "com.openai.codex".to_string(),
            }),
        };

        record_last_input_target_if_valid(&state, &target);

        assert_eq!(state.get().unwrap().app.bundle_id, "com.openai.codex");
        assert_eq!(state.get().unwrap().click_point.unwrap().y, 735.0);
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
    fn prompt_pick_session_uses_frontmost_business_app() {
        let target = prompt_pick_session_target(
            Some(FrontmostApp {
                name: "WeChat".to_string(),
                bundle_id: "com.tencent.xinWeChat".to_string(),
            }),
            vec![],
            None,
        )
        .unwrap();

        assert_eq!(target.app.bundle_id, "com.tencent.xinWeChat");
    }

    #[test]
    fn prompt_pick_session_falls_back_from_prompt_picker_to_visible_business_app() {
        let target = prompt_pick_session_target(
            Some(FrontmostApp {
                name: "Prompt Picker".to_string(),
                bundle_id: "local.promptpicker.dev".to_string(),
            }),
            vec![FrontmostApp {
                name: "Codex".to_string(),
                bundle_id: "com.openai.codex".to_string(),
            }],
            None,
        )
        .unwrap();

        assert_eq!(target.app.bundle_id, "com.openai.codex");
    }

    #[test]
    fn prompt_pick_session_does_not_skip_unsafe_visible_app_to_stale_recent_target() {
        let target = prompt_pick_session_target(
            Some(FrontmostApp {
                name: "Prompt Picker".to_string(),
                bundle_id: "local.promptpicker.dev".to_string(),
            }),
            vec![
                FrontmostApp {
                    name: "Finder".to_string(),
                    bundle_id: "com.apple.finder".to_string(),
                },
                FrontmostApp {
                    name: "Codex".to_string(),
                    bundle_id: "com.openai.codex".to_string(),
                },
            ],
            Some(LastInputTarget {
                app: FrontmostApp {
                    name: "Codex".to_string(),
                    bundle_id: "com.openai.codex".to_string(),
                },
                observed_at_ms: 123,
                click_point: None,
            }),
        );

        assert!(target.is_none());
    }

    #[test]
    fn prompt_pick_session_uses_recent_target_when_prompt_picker_has_no_visible_app() {
        let target = prompt_pick_session_target(
            Some(FrontmostApp {
                name: "Prompt Picker".to_string(),
                bundle_id: "local.promptpicker.dev".to_string(),
            }),
            vec![],
            Some(LastInputTarget {
                app: FrontmostApp {
                    name: "WeChat".to_string(),
                    bundle_id: "com.tencent.xinWeChat".to_string(),
                },
                observed_at_ms: now_ms(),
                click_point: None,
            }),
        )
        .unwrap();

        assert_eq!(target.app.bundle_id, "com.tencent.xinWeChat");
    }

    #[test]
    fn prompt_pick_session_prefers_recent_target_over_visible_app_when_picker_is_frontmost() {
        let target = prompt_pick_session_target(
            Some(FrontmostApp {
                name: "Prompt Picker".to_string(),
                bundle_id: "local.promptpicker.dev".to_string(),
            }),
            vec![FrontmostApp {
                name: "Codex".to_string(),
                bundle_id: "com.openai.codex".to_string(),
            }],
            Some(LastInputTarget {
                app: FrontmostApp {
                    name: "WeChat".to_string(),
                    bundle_id: "com.tencent.xinWeChat".to_string(),
                },
                observed_at_ms: now_ms(),
                click_point: Some(TargetClickPoint { x: 400.0, y: 700.0 }),
            }),
        )
        .unwrap();

        assert_eq!(target.app.bundle_id, "com.tencent.xinWeChat");
        assert_eq!(target.click_point.unwrap().x, 400.0);
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
    fn autosend_without_last_target_copies_without_sending() {
        let state = PromptPickSessionState::default();
        let result = paste_prompt_and_submit_to_session_target_with_senders(
            "hello",
            &state,
            |_, _, _| panic!("app sender must not run without a target"),
            |body| {
                assert_eq!(body, "hello");
                Ok(())
            },
        );

        let outcome = result.unwrap();
        assert!(outcome.copied);
        assert!(!outcome.sent);
        assert_eq!(outcome.reason, Some(AutosendFailureReason::NoSafeTarget));
    }

    #[test]
    fn autosend_session_target_uses_app_sender_with_click_point() {
        let state = PromptPickSessionState::default();
        state.set(PromptPickSessionTarget {
            app: FrontmostApp {
                name: "WeChat".to_string(),
                bundle_id: "com.tencent.xinWeChat".to_string(),
            },
            observed_at_ms: 123,
            click_point: Some(TargetClickPoint { x: 420.0, y: 720.0 }),
        });

        let result = paste_prompt_and_submit_to_session_target_with_senders(
            "hello",
            &state,
            |body, bundle_id, click_point| {
                assert_eq!(body, "hello");
                assert_eq!(bundle_id, "com.tencent.xinWeChat");
                assert_eq!(click_point.unwrap().y, 720.0);
                AutosendOutcome::sent()
            },
            |_| panic!("copy sender must not run for a safe app-only target"),
        );

        let outcome = result.unwrap();
        assert!(outcome.copied);
        assert!(outcome.sent);
        assert!(state.get().is_none());
    }

    #[test]
    fn autosend_session_target_does_not_use_click_point() {
        let state = PromptPickSessionState::default();
        state.set(PromptPickSessionTarget {
            app: FrontmostApp {
                name: "Codex".to_string(),
                bundle_id: "com.openai.codex".to_string(),
            },
            observed_at_ms: 123,
            click_point: None,
        });

        let result = paste_prompt_and_submit_to_session_target_with_senders(
            "hello",
            &state,
            |body, bundle_id, click_point| {
                assert_eq!(body, "hello");
                assert_eq!(bundle_id, "com.openai.codex");
                assert!(click_point.is_none());
                AutosendOutcome::sent()
            },
            |_| panic!("copy sender must not run when a click point exists"),
        );

        let outcome = result.unwrap();
        assert!(outcome.copied);
        assert!(outcome.sent);
    }

    #[test]
    fn autosend_reports_app_sender_failure_without_returning_err() {
        let state = PromptPickSessionState::default();
        state.set(PromptPickSessionTarget {
            app: FrontmostApp {
                name: "Codex".to_string(),
                bundle_id: "com.openai.codex".to_string(),
            },
            observed_at_ms: 123,
            click_point: None,
        });

        let result = paste_prompt_and_submit_to_session_target_with_senders(
            "hello",
            &state,
            |_, _, _| AutosendOutcome::paste_event_failed("app paste failed".to_string()),
            |_| panic!("copy sender must not run when a click point exists"),
        );

        let outcome = result.unwrap();
        assert!(outcome.copied);
        assert!(!outcome.sent);
        assert_eq!(outcome.error.as_deref(), Some("app paste failed"));
        assert_eq!(
            outcome.reason,
            Some(AutosendFailureReason::PasteEventFailed)
        );
    }

    #[test]
    fn autosend_sequence_uses_one_session_target_for_all_bodies() {
        let state = PromptPickSessionState::default();
        state.set(PromptPickSessionTarget {
            app: FrontmostApp {
                name: "WeChat".to_string(),
                bundle_id: "com.tencent.xinWeChat".to_string(),
            },
            observed_at_ms: 123,
            click_point: None,
        });
        let bodies = vec!["one".to_string(), "two".to_string(), "three".to_string()];
        let mut sent: Vec<(String, String)> = Vec::new();
        let mut sleeps = Vec::new();

        let result = paste_prompt_sequence_and_submit_to_session_target_with_senders(
            &bodies,
            700,
            &state,
            |body, bundle_id, _| {
                sent.push((body.to_string(), bundle_id.to_string()));
                AutosendOutcome::sent()
            },
            |_| panic!("copy sender must not run when target exists"),
            |delay_ms| sleeps.push(delay_ms),
        )
        .unwrap();

        assert!(result.sent);
        assert_eq!(result.sent_count, 3);
        assert!(state.get().is_none());
        assert_eq!(
            sent,
            vec![
                ("one".to_string(), "com.tencent.xinWeChat".to_string()),
                ("two".to_string(), "com.tencent.xinWeChat".to_string()),
                ("three".to_string(), "com.tencent.xinWeChat".to_string()),
            ]
        );
        assert_eq!(sleeps, vec![700, 700]);
    }

    #[test]
    fn autosend_sequence_clamps_interval_to_milliseconds_range() {
        let state = PromptPickSessionState::default();
        state.set(PromptPickSessionTarget {
            app: FrontmostApp {
                name: "Codex".to_string(),
                bundle_id: "com.openai.codex".to_string(),
            },
            observed_at_ms: 123,
            click_point: None,
        });
        let bodies = vec!["one".to_string(), "two".to_string()];
        let mut sleeps = Vec::new();

        paste_prompt_sequence_and_submit_to_session_target_with_senders(
            &bodies,
            10,
            &state,
            |_, _, _| AutosendOutcome::sent(),
            |_| panic!("copy sender must not run when target exists"),
            |delay_ms| sleeps.push(delay_ms),
        )
        .unwrap();

        assert_eq!(sleeps, vec![MIN_SEQUENCE_INTERVAL_MS]);
    }

    #[test]
    fn autosend_sequence_stops_at_first_failed_body() {
        let state = PromptPickSessionState::default();
        state.set(PromptPickSessionTarget {
            app: FrontmostApp {
                name: "Codex".to_string(),
                bundle_id: "com.openai.codex".to_string(),
            },
            observed_at_ms: 123,
            click_point: None,
        });
        let bodies = vec!["one".to_string(), "two".to_string(), "three".to_string()];
        let mut sent = Vec::new();
        let mut sleeps = Vec::new();

        let result = paste_prompt_sequence_and_submit_to_session_target_with_senders(
            &bodies,
            700,
            &state,
            |body, _, _| {
                sent.push(body.to_string());
                if body == "two" {
                    return AutosendOutcome::paste_event_failed("paste failed".to_string());
                }
                AutosendOutcome::sent()
            },
            |_| panic!("copy sender must not run when target exists"),
            |delay_ms| sleeps.push(delay_ms),
        )
        .unwrap();

        assert!(!result.sent);
        assert_eq!(result.sent_count, 1);
        assert_eq!(result.failed_index, Some(2));
        assert_eq!(sent, vec!["one".to_string(), "two".to_string()]);
        assert_eq!(sleeps, vec![700]);
    }

    #[test]
    fn autosend_sequence_without_target_copies_first_body_without_sending() {
        let state = PromptPickSessionState::default();
        let bodies = vec!["one".to_string(), "two".to_string()];
        let mut copied = String::new();

        let result = paste_prompt_sequence_and_submit_to_session_target_with_senders(
            &bodies,
            700,
            &state,
            |_, _, _| panic!("app sender must not run without target"),
            |body| {
                copied = body.to_string();
                Ok(())
            },
            |_| panic!("sleeper must not run without target"),
        )
        .unwrap();

        assert_eq!(copied, "one");
        assert!(!result.sent);
        assert_eq!(result.sent_count, 0);
        assert_eq!(result.failed_index, Some(1));
        assert_eq!(result.reason, Some(AutosendFailureReason::NoSafeTarget));
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
        assert_eq!(
            tray_menu_action(TRAY_OPEN_ACCESSIBILITY_ID),
            TrayMenuAction::OpenAccessibilitySettings
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
