use tauri::{
    image::Image,
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::TrayIconBuilder,
    Emitter, Manager, WindowEvent,
};
use tauri_plugin_clipboard_manager::ClipboardExt;

mod platform;
pub use platform::{
    accessibility_status, frontmost_app, request_accessibility_permission, AccessibilityStatus,
    AutosendOutcome, CandidateInput, FrontmostApp, FrontmostAppWithPid,
};
mod overlay_position;
pub use overlay_position::{prompt_button_position, OverlayPoint};
mod windows;
pub use windows::{
    hide_prompt_button, hide_prompt_popover, move_prompt_button_to, prompt_button_position_cmd,
    show_prompt_button, show_prompt_button_controls_from_button, show_prompt_popover,
    show_prompt_popover_from_button, toggle_prompt_popover_from_button,
};
mod macos_panels;
pub use macos_panels::{activate_main_window, configure_non_activating_panel};
mod prompt_files;
pub use prompt_files::{
    prompt_library_file_metadata, read_prompt_library_file, write_prompt_library_file,
};

#[tauri::command]
fn accessibility_status_cmd() -> AccessibilityStatus {
    accessibility_status()
}

#[tauri::command]
fn request_accessibility_permission_cmd() -> AccessibilityStatus {
    request_accessibility_permission()
}

#[derive(Clone, Debug, serde::Serialize)]
struct PromptInteractionPermissionStatus {
    required: bool,
    trusted: bool,
    native_prompt_requested: bool,
    language: String,
}

fn prompt_interaction_permission_status_from_parts(
    required: bool,
    trusted: bool,
    native_prompt_requested: bool,
    language: String,
) -> PromptInteractionPermissionStatus {
    PromptInteractionPermissionStatus {
        required,
        trusted: if required { trusted } else { true },
        native_prompt_requested,
        language,
    }
}

#[tauri::command]
fn prompt_interaction_permission_status(
    app: tauri::AppHandle,
) -> PromptInteractionPermissionStatus {
    let settings = read_settings_value(&app);
    prompt_interaction_permission_status_from_parts(
        cfg!(target_os = "macos"),
        accessibility_status().trusted,
        accessibility_prompt_requested(&settings),
        settings_language(&settings).to_string(),
    )
}

#[tauri::command]
fn request_prompt_interaction_permission(app: tauri::AppHandle) -> AccessibilityStatus {
    let _ = set_accessibility_prompt_requested(&app, true);
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

    if let Some(app) = platform::frontmost_app_with_pid() {
        record_last_app_if_valid(state.inner(), app);
    }

    None
}

#[tauri::command]
async fn begin_prompt_pick_session(
    session_id: u64,
    session_state: tauri::State<'_, PromptPickSessionState>,
    recent_state: tauri::State<'_, LastInputTargetState>,
) -> Result<Option<FrontmostApp>, String> {
    let session_state = session_state.inner().clone();
    let recent_state = recent_state.inner().clone();

    tauri::async_runtime::spawn_blocking(move || {
        if let Some(input_target) = platform::macos::current_input_target() {
            record_last_input_target_if_valid(&recent_state, &input_target);
        }

        let Some(target) = prompt_pick_session_target(
            platform::frontmost_app_with_pid(),
            platform::macos::visible_apps(),
            recent_state.get(),
        ) else {
            session_state.clear_if_current(session_id);
            return None;
        };
        record_prompt_pick_session_target_if_valid(&session_state, target, session_id)
    })
    .await
    .map_err(|error| format!("Prompt pick session task failed: {}", error))
}

#[tauri::command]
fn paste_prompt(body: String, app: tauri::AppHandle) -> Result<(), String> {
    platform::macos::paste_prompt_with_copier(&body, |text| copy_text_to_clipboard(&app, text))
}

#[tauri::command]
fn paste_prompt_to_app(
    body: String,
    bundle_id: String,
    app: tauri::AppHandle,
) -> Result<(), String> {
    platform::macos::paste_prompt_to_app_with_copier(&body, &bundle_id, |text| {
        copy_text_to_clipboard(&app, text)
    })
}

#[tauri::command]
async fn paste_prompt_to_last_target(
    body: String,
    state: tauri::State<'_, LastInputTargetState>,
    app: tauri::AppHandle,
) -> Result<(), String> {
    let state = state.inner().clone();
    let app = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        paste_prompt_to_last_target_impl(&body, &state, |text| copy_text_to_clipboard(&app, text))
    })
    .await
    .map_err(|error| format!("Paste task failed: {}", error))?
}

fn paste_prompt_to_last_target_impl<C>(
    body: &str,
    state: &LastInputTargetState,
    copy_sender: C,
) -> Result<(), String>
where
    C: FnOnce(&str) -> Result<(), String>,
{
    let Some(target) = state.get() else {
        return Err("Click into a text field first, then choose a prompt.".to_string());
    };
    platform::macos::paste_prompt_to_app_with_copier(body, &target.app.bundle_id, copy_sender)
}

#[tauri::command]
async fn paste_prompt_and_submit_to_last_target(
    body: String,
    submit_key: Option<String>,
    session_state: tauri::State<'_, PromptPickSessionState>,
    recent_state: tauri::State<'_, LastInputTargetState>,
    app: tauri::AppHandle,
) -> Result<AutosendOutcome, String> {
    let submit_key = native_submit_key_from_arg(submit_key)?;
    let session_state = session_state.inner().clone();
    let recent_state = recent_state.inner().clone();
    let app = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        paste_prompt_and_submit_to_last_target_impl(
            &body,
            &session_state,
            &recent_state,
            &app,
            submit_key,
        )
    })
    .await
    .map_err(|error| format!("Autosend task failed: {}", error))?
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
async fn paste_prompt_sequence_and_submit_to_last_target(
    bodies: Vec<String>,
    interval_ms: u64,
    submit_key: Option<String>,
    session_state: tauri::State<'_, PromptPickSessionState>,
    recent_state: tauri::State<'_, LastInputTargetState>,
    app: tauri::AppHandle,
) -> Result<AutosendSequenceOutcome, String> {
    let submit_key = native_submit_key_from_arg(submit_key)?;
    let session_state = session_state.inner().clone();
    let recent_state = recent_state.inner().clone();
    let app = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        paste_prompt_sequence_and_submit_to_last_target_impl(
            &bodies,
            interval_ms,
            &session_state,
            &recent_state,
            &app,
            submit_key,
        )
    })
    .await
    .map_err(|error| format!("Autosend sequence task failed: {}", error))?
}

fn paste_prompt_sequence_and_submit_to_last_target_impl(
    bodies: &[String],
    interval_ms: u64,
    state: &PromptPickSessionState,
    recent_state: &LastInputTargetState,
    app: &tauri::AppHandle,
    submit_key: platform::macos::NativeSubmitKey,
) -> Result<AutosendSequenceOutcome, String> {
    focus_preserving_prompt_sequence_to_last_target_impl(
        bodies,
        interval_ms,
        state,
        Some(recent_state),
        submit_key,
        |text| copy_text_to_clipboard(app, text),
        platform::frontmost_app_with_pid,
        platform::macos::post_focus_preserving_paste,
        platform::macos::post_focus_preserving_submit_key,
        |delay_ms| std::thread::sleep(std::time::Duration::from_millis(delay_ms)),
    )
}

fn paste_prompt_and_submit_to_last_target_impl(
    body: &str,
    state: &PromptPickSessionState,
    recent_state: &LastInputTargetState,
    app: &tauri::AppHandle,
    submit_key: platform::macos::NativeSubmitKey,
) -> Result<AutosendOutcome, String> {
    focus_preserving_prompt_to_last_target_impl(
        body,
        state,
        Some(recent_state),
        submit_key,
        |text| copy_text_to_clipboard(app, text),
        platform::frontmost_app_with_pid,
        platform::macos::post_focus_preserving_paste,
        platform::macos::post_focus_preserving_submit_key,
        |delay_ms| std::thread::sleep(std::time::Duration::from_millis(delay_ms)),
    )
}

fn copy_text_to_clipboard(app: &tauri::AppHandle, body: &str) -> Result<(), String> {
    app.clipboard()
        .write_text(body)
        .map_err(|error| format!("Clipboard write failed: {}", error))
}

const MIN_SEQUENCE_INTERVAL_MS: u64 = 200;
const MAX_SEQUENCE_INTERVAL_MS: u64 = 3_000;

fn clamp_sequence_interval_ms(interval_ms: u64) -> u64 {
    interval_ms.clamp(MIN_SEQUENCE_INTERVAL_MS, MAX_SEQUENCE_INTERVAL_MS)
}

const FOCUS_PRESERVING_PASTE_SETTLE_MS: u64 = 180;

fn native_submit_key_from_arg(
    submit_key: Option<String>,
) -> Result<platform::macos::NativeSubmitKey, String> {
    match submit_key.as_deref().unwrap_or("enter") {
        "none" => Ok(platform::macos::NativeSubmitKey::None),
        "enter" => Ok(platform::macos::NativeSubmitKey::Enter),
        "command_enter" => Ok(platform::macos::NativeSubmitKey::CommandEnter),
        value => Err(format!("Invalid submit key: {}", value)),
    }
}

fn prompt_pick_target_or_recent(
    session_state: &PromptPickSessionState,
    recent_state: Option<&LastInputTargetState>,
) -> Option<PromptPickSessionTarget> {
    if let Some(target) = session_state.take() {
        return Some(target);
    }

    recent_state
        .and_then(LastInputTargetState::get)
        .filter(is_recent_prompt_target)
        .filter(|target| is_usable_autosend_app(&target.app))
        .map(|target| PromptPickSessionTarget {
            app: target.app,
            pid: target.pid,
            observed_at_ms: now_ms(),
            click_point: target.click_point,
        })
}

fn focus_preserving_prompt_to_last_target_impl<C, F, P, S, W>(
    body: &str,
    state: &PromptPickSessionState,
    recent_state: Option<&LastInputTargetState>,
    submit_key: platform::macos::NativeSubmitKey,
    copy_sender: C,
    frontmost_reader: F,
    paste_sender: P,
    submit_sender: S,
    sleeper: W,
) -> Result<AutosendOutcome, String>
where
    C: FnOnce(&str) -> Result<(), String>,
    F: FnMut() -> Option<FrontmostAppWithPid>,
    P: FnOnce() -> Result<(), String>,
    S: FnOnce(platform::macos::NativeSubmitKey) -> Result<(), String>,
    W: FnOnce(u64),
{
    let Some(target) = prompt_pick_target_or_recent(state, recent_state) else {
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

    if !platform::macos::accessibility_status().trusted {
        return Ok(AutosendOutcome::missing_accessibility_permission());
    }

    Ok(guarded_focus_preserving_autosend_with_senders(
        body,
        &target,
        submit_key,
        copy_sender,
        frontmost_reader,
        paste_sender,
        submit_sender,
        sleeper,
    ))
}

fn focus_preserving_prompt_sequence_to_last_target_impl<C, F, P, S, W>(
    bodies: &[String],
    interval_ms: u64,
    state: &PromptPickSessionState,
    recent_state: Option<&LastInputTargetState>,
    submit_key: platform::macos::NativeSubmitKey,
    copy_sender: C,
    frontmost_reader: F,
    paste_sender: P,
    submit_sender: S,
    sleeper: W,
) -> Result<AutosendSequenceOutcome, String>
where
    C: FnMut(&str) -> Result<(), String>,
    F: FnMut() -> Option<FrontmostAppWithPid>,
    P: FnMut() -> Result<(), String>,
    S: FnMut(platform::macos::NativeSubmitKey) -> Result<(), String>,
    W: FnMut(u64),
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

    let Some(target) = prompt_pick_target_or_recent(state, recent_state) else {
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

    if !platform::macos::accessibility_status().trusted {
        return Ok(AutosendSequenceOutcome::from_failure(
            AutosendOutcome::missing_accessibility_permission(),
            0,
            1,
        ));
    }

    Ok(focus_preserving_prompt_sequence_for_target_with_senders(
        &clean_bodies,
        interval_ms,
        &target,
        submit_key,
        copy_sender,
        frontmost_reader,
        paste_sender,
        submit_sender,
        sleeper,
    ))
}

fn focus_preserving_prompt_sequence_for_target_with_senders<C, F, P, S, W>(
    clean_bodies: &[String],
    interval_ms: u64,
    target: &PromptPickSessionTarget,
    submit_key: platform::macos::NativeSubmitKey,
    mut copy_sender: C,
    mut frontmost_reader: F,
    mut paste_sender: P,
    mut submit_sender: S,
    mut sleeper: W,
) -> AutosendSequenceOutcome
where
    C: FnMut(&str) -> Result<(), String>,
    F: FnMut() -> Option<FrontmostAppWithPid>,
    P: FnMut() -> Result<(), String>,
    S: FnMut(platform::macos::NativeSubmitKey) -> Result<(), String>,
    W: FnMut(u64),
{
    let delay_ms = clamp_sequence_interval_ms(interval_ms);
    for (index, body) in clean_bodies.iter().enumerate() {
        let outcome = guarded_focus_preserving_autosend_with_senders(
            body,
            target,
            submit_key,
            |text| copy_sender(text),
            || frontmost_reader(),
            || paste_sender(),
            |key| submit_sender(key),
            |delay_ms| sleeper(delay_ms),
        );
        if !outcome.sent {
            return AutosendSequenceOutcome::from_failure(outcome, index, index + 1);
        }
        if index + 1 < clean_bodies.len() {
            sleeper(delay_ms);
        }
    }

    AutosendSequenceOutcome::sent_all(clean_bodies.len())
}

fn guarded_focus_preserving_autosend_with_senders<C, F, P, S, W>(
    body: &str,
    target: &PromptPickSessionTarget,
    submit_key: platform::macos::NativeSubmitKey,
    copy_sender: C,
    mut frontmost_reader: F,
    paste_sender: P,
    submit_sender: S,
    sleeper: W,
) -> AutosendOutcome
where
    C: FnOnce(&str) -> Result<(), String>,
    F: FnMut() -> Option<FrontmostAppWithPid>,
    P: FnOnce() -> Result<(), String>,
    S: FnOnce(platform::macos::NativeSubmitKey) -> Result<(), String>,
    W: FnOnce(u64),
{
    if let Err(error) = copy_sender(body) {
        return AutosendOutcome::copy_failed(error);
    }

    let before_paste = frontmost_reader();
    if !captured_target_matches_frontmost(target, before_paste.as_ref()) {
        return AutosendOutcome::copied_without_send(
            "Target app changed before paste; prompt was copied instead.".to_string(),
        );
    }

    if let Err(error) = paste_sender() {
        return AutosendOutcome::paste_event_failed(format!(
            "Focus-preserving paste failed: {}",
            error
        ));
    }

    sleeper(FOCUS_PRESERVING_PASTE_SETTLE_MS);

    if submit_key == platform::macos::NativeSubmitKey::None {
        return AutosendOutcome::sent();
    }

    let before_submit = frontmost_reader();
    if !captured_target_matches_frontmost(target, before_submit.as_ref()) {
        return AutosendOutcome::return_event_failed(
            "Target app changed after paste; submit was skipped.".to_string(),
        );
    }

    if let Err(error) = submit_sender(submit_key) {
        return AutosendOutcome::return_event_failed(format!(
            "Focus-preserving submit failed: {}",
            error
        ));
    }

    AutosendOutcome::sent()
}

fn paste_prompt_and_submit_to_session_target_with_senders<A, C>(
    body: &str,
    state: &PromptPickSessionState,
    recent_state: Option<&LastInputTargetState>,
    app_sender: A,
    copy_sender: C,
) -> Result<AutosendOutcome, String>
where
    A: FnOnce(&str, &str, Option<TargetClickPoint>) -> AutosendOutcome,
    C: FnOnce(&str) -> Result<(), String>,
{
    let Some(target) = prompt_pick_target_or_recent(state, recent_state) else {
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
    recent_state: Option<&LastInputTargetState>,
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

    let Some(target) = prompt_pick_target_or_recent(state, recent_state) else {
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
            return Ok(AutosendSequenceOutcome::from_failure(
                outcome,
                index,
                index + 1,
            ));
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
    app.emit("open-manager-window", ())
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
fn open_settings_window(app: tauri::AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("main") {
        window.show().map_err(|e| e.to_string())?;
        activate_main_window(&window)?;
        window.set_focus().map_err(|e| e.to_string())?;
    }
    app.emit("open-settings-window", ())
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
fn quit_prompt_picker(app: tauri::AppHandle) {
    app.exit(0);
}

#[tauri::command]
fn set_menu_language(app: tauri::AppHandle, language: String) -> Result<(), String> {
    let menu = build_menu_bar_menu(&app, &language)?;
    let Some(tray) = app.tray_by_id(TRAY_ID) else {
        return Err("Prompt Picker tray icon is not available.".to_string());
    };
    tray.set_menu(Some(menu)).map_err(|e| e.to_string())
}

#[derive(Clone, Debug, serde::Serialize)]
pub struct LastInputTarget {
    pub app: FrontmostApp,
    pub pid: Option<u32>,
    pub observed_at_ms: u128,
    pub click_point: Option<TargetClickPoint>,
}

#[derive(Clone, Copy, Debug, serde::Serialize)]
pub struct TargetClickPoint {
    pub x: f64,
    pub y: f64,
}

#[derive(Clone, Default)]
pub struct LastInputTargetState(std::sync::Arc<std::sync::Mutex<Option<LastInputTarget>>>);

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
    pub pid: Option<u32>,
    pub observed_at_ms: u128,
    pub click_point: Option<TargetClickPoint>,
}

#[derive(Default)]
struct PromptPickSessionInner {
    active_session_id: u64,
    target: Option<PromptPickSessionTarget>,
}

#[derive(Clone, Default)]
pub struct PromptPickSessionState(std::sync::Arc<std::sync::Mutex<PromptPickSessionInner>>);

impl PromptPickSessionState {
    pub fn begin(&self, session_id: u64) {
        let mut state = self.0.lock().expect("prompt pick session lock poisoned");
        state.active_session_id = session_id;
        state.target = None;
    }

    pub fn set(&self, target: PromptPickSessionTarget) {
        self.0
            .lock()
            .expect("prompt pick session lock poisoned")
            .target = Some(target);
    }

    pub fn set_if_current(&self, session_id: u64, target: PromptPickSessionTarget) -> bool {
        let mut state = self.0.lock().expect("prompt pick session lock poisoned");
        if state.active_session_id != session_id {
            return false;
        }
        state.target = Some(target);
        true
    }

    pub fn clear(&self) {
        self.0
            .lock()
            .expect("prompt pick session lock poisoned")
            .target = None;
    }

    pub fn clear_if_current(&self, session_id: u64) -> bool {
        let mut state = self.0.lock().expect("prompt pick session lock poisoned");
        if state.active_session_id != session_id {
            return false;
        }
        state.target = None;
        true
    }

    pub fn get(&self) -> Option<PromptPickSessionTarget> {
        self.0
            .lock()
            .expect("prompt pick session lock poisoned")
            .target
            .clone()
    }

    pub fn take(&self) -> Option<PromptPickSessionTarget> {
        self.0
            .lock()
            .expect("prompt pick session lock poisoned")
            .target
            .take()
    }
}

fn record_prompt_pick_session_target_if_valid(
    state: &PromptPickSessionState,
    target: PromptPickSessionTarget,
    session_id: u64,
) -> Option<FrontmostApp> {
    if !is_usable_autosend_app(&target.app) {
        state.clear_if_current(session_id);
        return None;
    }

    let app = target.app.clone();
    state.set_if_current(session_id, target).then_some(app)
}

fn prompt_pick_session_target(
    frontmost: Option<FrontmostAppWithPid>,
    visible_apps: Vec<FrontmostApp>,
    recent_target: Option<LastInputTarget>,
) -> Option<PromptPickSessionTarget> {
    let frontmost = frontmost?;
    if is_usable_autosend_app(&frontmost.app) {
        let click_point = recent_target
            .as_ref()
            .filter(|target| target.app.bundle_id == frontmost.app.bundle_id)
            .and_then(|target| target.click_point);
        return Some(PromptPickSessionTarget {
            app: frontmost.app,
            pid: frontmost.pid,
            observed_at_ms: now_ms(),
            click_point,
        });
    }

    if !is_prompt_picker_app(&frontmost.app) {
        return None;
    }

    if let Some(target) = recent_target
        .as_ref()
        .filter(|target| is_recent_prompt_target(target))
        .filter(|target| is_usable_autosend_app(&target.app))
    {
        return Some(PromptPickSessionTarget {
            app: target.app.clone(),
            pid: target.pid,
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
                pid: None,
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
            pid: target.pid,
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
        pid: None,
        observed_at_ms: now_ms(),
        click_point,
    });
}

fn record_last_app_if_valid(state: &LastInputTargetState, target: FrontmostAppWithPid) {
    if is_prompt_picker_app(&target.app) {
        return;
    }
    if is_unsafe_autosend_target(&target.app) {
        state.clear();
        return;
    }
    if !allows_app_only_autosend(&target.app) {
        state.clear();
        return;
    }
    state.set(LastInputTarget {
        app: target.app,
        pid: target.pid,
        observed_at_ms: now_ms(),
        click_point: None,
    });
}

fn captured_target_matches_frontmost(
    target: &PromptPickSessionTarget,
    frontmost: Option<&FrontmostAppWithPid>,
) -> bool {
    let Some(frontmost) = frontmost else {
        return false;
    };
    if frontmost.app.bundle_id != target.app.bundle_id {
        return false;
    }
    match (target.pid, frontmost.pid) {
        (Some(target_pid), Some(frontmost_pid)) => target_pid == frontmost_pid,
        _ => true,
    }
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
const TRAY_OPEN_SETTINGS_ID: &str = "open-settings-window";
const TRAY_QUIT_ID: &str = "quit";
const MENUBAR_TEMPLATE_ICON: &[u8] = include_bytes!("../icons/menubar-template.rgba");
const MENUBAR_TEMPLATE_ICON_SIZE: u32 = 22;

#[derive(Debug, PartialEq, Eq)]
struct MenuLabels {
    open_main: &'static str,
    open_settings: &'static str,
    show_button: &'static str,
    hide_button: &'static str,
    open_accessibility: &'static str,
    quit: &'static str,
}

fn menu_labels_for_language(language: &str) -> MenuLabels {
    match language {
        "zh-CN" => MenuLabels {
            open_main: "管理提示词...",
            open_settings: "设置...",
            show_button: "显示 Calico",
            hide_button: "隐藏 Calico",
            open_accessibility: "打开辅助功能设置",
            quit: "退出 Prompt Picker",
        },
        _ => MenuLabels {
            open_main: "Manage Prompts...",
            open_settings: "Settings...",
            show_button: "Show Calico",
            hide_button: "Hide Calico",
            open_accessibility: "Open Accessibility Settings",
            quit: "Quit Prompt Picker",
        },
    }
}

#[derive(Debug, PartialEq, Eq)]
enum TrayMenuAction {
    OpenMainWindow,
    ShowFloatingButton,
    HideFloatingButton,
    OpenAccessibilitySettings,
    OpenSettingsWindow,
    Quit,
    Unknown,
}

fn tray_menu_action(id: &str) -> TrayMenuAction {
    match id {
        TRAY_OPEN_MAIN_ID => TrayMenuAction::OpenMainWindow,
        TRAY_SHOW_BUTTON_ID => TrayMenuAction::ShowFloatingButton,
        TRAY_HIDE_BUTTON_ID => TrayMenuAction::HideFloatingButton,
        TRAY_OPEN_ACCESSIBILITY_ID => TrayMenuAction::OpenAccessibilitySettings,
        TRAY_OPEN_SETTINGS_ID => TrayMenuAction::OpenSettingsWindow,
        TRAY_QUIT_ID => TrayMenuAction::Quit,
        _ => TrayMenuAction::Unknown,
    }
}

fn menubar_template_icon() -> Image<'static> {
    Image::new(
        MENUBAR_TEMPLATE_ICON,
        MENUBAR_TEMPLATE_ICON_SIZE,
        MENUBAR_TEMPLATE_ICON_SIZE,
    )
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
        },
        "promptInsertion": {
            "mode": "paste_and_submit"
        },
        "permissions": {
            "accessibilityPromptRequested": false
        },
        "language": "zh-CN"
    })
}

fn settings_path(app: &tauri::AppHandle) -> Option<std::path::PathBuf> {
    app.path()
        .app_data_dir()
        .ok()
        .map(|dir| dir.join("settings.json"))
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

fn settings_language(settings: &serde_json::Value) -> &str {
    match settings.get("language").and_then(serde_json::Value::as_str) {
        Some("en-US") => "en-US",
        _ => "zh-CN",
    }
}

fn accessibility_prompt_requested(settings: &serde_json::Value) -> bool {
    settings
        .pointer("/permissions/accessibilityPromptRequested")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false)
}

fn write_settings_value(
    app: &tauri::AppHandle,
    settings: &serde_json::Value,
) -> Result<(), String> {
    let Some(path) = settings_path(app) else {
        return Err("Could not resolve settings path.".to_string());
    };
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let contents = serde_json::to_string_pretty(settings).map_err(|e| e.to_string())?;
    std::fs::write(path, contents).map_err(|e| e.to_string())
}

fn set_saved_floating_button_visible(app: &tauri::AppHandle, visible: bool) -> Result<(), String> {
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

fn set_accessibility_prompt_requested(
    app: &tauri::AppHandle,
    requested: bool,
) -> Result<(), String> {
    let mut settings = read_settings_value(app);
    if !settings.is_object() {
        settings = default_settings_value();
    }
    if settings.get("permissions").is_none() || !settings["permissions"].is_object() {
        settings["permissions"] = serde_json::json!({});
    }
    settings["permissions"]["accessibilityPromptRequested"] = serde_json::Value::Bool(requested);
    write_settings_value(app, &settings)
}

fn build_menu_bar_menu(
    app_handle: &tauri::AppHandle,
    language: &str,
) -> Result<Menu<tauri::Wry>, String> {
    let labels = menu_labels_for_language(language);
    let open_main = MenuItem::with_id(
        app_handle,
        TRAY_OPEN_MAIN_ID,
        labels.open_main,
        true,
        None::<&str>,
    )
    .map_err(|e| e.to_string())?;
    let open_settings = MenuItem::with_id(
        app_handle,
        TRAY_OPEN_SETTINGS_ID,
        labels.open_settings,
        true,
        None::<&str>,
    )
    .map_err(|e| e.to_string())?;
    let show_button = MenuItem::with_id(
        app_handle,
        TRAY_SHOW_BUTTON_ID,
        labels.show_button,
        true,
        None::<&str>,
    )
    .map_err(|e| e.to_string())?;
    let hide_button = MenuItem::with_id(
        app_handle,
        TRAY_HIDE_BUTTON_ID,
        labels.hide_button,
        true,
        None::<&str>,
    )
    .map_err(|e| e.to_string())?;
    let open_accessibility = MenuItem::with_id(
        app_handle,
        TRAY_OPEN_ACCESSIBILITY_ID,
        labels.open_accessibility,
        true,
        None::<&str>,
    )
    .map_err(|e| e.to_string())?;
    let separator = PredefinedMenuItem::separator(app_handle).map_err(|e| e.to_string())?;
    let quit = MenuItem::with_id(app_handle, TRAY_QUIT_ID, labels.quit, true, None::<&str>)
        .map_err(|e| e.to_string())?;

    Menu::with_items(
        app_handle,
        &[
            &open_main,
            &open_settings,
            &show_button,
            &hide_button,
            &open_accessibility,
            &separator,
            &quit,
        ],
    )
    .map_err(|e| e.to_string())
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
    let menu = build_menu_bar_menu(
        app_handle,
        settings_language(&read_settings_value(app_handle)),
    )?;

    let tray_builder = TrayIconBuilder::with_id(TRAY_ID)
        .menu(&menu)
        .tooltip("Prompt Picker")
        .show_menu_on_left_click(true)
        .icon_as_template(true)
        .icon(menubar_template_icon())
        .on_menu_event(|app, event| match tray_menu_action(event.id().as_ref()) {
            TrayMenuAction::OpenMainWindow => {
                let _ = open_main_window(app.clone());
            }
            TrayMenuAction::OpenSettingsWindow => {
                let _ = open_settings_window(app.clone());
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
            prompt_interaction_permission_status,
            request_prompt_interaction_permission,
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
            toggle_prompt_popover_from_button,
            show_prompt_button_controls_from_button,
            prompt_button_position_cmd,
            move_prompt_button_to,
            open_main_window,
            open_settings_window,
            set_menu_language,
            quit_prompt_picker,
            read_prompt_library_file,
            write_prompt_library_file,
            prompt_library_file_metadata
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
    use std::{cell::RefCell, collections::VecDeque};

    fn frontmost_target(name: &str, bundle_id: &str, pid: Option<u32>) -> FrontmostAppWithPid {
        FrontmostAppWithPid {
            app: FrontmostApp {
                name: name.to_string(),
                bundle_id: bundle_id.to_string(),
            },
            pid,
        }
    }

    fn prompt_target(name: &str, bundle_id: &str, pid: Option<u32>) -> PromptPickSessionTarget {
        PromptPickSessionTarget {
            app: FrontmostApp {
                name: name.to_string(),
                bundle_id: bundle_id.to_string(),
            },
            pid,
            observed_at_ms: now_ms(),
            click_point: None,
        }
    }

    #[test]
    fn stores_and_reads_last_input_target() {
        let state = LastInputTargetState::default();
        let target = LastInputTarget {
            app: FrontmostApp {
                name: "Notes".to_string(),
                bundle_id: "com.apple.Notes".to_string(),
            },
            pid: None,
            observed_at_ms: 123,
            click_point: None,
        };

        state.set(target);

        assert_eq!(state.get().unwrap().app.bundle_id, "com.apple.Notes");
    }

    #[test]
    fn cloned_last_input_target_state_shares_target() {
        let state = LastInputTargetState::default();
        let cloned = state.clone();

        state.set(LastInputTarget {
            app: FrontmostApp {
                name: "Codex".to_string(),
                bundle_id: "com.openai.codex".to_string(),
            },
            pid: None,
            observed_at_ms: now_ms(),
            click_point: Some(TargetClickPoint { x: 12.0, y: 34.0 }),
        });

        assert_eq!(cloned.get().unwrap().app.bundle_id, "com.openai.codex");
    }

    #[test]
    fn cloned_prompt_pick_session_state_shares_target() {
        let state = PromptPickSessionState::default();
        let cloned = state.clone();

        state.begin(7);
        assert!(cloned.set_if_current(
            7,
            PromptPickSessionTarget {
                app: FrontmostApp {
                    name: "Codex".to_string(),
                    bundle_id: "com.openai.codex".to_string(),
                },
                pid: None,
                observed_at_ms: now_ms(),
                click_point: None,
            }
        ));

        assert_eq!(state.take().unwrap().app.bundle_id, "com.openai.codex");
    }

    #[test]
    fn autosend_and_prompt_capture_commands_use_spawn_blocking() {
        let source = include_str!("lib.rs");

        assert!(source.contains("async fn begin_prompt_pick_session"));
        assert!(source.contains("async fn paste_prompt_to_last_target"));
        assert!(source.contains("async fn paste_prompt_and_submit_to_last_target"));
        assert!(source.contains("async fn paste_prompt_sequence_and_submit_to_last_target"));
        assert!(source.matches("tauri::async_runtime::spawn_blocking(move ||").count() >= 4);
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
    fn resolves_menu_labels_by_language() {
        assert_eq!(menu_labels_for_language("zh-CN").open_main, "管理提示词...");
        assert_eq!(menu_labels_for_language("zh-CN").open_settings, "设置...");
        assert_eq!(
            menu_labels_for_language("en-US").open_main,
            "Manage Prompts..."
        );
        assert_eq!(menu_labels_for_language("bad").quit, "Quit Prompt Picker");
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
            pid: None,
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
            Some(frontmost_target("WeChat", "com.tencent.xinWeChat", Some(123))),
            vec![],
            None,
        )
        .unwrap();

        assert_eq!(target.app.bundle_id, "com.tencent.xinWeChat");
        assert_eq!(target.pid, Some(123));
    }

    #[test]
    fn captured_target_matches_only_same_bundle_and_pid() {
        let target = PromptPickSessionTarget {
            app: FrontmostApp {
                name: "WeChat".to_string(),
                bundle_id: "com.tencent.xinWeChat".to_string(),
            },
            pid: Some(123),
            observed_at_ms: now_ms(),
            click_point: None,
        };
        let same = frontmost_target("WeChat", "com.tencent.xinWeChat", Some(123));
        let different_pid = frontmost_target("WeChat", "com.tencent.xinWeChat", Some(456));
        let different_bundle = frontmost_target("Notes", "com.apple.Notes", Some(123));

        assert!(captured_target_matches_frontmost(&target, Some(&same)));
        assert!(!captured_target_matches_frontmost(
            &target,
            Some(&different_pid)
        ));
        assert!(!captured_target_matches_frontmost(
            &target,
            Some(&different_bundle)
        ));
        assert!(!captured_target_matches_frontmost(&target, None));
    }

    #[test]
    fn captured_target_falls_back_to_bundle_when_pid_is_unavailable() {
        let target = PromptPickSessionTarget {
            app: FrontmostApp {
                name: "Notes".to_string(),
                bundle_id: "com.apple.Notes".to_string(),
            },
            pid: None,
            observed_at_ms: now_ms(),
            click_point: None,
        };
        let frontmost = frontmost_target("Notes", "com.apple.Notes", Some(123));

        assert!(captured_target_matches_frontmost(&target, Some(&frontmost)));
    }

    #[test]
    fn send_behavior_submit_key_parser_accepts_known_values() {
        assert_eq!(
            native_submit_key_from_arg(None).unwrap(),
            platform::macos::NativeSubmitKey::Enter
        );
        assert_eq!(
            native_submit_key_from_arg(Some("none".to_string())).unwrap(),
            platform::macos::NativeSubmitKey::None
        );
        assert_eq!(
            native_submit_key_from_arg(Some("command_enter".to_string())).unwrap(),
            platform::macos::NativeSubmitKey::CommandEnter
        );
    }

    #[test]
    fn send_behavior_submit_key_parser_rejects_unknown_values() {
        assert_eq!(
            native_submit_key_from_arg(Some("space".to_string())).unwrap_err(),
            "Invalid submit key: space"
        );
    }

    #[test]
    fn focus_preserving_autosend_stops_before_paste_when_frontmost_changed() {
        let target = prompt_target("WeChat", "com.tencent.xinWeChat", Some(123));
        let events = RefCell::new(Vec::new());
        let mut frontmost = VecDeque::from([frontmost_target("Notes", "com.apple.Notes", Some(9))]);

        let outcome = guarded_focus_preserving_autosend_with_senders(
            "hello",
            &target,
            platform::macos::NativeSubmitKey::Enter,
            |body| {
                assert_eq!(body, "hello");
                events.borrow_mut().push("copy");
                Ok(())
            },
            || frontmost.pop_front(),
            || {
                events.borrow_mut().push("paste");
                Ok(())
            },
            |_| {
                events.borrow_mut().push("submit");
                Ok(())
            },
            |_| events.borrow_mut().push("sleep"),
        );

        assert!(outcome.copied);
        assert!(!outcome.sent);
        assert_eq!(outcome.reason, Some(AutosendFailureReason::NoSafeTarget));
        assert_eq!(&*events.borrow(), &["copy"]);
    }

    #[test]
    fn focus_preserving_autosend_skips_submit_when_frontmost_changes_after_paste() {
        let target = prompt_target("WeChat", "com.tencent.xinWeChat", Some(123));
        let events = RefCell::new(Vec::new());
        let mut frontmost = VecDeque::from([
            frontmost_target("WeChat", "com.tencent.xinWeChat", Some(123)),
            frontmost_target("Notes", "com.apple.Notes", Some(9)),
        ]);

        let outcome = guarded_focus_preserving_autosend_with_senders(
            "hello",
            &target,
            platform::macos::NativeSubmitKey::Enter,
            |_| {
                events.borrow_mut().push("copy");
                Ok(())
            },
            || frontmost.pop_front(),
            || {
                events.borrow_mut().push("paste");
                Ok(())
            },
            |_| {
                events.borrow_mut().push("submit");
                Ok(())
            },
            |_| events.borrow_mut().push("sleep"),
        );

        assert!(outcome.copied);
        assert!(!outcome.sent);
        assert_eq!(
            outcome.reason,
            Some(AutosendFailureReason::ReturnEventFailed)
        );
        assert_eq!(&*events.borrow(), &["copy", "paste", "sleep"]);
    }

    #[test]
    fn focus_preserving_autosend_pastes_and_submits_when_target_stays_frontmost() {
        let target = prompt_target("WeChat", "com.tencent.xinWeChat", Some(123));
        let events = RefCell::new(Vec::new());
        let submitted = RefCell::new(None);
        let mut frontmost = VecDeque::from([
            frontmost_target("WeChat", "com.tencent.xinWeChat", Some(123)),
            frontmost_target("WeChat", "com.tencent.xinWeChat", Some(123)),
        ]);

        let outcome = guarded_focus_preserving_autosend_with_senders(
            "hello",
            &target,
            platform::macos::NativeSubmitKey::Enter,
            |_| {
                events.borrow_mut().push("copy");
                Ok(())
            },
            || frontmost.pop_front(),
            || {
                events.borrow_mut().push("paste");
                Ok(())
            },
            |key| {
                events.borrow_mut().push("submit");
                submitted.replace(Some(key));
                Ok(())
            },
            |_| events.borrow_mut().push("sleep"),
        );

        assert!(outcome.sent);
        assert_eq!(*submitted.borrow(), Some(platform::macos::NativeSubmitKey::Enter));
        assert_eq!(&*events.borrow(), &["copy", "paste", "sleep", "submit"]);
    }

    #[test]
    fn focus_preserving_sequence_stops_before_first_paste_when_frontmost_changed() {
        let target = prompt_target("WeChat", "com.tencent.xinWeChat", Some(123));
        let events = RefCell::new(Vec::new());
        let bodies = vec!["one".to_string(), "two".to_string()];
        let mut frontmost = VecDeque::from([frontmost_target("Notes", "com.apple.Notes", Some(9))]);

        let outcome = focus_preserving_prompt_sequence_for_target_with_senders(
            &bodies,
            700,
            &target,
            platform::macos::NativeSubmitKey::Enter,
            |_| {
                events.borrow_mut().push("copy");
                Ok(())
            },
            || frontmost.pop_front(),
            || {
                events.borrow_mut().push("paste");
                Ok(())
            },
            |_| {
                events.borrow_mut().push("submit");
                Ok(())
            },
            |_| events.borrow_mut().push("sleep"),
        );

        assert!(!outcome.sent);
        assert_eq!(outcome.sent_count, 0);
        assert_eq!(outcome.failed_index, Some(1));
        assert_eq!(outcome.reason, Some(AutosendFailureReason::NoSafeTarget));
        assert_eq!(&*events.borrow(), &["copy"]);
    }

    #[test]
    fn focus_preserving_sequence_stops_before_second_paste_when_frontmost_changed() {
        let target = prompt_target("WeChat", "com.tencent.xinWeChat", Some(123));
        let events = RefCell::new(Vec::new());
        let bodies = vec!["one".to_string(), "two".to_string()];
        let mut frontmost = VecDeque::from([
            frontmost_target("WeChat", "com.tencent.xinWeChat", Some(123)),
            frontmost_target("WeChat", "com.tencent.xinWeChat", Some(123)),
            frontmost_target("Notes", "com.apple.Notes", Some(9)),
        ]);

        let outcome = focus_preserving_prompt_sequence_for_target_with_senders(
            &bodies,
            700,
            &target,
            platform::macos::NativeSubmitKey::Enter,
            |_| {
                events.borrow_mut().push("copy");
                Ok(())
            },
            || frontmost.pop_front(),
            || {
                events.borrow_mut().push("paste");
                Ok(())
            },
            |_| {
                events.borrow_mut().push("submit");
                Ok(())
            },
            |_| events.borrow_mut().push("sleep"),
        );

        assert!(!outcome.sent);
        assert_eq!(outcome.sent_count, 1);
        assert_eq!(outcome.failed_index, Some(2));
        assert_eq!(outcome.reason, Some(AutosendFailureReason::NoSafeTarget));
        assert_eq!(
            &*events.borrow(),
            &["copy", "paste", "sleep", "submit", "sleep", "copy"]
        );
    }

    #[test]
    fn focus_preserving_sequence_skips_submit_when_frontmost_changes_after_paste() {
        let target = prompt_target("WeChat", "com.tencent.xinWeChat", Some(123));
        let events = RefCell::new(Vec::new());
        let bodies = vec!["one".to_string()];
        let mut frontmost = VecDeque::from([
            frontmost_target("WeChat", "com.tencent.xinWeChat", Some(123)),
            frontmost_target("Notes", "com.apple.Notes", Some(9)),
        ]);

        let outcome = focus_preserving_prompt_sequence_for_target_with_senders(
            &bodies,
            700,
            &target,
            platform::macos::NativeSubmitKey::Enter,
            |_| {
                events.borrow_mut().push("copy");
                Ok(())
            },
            || frontmost.pop_front(),
            || {
                events.borrow_mut().push("paste");
                Ok(())
            },
            |_| {
                events.borrow_mut().push("submit");
                Ok(())
            },
            |_| events.borrow_mut().push("sleep"),
        );

        assert!(!outcome.sent);
        assert_eq!(outcome.sent_count, 0);
        assert_eq!(outcome.failed_index, Some(1));
        assert_eq!(
            outcome.reason,
            Some(AutosendFailureReason::ReturnEventFailed)
        );
        assert_eq!(&*events.borrow(), &["copy", "paste", "sleep"]);
    }

    #[test]
    fn prompt_pick_session_falls_back_from_prompt_picker_to_visible_business_app() {
        let target = prompt_pick_session_target(
            Some(frontmost_target("Prompt Picker", "local.promptpicker.dev", Some(1))),
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
            Some(frontmost_target("Prompt Picker", "local.promptpicker.dev", Some(1))),
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
                pid: None,
                observed_at_ms: 123,
                click_point: None,
            }),
        );

        assert!(target.is_none());
    }

    #[test]
    fn prompt_pick_session_uses_recent_target_when_prompt_picker_has_no_visible_app() {
        let target = prompt_pick_session_target(
            Some(frontmost_target("Prompt Picker", "local.promptpicker.dev", Some(1))),
            vec![],
            Some(LastInputTarget {
                app: FrontmostApp {
                    name: "WeChat".to_string(),
                    bundle_id: "com.tencent.xinWeChat".to_string(),
                },
                pid: Some(456),
                observed_at_ms: now_ms(),
                click_point: None,
            }),
        )
        .unwrap();

        assert_eq!(target.app.bundle_id, "com.tencent.xinWeChat");
        assert_eq!(target.pid, Some(456));
    }

    #[test]
    fn prompt_pick_session_prefers_recent_target_over_visible_app_when_picker_is_frontmost() {
        let target = prompt_pick_session_target(
            Some(frontmost_target("Prompt Picker", "local.promptpicker.dev", Some(1))),
            vec![FrontmostApp {
                name: "Codex".to_string(),
                bundle_id: "com.openai.codex".to_string(),
            }],
            Some(LastInputTarget {
                app: FrontmostApp {
                    name: "WeChat".to_string(),
                    bundle_id: "com.tencent.xinWeChat".to_string(),
                },
                pid: None,
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
        let result = paste_prompt_to_last_target_impl("hello", &state, |_| {
            panic!("copy sender must not run without a target")
        });

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
            None,
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
    fn autosend_falls_back_to_recent_target_when_prompt_session_is_not_ready() {
        let session_state = PromptPickSessionState::default();
        let recent_state = LastInputTargetState::default();
        recent_state.set(LastInputTarget {
            app: FrontmostApp {
                name: "Codex".to_string(),
                bundle_id: "com.openai.codex".to_string(),
            },
            pid: None,
            observed_at_ms: now_ms(),
            click_point: Some(TargetClickPoint { x: 640.0, y: 720.0 }),
        });

        let result = paste_prompt_and_submit_to_session_target_with_senders(
            "hello",
            &session_state,
            Some(&recent_state),
            |body, bundle_id, click_point| {
                assert_eq!(body, "hello");
                assert_eq!(bundle_id, "com.openai.codex");
                assert_eq!(click_point.unwrap().x, 640.0);
                AutosendOutcome::sent()
            },
            |_| panic!("copy sender must not run when recent target is usable"),
        );

        let outcome = result.unwrap();
        assert!(outcome.sent);
        assert!(session_state.get().is_none());
    }

    #[test]
    fn stale_prompt_pick_session_capture_is_ignored_after_new_session_begins() {
        let session_state = PromptPickSessionState::default();
        session_state.begin(1);
        assert!(record_prompt_pick_session_target_if_valid(
            &session_state,
            PromptPickSessionTarget {
                app: FrontmostApp {
                    name: "WeChat".to_string(),
                    bundle_id: "com.tencent.xinWeChat".to_string(),
                },
                pid: None,
                observed_at_ms: now_ms(),
                click_point: None,
            },
            1,
        )
        .is_some());
        assert_eq!(
            session_state.get().unwrap().app.bundle_id,
            "com.tencent.xinWeChat"
        );

        session_state.begin(2);
        assert!(record_prompt_pick_session_target_if_valid(
            &session_state,
            PromptPickSessionTarget {
                app: FrontmostApp {
                    name: "WeChat".to_string(),
                    bundle_id: "com.tencent.xinWeChat".to_string(),
                },
                pid: None,
                observed_at_ms: now_ms(),
                click_point: None,
            },
            1,
        )
        .is_none());
        assert!(session_state.get().is_none());

        let recent_state = LastInputTargetState::default();
        recent_state.set(LastInputTarget {
            app: FrontmostApp {
                name: "Codex".to_string(),
                bundle_id: "com.openai.codex".to_string(),
            },
            pid: None,
            observed_at_ms: now_ms(),
            click_point: None,
        });

        let result = paste_prompt_and_submit_to_session_target_with_senders(
            "hello",
            &session_state,
            Some(&recent_state),
            |_, bundle_id, _| {
                assert_eq!(bundle_id, "com.openai.codex");
                AutosendOutcome::sent()
            },
            |_| panic!("copy sender must not run when recent target is usable"),
        );

        assert!(result.unwrap().sent);
    }

    #[test]
    fn autosend_session_target_uses_app_sender_with_click_point() {
        let state = PromptPickSessionState::default();
        state.set(PromptPickSessionTarget {
            app: FrontmostApp {
                name: "WeChat".to_string(),
                bundle_id: "com.tencent.xinWeChat".to_string(),
            },
            pid: None,
            observed_at_ms: 123,
            click_point: Some(TargetClickPoint { x: 420.0, y: 720.0 }),
        });

        let result = paste_prompt_and_submit_to_session_target_with_senders(
            "hello",
            &state,
            None,
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
            pid: None,
            observed_at_ms: 123,
            click_point: None,
        });

        let result = paste_prompt_and_submit_to_session_target_with_senders(
            "hello",
            &state,
            None,
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
            pid: None,
            observed_at_ms: 123,
            click_point: None,
        });

        let result = paste_prompt_and_submit_to_session_target_with_senders(
            "hello",
            &state,
            None,
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
            pid: None,
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
            None,
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
            pid: None,
            observed_at_ms: 123,
            click_point: None,
        });
        let bodies = vec!["one".to_string(), "two".to_string()];
        let mut sleeps = Vec::new();

        paste_prompt_sequence_and_submit_to_session_target_with_senders(
            &bodies,
            10,
            &state,
            None,
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
            pid: None,
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
            None,
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
            None,
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
    fn autosend_sequence_falls_back_to_recent_target_when_prompt_session_is_not_ready() {
        let session_state = PromptPickSessionState::default();
        let recent_state = LastInputTargetState::default();
        recent_state.set(LastInputTarget {
            app: FrontmostApp {
                name: "WeChat".to_string(),
                bundle_id: "com.tencent.xinWeChat".to_string(),
            },
            pid: None,
            observed_at_ms: now_ms(),
            click_point: None,
        });
        let bodies = vec!["one".to_string(), "two".to_string()];
        let mut sent = Vec::new();

        let result = paste_prompt_sequence_and_submit_to_session_target_with_senders(
            &bodies,
            700,
            &session_state,
            Some(&recent_state),
            |body, bundle_id, click_point| {
                sent.push((body.to_string(), bundle_id.to_string(), click_point));
                AutosendOutcome::sent()
            },
            |_| panic!("copy sender must not run when recent target is usable"),
            |_| {},
        )
        .unwrap();

        assert!(result.sent);
        assert_eq!(result.sent_count, 2);
        assert_eq!(sent.len(), 2);
        assert_eq!(sent[0].1, "com.tencent.xinWeChat");
        assert!(sent[0].2.is_none());
    }

    #[test]
    fn accepts_non_codex_target_for_autosend() {
        let state = LastInputTargetState::default();
        state.set(LastInputTarget {
            app: FrontmostApp {
                name: "WeChat".to_string(),
                bundle_id: "com.tencent.xinWeChat".to_string(),
            },
            pid: None,
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
            pid: None,
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
            pid: None,
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
            pid: None,
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
            frontmost_target("WeChat", "com.tencent.xinWeChat", Some(123)),
        );

        assert_eq!(state.get().unwrap().app.bundle_id, "com.tencent.xinWeChat");
        assert_eq!(state.get().unwrap().pid, Some(123));
    }

    #[test]
    fn skips_prompt_picker_as_frontmost_app_fallback() {
        let state = LastInputTargetState::default();
        record_last_app_if_valid(
            &state,
            frontmost_target("Prompt Picker", "local.promptpicker.dev", Some(1)),
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
        assert_eq!(
            tray_menu_action(TRAY_OPEN_SETTINGS_ID),
            TrayMenuAction::OpenSettingsWindow
        );
        assert_eq!(tray_menu_action(TRAY_QUIT_ID), TrayMenuAction::Quit);
    }

    #[test]
    fn ignores_unknown_tray_menu_item_ids() {
        assert_eq!(tray_menu_action("unknown"), TrayMenuAction::Unknown);
    }

    #[test]
    fn menubar_template_icon_is_transparent_mask() {
        let icon = menubar_template_icon();

        assert_eq!(icon.width(), MENUBAR_TEMPLATE_ICON_SIZE);
        assert_eq!(icon.height(), MENUBAR_TEMPLATE_ICON_SIZE);
        assert_eq!(
            icon.rgba().len(),
            (MENUBAR_TEMPLATE_ICON_SIZE * MENUBAR_TEMPLATE_ICON_SIZE * 4) as usize
        );
        let alpha_values: std::collections::BTreeSet<u8> =
            icon.rgba().chunks_exact(4).map(|pixel| pixel[3]).collect();
        let opaque_pixels = icon
            .rgba()
            .chunks_exact(4)
            .filter(|pixel| pixel[3] == 255)
            .count();

        assert!(alpha_values.contains(&0));
        assert!(alpha_values.contains(&255));
        assert!(alpha_values
            .iter()
            .all(|alpha| *alpha == 0 || *alpha == 255));
        assert!((60..=220).contains(&opaque_pixels));
    }

    #[test]
    fn default_settings_tracks_accessibility_prompt_history() {
        let settings = default_settings_value();

        assert_eq!(
            settings.pointer("/permissions/accessibilityPromptRequested"),
            Some(&serde_json::Value::Bool(false))
        );
    }

    #[test]
    fn permission_status_does_not_require_accessibility_on_non_macos() {
        let status = prompt_interaction_permission_status_from_parts(
            false,
            false,
            false,
            "zh-CN".to_string(),
        );

        assert!(!status.required);
        assert!(status.trusted);
        assert!(!status.native_prompt_requested);
        assert_eq!(status.language, "zh-CN");
    }

    #[test]
    fn permission_status_reports_untrusted_macos_prompt_state() {
        let status =
            prompt_interaction_permission_status_from_parts(true, false, true, "en-US".to_string());

        assert!(status.required);
        assert!(!status.trusted);
        assert!(status.native_prompt_requested);
        assert_eq!(status.language, "en-US");
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
