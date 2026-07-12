use tauri::{
    image::Image,
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::TrayIconBuilder,
    Emitter, Manager, WindowEvent,
};
use tauri_plugin_clipboard_manager::ClipboardExt;

#[cfg(debug_assertions)]
mod calico_probe;
mod platform;
pub use platform::{
    accessibility_status, frontmost_app, request_accessibility_permission, AccessibilityStatus,
    AutosendOutcome, CandidateInput, FrontmostApp, FrontmostAppWithPid,
};
mod overlay_position;
pub use overlay_position::{prompt_button_position, OverlayPoint};
mod windows;
pub use windows::{
    acknowledge_prompt_popover_mode, hide_prompt_button, hide_prompt_popover,
    move_prompt_button_to, prompt_button_position_cmd, show_prompt_button,
    show_prompt_button_controls_from_button, show_prompt_popover, show_prompt_popover_from_button,
    toggle_prompt_popover_from_button,
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
    settings_state: tauri::State<SettingsFileState>,
) -> PromptInteractionPermissionStatus {
    let settings = settings_state.read_value();
    prompt_interaction_permission_status_from_parts(
        cfg!(target_os = "macos"),
        accessibility_status().trusted,
        accessibility_prompt_requested(&settings),
        settings_language(&settings).to_string(),
    )
}

#[tauri::command]
fn request_prompt_interaction_permission(
    settings_state: tauri::State<SettingsFileState>,
) -> AccessibilityStatus {
    let _ = set_accessibility_prompt_requested(settings_state.inner(), true);
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
    session_state.begin(session_id);

    tauri::async_runtime::spawn_blocking(move || {
        if let Some(input_target) = platform::macos::current_input_target() {
            record_last_input_target_if_valid(&recent_state, &input_target);
        }

        let recent_identity = recent_state.captured_identity();
        let Some(target) =
            prompt_pick_session_target(platform::frontmost_app_with_pid(), recent_state.get())
        else {
            session_state.clear_if_current(session_id);
            return None;
        };
        let Some(identity) = captured_identity_for_target(&target, recent_identity.as_ref()) else {
            session_state.clear_if_current(session_id);
            return None;
        };
        record_prompt_pick_session_target_if_valid(&session_state, target, identity, session_id)
    })
    .await
    .map_err(|error| format!("Prompt pick session task failed: {}", error))
}

#[tauri::command]
fn paste_prompt(body: String, app: tauri::AppHandle) -> Result<(), String> {
    platform::macos::paste_prompt_with_copier(&body, |text| copy_text_to_clipboard(&app, text))
}

#[tauri::command]
async fn paste_prompt_and_submit_to_last_target(
    body: String,
    submit_key: Option<String>,
    session_state: tauri::State<'_, PromptPickSessionState>,
    recent_state: tauri::State<'_, LastInputTargetState>,
    settings_state: tauri::State<'_, SettingsFileState>,
    app: tauri::AppHandle,
) -> Result<AutosendOutcome, String> {
    native_submit_key_from_arg(submit_key)?;
    let submit_key = authoritative_submit_key(settings_state.read_text());
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
    pub processed_count: usize,
    pub completion: Option<platform::AutosendCompletion>,
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
            processed_count: count,
            completion: Some(platform::AutosendCompletion::Submitted),
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
            processed_count: sent_count,
            completion: outcome.completion,
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
    settings_state: tauri::State<'_, SettingsFileState>,
    app: tauri::AppHandle,
) -> Result<AutosendSequenceOutcome, String> {
    native_submit_key_from_arg(submit_key)?;
    let submit_key = authoritative_submit_key(settings_state.read_text());
    let session_state = session_state.inner().clone();
    let recent_state = recent_state.inner().clone();
    let app = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        if submit_key == platform::macos::NativeSubmitKey::None {
            let body = bodies.join("\n\n");
            let outcome = paste_prompt_and_submit_to_last_target_impl(
                &body,
                &session_state,
                &recent_state,
                &app,
                submit_key,
            )?;
            return Ok(if outcome.sent {
                AutosendSequenceOutcome::sent_all(bodies.len())
            } else {
                AutosendSequenceOutcome::from_failure(outcome, 0, 0)
            });
        }
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
    let captured_window = state
        .captured_identity()
        .or_else(|| recent_state.captured_identity())
        .and_then(|identity| identity.window.map(|window| window.frame));
    paste_prompt_sequence_and_submit_to_session_target_with_senders(
        bodies,
        interval_ms,
        state,
        Some(recent_state),
        submit_key,
        |body, bundle_id, click_point, submit_key| {
            platform::macos::paste_prompt_and_submit_to_app_clipboard_with_copier(
                body,
                bundle_id,
                click_point.map(|point| (point.x, point.y)),
                captured_window.as_ref(),
                submit_key,
                |text| copy_text_to_clipboard(app, text),
            )
        },
        |text| copy_text_to_clipboard(app, text),
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
    let captured_window = state
        .captured_identity()
        .or_else(|| recent_state.captured_identity())
        .and_then(|identity| identity.window.map(|window| window.frame));
    paste_prompt_and_submit_to_session_target_with_senders(
        body,
        state,
        Some(recent_state),
        submit_key,
        |body, bundle_id, click_point, submit_key| {
            platform::macos::paste_prompt_and_submit_to_app_clipboard_with_copier(
                body,
                bundle_id,
                click_point.map(|point| (point.x, point.y)),
                captured_window.as_ref(),
                submit_key,
                |text| copy_text_to_clipboard(app, text),
            )
        },
        |text| copy_text_to_clipboard(app, text),
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

fn authoritative_submit_key(
    settings_text: Result<Option<String>, String>,
) -> platform::macos::NativeSubmitKey {
    settings_text
        .ok()
        .flatten()
        .and_then(|contents| serde_json::from_str::<serde_json::Value>(&contents).ok())
        .and_then(|settings| {
            settings
                .pointer("/promptInsertion/mode")
                .and_then(serde_json::Value::as_str)
                .map(str::to_owned)
        })
        .map(|mode| match mode.as_str() {
            "paste_only" => platform::macos::NativeSubmitKey::None,
            "paste_command_enter" => platform::macos::NativeSubmitKey::CommandEnter,
            "paste_enter" | "paste_and_submit" => platform::macos::NativeSubmitKey::Enter,
            _ => platform::macos::NativeSubmitKey::None,
        })
        .unwrap_or(platform::macos::NativeSubmitKey::None)
}

#[allow(dead_code)]
fn repair_target_focus(target: &PromptPickSessionTarget) -> Result<(), String> {
    let Some(pid) = target.pid else {
        return Err("Captured target pid is unavailable for AX focus repair.".to_string());
    };
    platform::macos::repair_focus_to_editable_element(pid)
}

#[allow(dead_code)]
fn recover_target_for_autosend(target: &PromptPickSessionTarget) -> Result<(), String> {
    platform::macos::recover_target_app_for_autosend(
        &target.app.bundle_id,
        target.click_point.map(|point| (point.x, point.y)),
    )?;

    if target.click_point.is_none() {
        repair_target_focus(target)?;
    }

    Ok(())
}

fn prompt_pick_target_or_recent(
    session_state: &PromptPickSessionState,
    recent_state: Option<&LastInputTargetState>,
) -> Option<CapturedPromptTarget> {
    if let Some((target, identity)) = session_state.take_captured() {
        return Some(CapturedPromptTarget { target, identity });
    }

    let recent_state = recent_state?;
    let target = recent_state.get()?;
    if !is_recent_prompt_target(&target) || !is_usable_autosend_app(&target.app) {
        return None;
    }
    Some(CapturedPromptTarget {
        target: PromptPickSessionTarget {
            app: target.app,
            pid: target.pid,
            observed_at_ms: now_ms(),
            click_point: target.click_point,
        },
        identity: recent_state.captured_identity(),
    })
}

struct CapturedPromptTarget {
    target: PromptPickSessionTarget,
    identity: Option<CapturedTargetIdentity>,
}

impl std::ops::Deref for CapturedPromptTarget {
    type Target = PromptPickSessionTarget;

    fn deref(&self) -> &Self::Target {
        &self.target
    }
}

#[allow(dead_code)]
fn focus_preserving_prompt_to_last_target_impl<C, F, R, P, S, W>(
    body: &str,
    state: &PromptPickSessionState,
    recent_state: Option<&LastInputTargetState>,
    submit_key: platform::macos::NativeSubmitKey,
    copy_sender: C,
    frontmost_reader: F,
    recover_target: R,
    paste_sender: P,
    submit_sender: S,
    sleeper: W,
) -> Result<AutosendOutcome, String>
where
    C: FnOnce(&str) -> Result<(), String>,
    F: FnMut() -> Option<FrontmostAppWithPid>,
    R: FnMut(&PromptPickSessionTarget) -> Result<(), String>,
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

    if target
        .identity
        .as_ref()
        .is_some_and(|identity| !captured_target_identity_is_current(identity))
    {
        return Ok(stale_target_outcome());
    }

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
        recover_target,
        paste_sender,
        submit_sender,
        sleeper,
    ))
}

#[allow(dead_code)]
fn focus_preserving_prompt_sequence_to_last_target_impl<C, F, R, P, S, W>(
    bodies: &[String],
    interval_ms: u64,
    state: &PromptPickSessionState,
    recent_state: Option<&LastInputTargetState>,
    submit_key: platform::macos::NativeSubmitKey,
    copy_sender: C,
    frontmost_reader: F,
    recover_target: R,
    paste_sender: P,
    submit_sender: S,
    sleeper: W,
) -> Result<AutosendSequenceOutcome, String>
where
    C: FnMut(&str) -> Result<(), String>,
    F: FnMut() -> Option<FrontmostAppWithPid>,
    R: FnMut(&PromptPickSessionTarget) -> Result<(), String>,
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

    if target
        .identity
        .as_ref()
        .is_some_and(|identity| !captured_target_identity_is_current(identity))
    {
        return Ok(AutosendSequenceOutcome::from_failure(
            stale_target_outcome(),
            0,
            0,
        ));
    }

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
        recover_target,
        paste_sender,
        submit_sender,
        sleeper,
    ))
}

fn focus_preserving_prompt_sequence_for_target_with_senders<C, F, R, P, S, W>(
    clean_bodies: &[String],
    interval_ms: u64,
    target: &PromptPickSessionTarget,
    submit_key: platform::macos::NativeSubmitKey,
    mut copy_sender: C,
    mut frontmost_reader: F,
    mut recover_target: R,
    mut paste_sender: P,
    mut submit_sender: S,
    mut sleeper: W,
) -> AutosendSequenceOutcome
where
    C: FnMut(&str) -> Result<(), String>,
    F: FnMut() -> Option<FrontmostAppWithPid>,
    R: FnMut(&PromptPickSessionTarget) -> Result<(), String>,
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
            |target| recover_target(target),
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

fn guarded_focus_preserving_autosend_with_senders<C, F, R, P, S, W>(
    body: &str,
    target: &PromptPickSessionTarget,
    submit_key: platform::macos::NativeSubmitKey,
    copy_sender: C,
    mut frontmost_reader: F,
    mut recover_target: R,
    paste_sender: P,
    submit_sender: S,
    sleeper: W,
) -> AutosendOutcome
where
    C: FnOnce(&str) -> Result<(), String>,
    F: FnMut() -> Option<FrontmostAppWithPid>,
    R: FnMut(&PromptPickSessionTarget) -> Result<(), String>,
    P: FnOnce() -> Result<(), String>,
    S: FnOnce(platform::macos::NativeSubmitKey) -> Result<(), String>,
    W: FnOnce(u64),
{
    if let Err(error) = copy_sender(body) {
        return AutosendOutcome::copy_failed(error);
    }

    let before_paste = frontmost_reader();
    let classification = classify_target_frontmost(target, before_paste.as_ref());
    emit_autosend_diagnostic(
        "before-paste",
        target,
        before_paste.as_ref(),
        Some(classification),
    );
    match classification {
        TargetFrontmostStatus::Target => {}
        TargetFrontmostStatus::PromptPicker => {
            if recover_target(target).is_err() {
                emit_autosend_diagnostic(
                    "recovery-failed",
                    target,
                    before_paste.as_ref(),
                    Some(classification),
                );
                return AutosendOutcome::copied_without_send(
                    "Target app changed before paste; prompt was copied instead.".to_string(),
                );
            }
            let after_recovery = frontmost_reader();
            if !captured_target_matches_frontmost(target, after_recovery.as_ref()) {
                emit_autosend_diagnostic(
                    "post-recovery-mismatch",
                    target,
                    after_recovery.as_ref(),
                    Some(classification),
                );
                return AutosendOutcome::copied_without_send(
                    "Target app changed before paste; prompt was copied instead.".to_string(),
                );
            }
            emit_autosend_diagnostic(
                "post-recovery-target",
                target,
                after_recovery.as_ref(),
                Some(TargetFrontmostStatus::Target),
            );
        }
        TargetFrontmostStatus::OtherOrUnknown => {
            emit_autosend_diagnostic(
                "other-or-unknown",
                target,
                before_paste.as_ref(),
                Some(classification),
            );
            return AutosendOutcome::copied_without_send(
                "Target app changed before paste; prompt was copied instead.".to_string(),
            );
        }
    }

    if let Err(error) = paste_sender() {
        return AutosendOutcome::paste_event_failed(format!(
            "Focus-preserving paste failed: {}",
            error
        ));
    }

    sleeper(FOCUS_PRESERVING_PASTE_SETTLE_MS);

    if submit_key == platform::macos::NativeSubmitKey::None {
        emit_autosend_diagnostic("sent-without-submit", target, None, None);
        return AutosendOutcome::pasted_only();
    }

    let before_submit = frontmost_reader();
    if !captured_target_matches_frontmost(target, before_submit.as_ref()) {
        emit_autosend_diagnostic(
            "before-submit-mismatch",
            target,
            before_submit.as_ref(),
            None,
        );
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

    emit_autosend_diagnostic("sent", target, before_submit.as_ref(), None);
    AutosendOutcome::sent()
}

fn paste_prompt_and_submit_to_session_target_with_senders<A, C>(
    body: &str,
    state: &PromptPickSessionState,
    recent_state: Option<&LastInputTargetState>,
    submit_key: platform::macos::NativeSubmitKey,
    app_sender: A,
    copy_sender: C,
) -> Result<AutosendOutcome, String>
where
    A: FnOnce(
        &str,
        &str,
        Option<TargetClickPoint>,
        platform::macos::NativeSubmitKey,
    ) -> AutosendOutcome,
    C: FnOnce(&str) -> Result<(), String>,
{
    let Some(target) = prompt_pick_target_or_recent(state, recent_state) else {
        return Ok(copy_without_sending(
            body,
            copy_sender,
            "No prompt pick target app was recorded for autosend.",
        ));
    };

    if target
        .identity
        .as_ref()
        .is_some_and(|identity| !captured_target_identity_is_current(identity))
    {
        return Ok(stale_target_outcome());
    }

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

    Ok(app_sender(
        body,
        &target.app.bundle_id,
        target.click_point,
        submit_key,
    ))
}

fn paste_prompt_sequence_and_submit_to_session_target_with_senders<A, C, S>(
    bodies: &[String],
    interval_ms: u64,
    state: &PromptPickSessionState,
    recent_state: Option<&LastInputTargetState>,
    submit_key: platform::macos::NativeSubmitKey,
    mut app_sender: A,
    copy_sender: C,
    mut sleeper: S,
) -> Result<AutosendSequenceOutcome, String>
where
    A: FnMut(
        &str,
        &str,
        Option<TargetClickPoint>,
        platform::macos::NativeSubmitKey,
    ) -> AutosendOutcome,
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

    if target
        .identity
        .as_ref()
        .is_some_and(|identity| !captured_target_identity_is_current(identity))
    {
        return Ok(AutosendSequenceOutcome::from_failure(
            stale_target_outcome(),
            0,
            0,
        ));
    }

    if is_unsafe_autosend_target(&target.app) || !allows_app_only_autosend(&target.app) {
        let outcome = copy_without_sending(
            first_body,
            copy_sender,
            "Target app is not safe for app-only autosend.",
        );
        return Ok(AutosendSequenceOutcome::from_failure(outcome, 0, 1));
    }

    if submit_key == platform::macos::NativeSubmitKey::None {
        let joined = clean_bodies.join("\n\n");
        let outcome = app_sender(
            &joined,
            &target.app.bundle_id,
            target.click_point,
            submit_key,
        );
        if outcome.completion == Some(platform::AutosendCompletion::PastedOnly) {
            return Ok(AutosendSequenceOutcome {
                copied: true,
                sent: false,
                sent_count: 0,
                processed_count: clean_bodies.len(),
                completion: outcome.completion,
                failed_index: None,
                error: None,
                reason: None,
            });
        }
        return Ok(AutosendSequenceOutcome::from_failure(outcome, 0, 1));
    }

    let delay_ms = clamp_sequence_interval_ms(interval_ms);
    for (index, body) in clean_bodies.iter().enumerate() {
        let outcome = app_sender(body, &target.app.bundle_id, target.click_point, submit_key);
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
        return Err("Prompt Drawer tray icon is not available.".to_string());
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

#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize)]
pub struct TargetClickPoint {
    pub x: f64,
    pub y: f64,
}

#[derive(Clone, Debug, PartialEq)]
struct TargetApplicationIdentity {
    bundle_id: String,
    main_pid: u32,
    launch_identity: platform::ProcessLaunchIdentity,
}

#[derive(Clone, Debug, PartialEq)]
struct TargetWindowIdentity {
    owner_pid: u32,
    frame: CandidateInput,
    role: Option<String>,
    title_hash: Option<String>,
    cg_window_id: Option<u32>,
}

#[derive(Clone, Debug, PartialEq)]
struct CapturedTargetIdentity {
    application: TargetApplicationIdentity,
    window: Option<TargetWindowIdentity>,
}

#[derive(Default)]
struct LastInputTargetInner {
    target: Option<LastInputTarget>,
    identity: Option<CapturedTargetIdentity>,
}

#[derive(Clone, Default)]
pub struct LastInputTargetState(std::sync::Arc<std::sync::Mutex<LastInputTargetInner>>);

impl LastInputTargetState {
    pub fn set(&self, target: LastInputTarget) {
        let mut state = self.0.lock().expect("last input target lock poisoned");
        state.target = Some(target);
        state.identity = None;
    }

    fn set_captured(&self, target: LastInputTarget, identity: CapturedTargetIdentity) {
        let mut state = self.0.lock().expect("last input target lock poisoned");
        state.target = Some(target);
        state.identity = Some(identity);
    }

    pub fn clear(&self) {
        let mut state = self.0.lock().expect("last input target lock poisoned");
        state.target = None;
        state.identity = None;
    }

    pub fn get(&self) -> Option<LastInputTarget> {
        self.0
            .lock()
            .expect("last input target lock poisoned")
            .target
            .clone()
    }

    fn captured_identity(&self) -> Option<CapturedTargetIdentity> {
        self.0
            .lock()
            .expect("last input target lock poisoned")
            .identity
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
    identity: Option<CapturedTargetIdentity>,
}

#[derive(Clone, Default)]
pub struct PromptPickSessionState(std::sync::Arc<std::sync::Mutex<PromptPickSessionInner>>);

impl PromptPickSessionState {
    pub fn begin(&self, session_id: u64) {
        let mut state = self.0.lock().expect("prompt pick session lock poisoned");
        state.active_session_id = session_id;
        state.target = None;
        state.identity = None;
    }

    pub fn begin_if_new(&self, session_id: u64) {
        let mut state = self.0.lock().expect("prompt pick session lock poisoned");
        if state.active_session_id == session_id {
            return;
        }
        state.active_session_id = session_id;
        state.target = None;
        state.identity = None;
    }

    pub fn set(&self, target: PromptPickSessionTarget) {
        let mut state = self.0.lock().expect("prompt pick session lock poisoned");
        state.target = Some(target);
        state.identity = None;
    }

    pub fn set_if_current(&self, session_id: u64, target: PromptPickSessionTarget) -> bool {
        let mut state = self.0.lock().expect("prompt pick session lock poisoned");
        if state.active_session_id != session_id {
            return false;
        }
        state.target = Some(target);
        state.identity = None;
        true
    }

    fn set_captured_if_current(
        &self,
        session_id: u64,
        target: PromptPickSessionTarget,
        identity: CapturedTargetIdentity,
    ) -> bool {
        let mut state = self.0.lock().expect("prompt pick session lock poisoned");
        if state.active_session_id != session_id {
            return false;
        }
        state.target = Some(target);
        state.identity = Some(identity);
        true
    }

    pub fn clear(&self) {
        let mut state = self.0.lock().expect("prompt pick session lock poisoned");
        state.target = None;
        state.identity = None;
    }

    pub fn clear_if_current(&self, session_id: u64) -> bool {
        let mut state = self.0.lock().expect("prompt pick session lock poisoned");
        if state.active_session_id != session_id {
            return false;
        }
        state.target = None;
        state.identity = None;
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
        let mut state = self.0.lock().expect("prompt pick session lock poisoned");
        state.identity = None;
        state.target.take()
    }

    fn take_captured(&self) -> Option<(PromptPickSessionTarget, Option<CapturedTargetIdentity>)> {
        let mut state = self.0.lock().expect("prompt pick session lock poisoned");
        let target = state.target.take()?;
        Some((target, state.identity.take()))
    }

    fn captured_identity(&self) -> Option<CapturedTargetIdentity> {
        self.0
            .lock()
            .expect("prompt pick session lock poisoned")
            .identity
            .clone()
    }
}

pub(crate) struct PromptButtonVisibilityState {
    desired_visible: std::sync::atomic::AtomicBool,
    generation: std::sync::atomic::AtomicU64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PromptButtonRendererAction {
    Ignore,
    HideCurrent,
    ShowCurrent,
}

impl PromptButtonRendererAction {
    fn as_str(self) -> &'static str {
        match self {
            Self::Ignore => "ignore",
            Self::HideCurrent => "hideCurrent",
            Self::ShowCurrent => "showCurrent",
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct PromptButtonRendererSnapshot {
    instance_id: u64,
    transition_id: u64,
    ready: bool,
    visibility_generation: u64,
    action: PromptButtonRendererAction,
}

#[derive(Default)]
struct PromptButtonRendererInner {
    next_instance_id: u64,
    current_instance_id: u64,
    last_transition_id: u64,
    ready: bool,
    resume_requested: bool,
}

#[derive(Default)]
pub(crate) struct PromptButtonRendererState {
    inner: std::sync::Mutex<PromptButtonRendererInner>,
}

#[derive(Default)]
pub(crate) struct PromptButtonRecoveryUrlState {
    url: std::sync::Mutex<Option<tauri::Url>>,
}

impl PromptButtonRecoveryUrlState {
    pub(crate) fn store(&self, url: tauri::Url) {
        *self.url.lock().expect("recovery URL state lock poisoned") = Some(url);
    }

    fn get(&self) -> Option<tauri::Url> {
        self.url
            .lock()
            .expect("recovery URL state lock poisoned")
            .clone()
    }
}

impl PromptButtonRendererState {
    pub(crate) fn allocate_instance(&self) -> u64 {
        let mut inner = self.inner.lock().expect("renderer state lock poisoned");
        inner.next_instance_id = inner.next_instance_id.wrapping_add(1).max(1);
        inner.current_instance_id = inner.next_instance_id;
        inner.last_transition_id = 0;
        inner.ready = false;
        inner.resume_requested = false;
        inner.current_instance_id
    }

    pub(crate) fn is_ready(&self) -> bool {
        self.inner
            .lock()
            .expect("renderer state lock poisoned")
            .ready
    }

    pub(crate) fn request_resume_once(&self) -> bool {
        let mut inner = self.inner.lock().expect("renderer state lock poisoned");
        if inner.ready || inner.resume_requested || inner.current_instance_id == 0 {
            return false;
        }
        inner.resume_requested = true;
        true
    }

    fn current(&self) -> (u64, u64, bool) {
        let inner = self.inner.lock().expect("renderer state lock poisoned");
        (
            inner.current_instance_id,
            inner.last_transition_id,
            inner.ready,
        )
    }

    fn instance_is_current_unready(&self, instance_id: u64) -> bool {
        let inner = self.inner.lock().expect("renderer state lock poisoned");
        inner.current_instance_id == instance_id && !inner.ready
    }

    fn accept(
        &self,
        instance_id: u64,
        transition_id: u64,
        ready: bool,
        visibility: &PromptButtonVisibilityState,
    ) -> Option<PromptButtonRendererSnapshot> {
        let mut inner = self.inner.lock().expect("renderer state lock poisoned");
        if instance_id == 0
            || instance_id != inner.current_instance_id
            || transition_id <= inner.last_transition_id
        {
            return None;
        }
        inner.last_transition_id = transition_id;
        inner.ready = ready;
        inner.resume_requested = false;
        let visibility_generation = visibility.generation();
        let action = if !ready {
            PromptButtonRendererAction::HideCurrent
        } else if visibility.desired_visible() {
            PromptButtonRendererAction::ShowCurrent
        } else {
            PromptButtonRendererAction::Ignore
        };
        Some(PromptButtonRendererSnapshot {
            instance_id,
            transition_id,
            ready,
            visibility_generation,
            action,
        })
    }

    fn action_is_current(
        &self,
        snapshot: PromptButtonRendererSnapshot,
        visibility: &PromptButtonVisibilityState,
    ) -> bool {
        let inner = self.inner.lock().expect("renderer state lock poisoned");
        let transition_is_current = inner.current_instance_id == snapshot.instance_id
            && inner.last_transition_id == snapshot.transition_id
            && inner.ready == snapshot.ready;
        if !transition_is_current {
            return false;
        }
        match snapshot.action {
            PromptButtonRendererAction::Ignore => false,
            PromptButtonRendererAction::HideCurrent => !inner.ready,
            PromptButtonRendererAction::ShowCurrent => {
                inner.ready && visibility.may_show(snapshot.visibility_generation)
            }
        }
    }
}

#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct PromptButtonRendererReadyOutcome {
    accepted: bool,
    current_instance_id: u64,
    transition_id: u64,
    ready: bool,
    action: String,
    applied: bool,
}

#[tauri::command]
fn set_prompt_button_renderer_ready(
    renderer_instance_id: u64,
    transition_id: u64,
    ready: bool,
    app: tauri::AppHandle,
    renderer_state: tauri::State<PromptButtonRendererState>,
    visibility_state: tauri::State<PromptButtonVisibilityState>,
) -> PromptButtonRendererReadyOutcome {
    let Some(snapshot) = renderer_state.accept(
        renderer_instance_id,
        transition_id,
        ready,
        visibility_state.inner(),
    ) else {
        let (current_instance_id, current_transition_id, current_ready) = renderer_state.current();
        return PromptButtonRendererReadyOutcome {
            accepted: false,
            current_instance_id,
            transition_id: current_transition_id,
            ready: current_ready,
            action: "ignore".to_string(),
            applied: false,
        };
    };

    let applied = renderer_state.action_is_current(snapshot, visibility_state.inner())
        && match snapshot.action {
            PromptButtonRendererAction::Ignore => false,
            PromptButtonRendererAction::HideCurrent => hide_prompt_button(app.clone()).is_ok(),
            PromptButtonRendererAction::ShowCurrent => {
                crate::windows::show_ready_prompt_button_window(&app).is_ok()
            }
        };

    PromptButtonRendererReadyOutcome {
        accepted: true,
        current_instance_id: snapshot.instance_id,
        transition_id: snapshot.transition_id,
        ready: snapshot.ready,
        action: snapshot.action.as_str().to_string(),
        applied,
    }
}

fn is_prompt_button_webview(label: &str) -> bool {
    label == crate::windows::BUTTON_WINDOW_LABEL
}

const PROMPT_BUTTON_TERMINATION_RETRY_DELAYS_MS: [u64; 3] = [0, 100, 400];

fn prompt_button_termination_retry_delay(attempt: usize) -> Option<u64> {
    PROMPT_BUTTON_TERMINATION_RETRY_DELAYS_MS
        .get(attempt)
        .copied()
}

fn prompt_button_recovery_url(
    current_url: Result<tauri::Url, String>,
    stored_url: Option<tauri::Url>,
    instance_id: u64,
) -> Result<tauri::Url, String> {
    let mut url = current_url.or_else(|_| {
        stored_url.ok_or_else(|| "Prompt button recovery URL is missing.".to_string())
    })?;
    url.set_path("/overlay.html");
    url.set_query(Some(&format!("rendererInstanceId={instance_id}")));
    Ok(url)
}

fn perform_prompt_button_webview_recovery<Hide, CurrentUrl, Navigate>(
    stored_url: Option<tauri::Url>,
    instance_id: u64,
    hide: Hide,
    current_url: CurrentUrl,
    navigate: Navigate,
) -> Result<tauri::Url, String>
where
    Hide: FnOnce() -> Result<(), String>,
    CurrentUrl: FnOnce() -> Result<tauri::Url, String>,
    Navigate: FnOnce(tauri::Url) -> Result<(), String>,
{
    hide().map_err(|error| format!("Failed to hide terminated prompt button WebView: {error}"))?;
    let recovery_url = prompt_button_recovery_url(current_url(), stored_url, instance_id)?;
    navigate(recovery_url.clone())
        .map_err(|error| format!("Failed to navigate terminated prompt button WebView: {error}"))?;
    Ok(recovery_url)
}

fn recover_prompt_button_webview_once<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    instance_id: u64,
) -> Result<(), String> {
    let renderer_state = app.state::<PromptButtonRendererState>();
    if !renderer_state.instance_is_current_unready(instance_id) {
        return Err(format!(
            "Prompt button renderer instance {instance_id} is no longer current."
        ));
    }
    let window = app
        .get_webview_window(crate::windows::BUTTON_WINDOW_LABEL)
        .ok_or_else(|| "Prompt button window is missing during WebContent recovery.".to_string())?;
    let stored_url = app.state::<PromptButtonRecoveryUrlState>().get();
    let recovery_url = perform_prompt_button_webview_recovery(
        stored_url,
        instance_id,
        || window.hide().map_err(|error| error.to_string()),
        || window.url().map_err(|error| error.to_string()),
        |url| window.navigate(url).map_err(|error| error.to_string()),
    )?;
    app.state::<PromptButtonRecoveryUrlState>()
        .store(recovery_url);
    Ok(())
}

fn queue_prompt_button_webcontent_recovery<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    instance_id: u64,
    attempt: usize,
) -> Result<(), String> {
    let queued_app = app.clone();
    app.run_on_main_thread(move || {
        match recover_prompt_button_webview_once(&queued_app, instance_id) {
            Ok(()) => {
                eprintln!(
                    "Recovered prompt button WebContent in the existing window on attempt {}.",
                    attempt + 1
                );
            }
            Err(error) => {
                eprintln!(
                    "Prompt button WebContent recovery attempt {} failed: {error}",
                    attempt + 1
                );
                let renderer_state = queued_app.state::<PromptButtonRendererState>();
                if renderer_state.instance_is_current_unready(instance_id)
                    && prompt_button_termination_retry_delay(attempt + 1).is_some()
                {
                    if let Err(schedule_error) = schedule_prompt_button_webcontent_recovery(
                        queued_app.clone(),
                        instance_id,
                        attempt + 1,
                    ) {
                        eprintln!(
                            "Failed to schedule prompt button WebContent recovery attempt {}: {schedule_error}",
                            attempt + 2
                        );
                    }
                }
            }
        }
    })
    .map_err(|error| format!("Failed to queue prompt button WebContent recovery: {error}"))
}

fn schedule_prompt_button_webcontent_recovery<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    instance_id: u64,
    attempt: usize,
) -> Result<(), String> {
    let delay_ms = prompt_button_termination_retry_delay(attempt)
        .ok_or_else(|| "Prompt button WebContent recovery attempts are exhausted.".to_string())?;
    if delay_ms == 0 {
        return queue_prompt_button_webcontent_recovery(app, instance_id, attempt);
    }
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(delay_ms));
        if let Err(error) = queue_prompt_button_webcontent_recovery(app, instance_id, attempt) {
            eprintln!(
                "Failed to queue prompt button WebContent recovery attempt {}: {error}",
                attempt + 1
            );
        }
    });
    Ok(())
}

fn handle_prompt_button_webcontent_termination<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    label: &str,
) -> bool {
    if !is_prompt_button_webview(label) {
        return false;
    }
    let renderer_state = app.state::<PromptButtonRendererState>();
    let instance_id = renderer_state.allocate_instance();
    match schedule_prompt_button_webcontent_recovery(app.clone(), instance_id, 0) {
        Ok(()) => true,
        Err(error) => {
            eprintln!("Failed to start prompt button WebContent recovery: {error}");
            false
        }
    }
}

#[cfg(debug_assertions)]
#[tauri::command]
fn simulate_prompt_button_webcontent_termination(app: tauri::AppHandle) -> bool {
    handle_prompt_button_webcontent_termination(&app, crate::windows::BUTTON_WINDOW_LABEL)
}

impl PromptButtonVisibilityState {
    fn new(visible: bool) -> Self {
        Self {
            desired_visible: std::sync::atomic::AtomicBool::new(visible),
            generation: std::sync::atomic::AtomicU64::new(0),
        }
    }

    fn from_settings(settings: &serde_json::Value) -> Self {
        let visible = settings
            .pointer("/floatingButton/visible")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(true);
        Self::new(visible)
    }

    pub(crate) fn desired_visible(&self) -> bool {
        self.desired_visible
            .load(std::sync::atomic::Ordering::Acquire)
    }

    fn generation(&self) -> u64 {
        self.generation.load(std::sync::atomic::Ordering::Acquire)
    }

    fn set(&self, visible: bool) -> u64 {
        self.desired_visible
            .store(visible, std::sync::atomic::Ordering::Release);
        self.generation
            .fetch_add(1, std::sync::atomic::Ordering::AcqRel)
            + 1
    }

    fn may_show(&self, requested_generation: u64) -> bool {
        self.desired_visible() && self.generation() == requested_generation
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PromptButtonEnsureAction {
    None,
    ShowExisting,
    BuildMissing,
}

fn prompt_button_ensure_action(
    expected_visible: bool,
    window_present: bool,
    window_visible: bool,
    renderer_ready: bool,
) -> PromptButtonEnsureAction {
    if !expected_visible || (window_present && (!renderer_ready || window_visible)) {
        PromptButtonEnsureAction::None
    } else if window_present {
        PromptButtonEnsureAction::ShowExisting
    } else {
        PromptButtonEnsureAction::BuildMissing
    }
}

struct SettingsFileState {
    path: std::path::PathBuf,
    io_lock: std::sync::Mutex<()>,
}

impl SettingsFileState {
    fn new(path: std::path::PathBuf) -> Self {
        Self {
            path,
            io_lock: std::sync::Mutex::new(()),
        }
    }

    fn read_text(&self) -> Result<Option<String>, String> {
        let _guard = self.io_lock.lock().map_err(|e| e.to_string())?;
        match std::fs::read_to_string(&self.path) {
            Ok(contents) => Ok(Some(contents)),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error.to_string()),
        }
    }

    fn read_value(&self) -> serde_json::Value {
        let Ok(_guard) = self.io_lock.lock() else {
            return default_settings_value();
        };
        self.read_value_unlocked()
    }

    fn read_value_unlocked(&self) -> serde_json::Value {
        std::fs::read_to_string(&self.path)
            .ok()
            .and_then(|contents| serde_json::from_str(&contents).ok())
            .unwrap_or_else(default_settings_value)
    }

    fn write_value_unlocked(&self, settings: &serde_json::Value) -> Result<(), String> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let contents = serde_json::to_string_pretty(settings).map_err(|e| e.to_string())?;
        std::fs::write(&self.path, contents).map_err(|e| e.to_string())
    }

    fn write_frontend_text(
        &self,
        value: &str,
        visibility_state: &PromptButtonVisibilityState,
    ) -> Result<(), String> {
        let mut settings: serde_json::Value =
            serde_json::from_str(value).map_err(|e| format!("Invalid settings JSON: {e}"))?;
        if !settings.is_object() {
            return Err("Settings JSON must be an object.".to_string());
        }
        if !settings
            .get("floatingButton")
            .is_some_and(serde_json::Value::is_object)
        {
            settings["floatingButton"] = serde_json::json!({});
        }
        let _guard = self.io_lock.lock().map_err(|e| e.to_string())?;
        settings["floatingButton"]["visible"] =
            serde_json::Value::Bool(visibility_state.desired_visible());
        self.write_value_unlocked(&settings)
    }

    fn patch_bool(&self, pointer: &[&str], value: bool) -> Result<(), String> {
        let _guard = self.io_lock.lock().map_err(|e| e.to_string())?;
        let mut settings = self.read_value_unlocked();
        if !settings.is_object() {
            settings = default_settings_value();
        }
        let (section, key) = (pointer[0], pointer[1]);
        if !settings
            .get(section)
            .is_some_and(serde_json::Value::is_object)
        {
            settings[section] = serde_json::json!({});
        }
        settings[section][key] = serde_json::Value::Bool(value);
        self.write_value_unlocked(&settings)
    }
}

fn record_prompt_pick_session_target_if_valid(
    state: &PromptPickSessionState,
    target: PromptPickSessionTarget,
    identity: CapturedTargetIdentity,
    session_id: u64,
) -> Option<FrontmostApp> {
    if !is_usable_autosend_app(&target.app) {
        state.clear_if_current(session_id);
        return None;
    }

    let app = target.app.clone();
    state
        .set_captured_if_current(session_id, target, identity)
        .then_some(app)
}

fn captured_identity_for_target(
    target: &PromptPickSessionTarget,
    recent_identity: Option<&CapturedTargetIdentity>,
) -> Option<CapturedTargetIdentity> {
    let pid = target.pid?;
    if let Some(identity) = recent_identity.filter(|identity| {
        identity.application.bundle_id == target.app.bundle_id
            && identity.application.main_pid == pid
            && target_application_identity_is_current(&identity.application)
    }) {
        return Some(identity.clone());
    }

    Some(CapturedTargetIdentity {
        application: TargetApplicationIdentity {
            bundle_id: target.app.bundle_id.clone(),
            main_pid: pid,
            launch_identity: platform::macos::process_launch_identity(pid)?,
        },
        window: None,
    })
}

fn target_application_identity_is_current(identity: &TargetApplicationIdentity) -> bool {
    target_application_identity_matches(
        identity,
        platform::macos::process_launch_identity(identity.main_pid),
    )
}

fn target_application_identity_matches(
    identity: &TargetApplicationIdentity,
    current: Option<platform::ProcessLaunchIdentity>,
) -> bool {
    match current {
        Some(current) => current == identity.launch_identity,
        None => !cfg!(target_os = "macos"),
    }
}

fn window_identity_matches(
    captured: &TargetWindowIdentity,
    current: &TargetWindowIdentity,
) -> bool {
    const FRAME_TOLERANCE: f64 = 2.0;
    captured.owner_pid == current.owner_pid
        && captured.role == current.role
        && captured.title_hash == current.title_hash
        && (captured.frame.x - current.frame.x).abs() <= FRAME_TOLERANCE
        && (captured.frame.y - current.frame.y).abs() <= FRAME_TOLERANCE
        && (captured.frame.width - current.frame.width).abs() <= FRAME_TOLERANCE
        && (captured.frame.height - current.frame.height).abs() <= FRAME_TOLERANCE
}

fn bundle_requires_exact_window(bundle_id: &str) -> bool {
    matches!(
        bundle_id,
        "com.anthropic.claudefordesktop" | "com.tencent.xinWeChat"
    )
}

fn captured_target_identity_is_current(identity: &CapturedTargetIdentity) -> bool {
    target_application_identity_is_current(&identity.application)
        && (!bundle_requires_exact_window(&identity.application.bundle_id)
            || identity.window.is_some())
}

fn stale_target_outcome() -> AutosendOutcome {
    AutosendOutcome {
        copied: false,
        sent: false,
        completion: None,
        error: Some("The captured target app or window is no longer available.".to_string()),
        reason: Some(platform::macos::AutosendFailureReason::NoSafeTarget),
    }
}

fn prompt_pick_session_target(
    frontmost: Option<FrontmostAppWithPid>,
    recent_target: Option<LastInputTarget>,
) -> Option<PromptPickSessionTarget> {
    let frontmost = frontmost?;

    // Identify PP itself — by authoritative PID comparison first, then by the
    // pre-existing bundle-id / name heuristics as a secondary defense. When the
    // frontmost app is PP (e.g. our own popover became "front" per lsappinfo),
    // we must NOT use it as the autosend target. Instead we fall through to the
    // recent_target fallback below — returning early would drop that fallback
    // and surface "Copied. Paste manually." in the UI.
    let is_prompt_picker =
        frontmost.pid == Some(std::process::id()) || is_prompt_picker_app(&frontmost.app);

    if !is_prompt_picker && is_usable_autosend_app(&frontmost.app) {
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

    if !is_prompt_picker {
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
    if !allows_app_only_autosend(&app) {
        state.clear();
        return;
    };

    let click_point = if has_focused_input_frame(&target.frame) {
        Some(target_click_point_from_tuple(target.click_point))
    } else {
        state
            .get()
            .filter(|recent| recent.app.bundle_id == app.bundle_id)
            .filter(|recent| recent.pid == Some(target.pid))
            .filter(is_recent_prompt_target)
            .and_then(|recent| recent.click_point)
    };

    let Some(launch_identity) = platform::macos::process_launch_identity(target.pid) else {
        state.clear();
        return;
    };
    let identity = CapturedTargetIdentity {
        application: TargetApplicationIdentity {
            bundle_id: app.bundle_id.clone(),
            main_pid: target.pid,
            launch_identity,
        },
        window: Some(TargetWindowIdentity {
            owner_pid: target.pid,
            frame: target.window_frame.clone(),
            role: Some("AXWindow".to_string()),
            title_hash: None,
            cg_window_id: None,
        }),
    };

    state.set_captured(
        LastInputTarget {
            app,
            pid: Some(target.pid),
            observed_at_ms: now_ms(),
            click_point,
        },
        identity,
    );
}

fn record_last_app_if_valid(state: &LastInputTargetState, target: FrontmostAppWithPid) {
    if target.pid == Some(std::process::id()) || is_prompt_picker_app(&target.app) {
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
    let Some(pid) = target.pid else {
        state.clear();
        return;
    };
    let Some(launch_identity) = platform::macos::process_launch_identity(pid) else {
        state.clear();
        return;
    };
    let identity = CapturedTargetIdentity {
        application: TargetApplicationIdentity {
            bundle_id: target.app.bundle_id.clone(),
            main_pid: pid,
            launch_identity,
        },
        window: None,
    };
    state.set_captured(
        LastInputTarget {
            app: target.app,
            pid: Some(pid),
            observed_at_ms: now_ms(),
            click_point: None,
        },
        identity,
    );
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TargetFrontmostStatus {
    Target,
    PromptPicker,
    OtherOrUnknown,
}

fn classify_target_frontmost(
    target: &PromptPickSessionTarget,
    frontmost: Option<&FrontmostAppWithPid>,
) -> TargetFrontmostStatus {
    let Some(frontmost) = frontmost else {
        return TargetFrontmostStatus::OtherOrUnknown;
    };
    if captured_target_matches_frontmost(target, Some(frontmost)) {
        return TargetFrontmostStatus::Target;
    }
    if is_prompt_picker_app(&frontmost.app) {
        return TargetFrontmostStatus::PromptPicker;
    }
    TargetFrontmostStatus::OtherOrUnknown
}

fn autosend_diagnostic_line(
    stage: &str,
    target_bundle_id: Option<&str>,
    has_click_point: bool,
    frontmost_bundle_id: Option<&str>,
    classification: Option<TargetFrontmostStatus>,
) -> String {
    format!(
        "prompt-picker-autosend stage={} target={} has_click_point={} frontmost={} classification={}",
        stage,
        target_bundle_id.unwrap_or("unknown"),
        has_click_point,
        frontmost_bundle_id.unwrap_or("unknown"),
        classification
            .map(|status| format!("{:?}", status))
            .unwrap_or_else(|| "unknown".to_string())
    )
}

fn autosend_diagnostics_enabled() -> bool {
    std::env::var("PROMPT_PICKER_FOCUS_DIAGNOSTICS").is_ok()
}

fn emit_autosend_diagnostic(
    stage: &str,
    target: &PromptPickSessionTarget,
    frontmost: Option<&FrontmostAppWithPid>,
    classification: Option<TargetFrontmostStatus>,
) {
    if !autosend_diagnostics_enabled() {
        return;
    }
    eprintln!(
        "{}",
        autosend_diagnostic_line(
            stage,
            Some(&target.app.bundle_id),
            target.click_point.is_some(),
            frontmost.map(|app| app.app.bundle_id.as_str()),
            classification,
        )
    );
}

fn is_prompt_picker_app(app: &FrontmostApp) -> bool {
    app.bundle_id == "local.promptpicker.dev" || app.name == "Prompt Drawer"
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

fn target_click_point_from_tuple(point: (f64, f64)) -> TargetClickPoint {
    TargetClickPoint {
        x: point.0,
        y: point.1,
    }
}

fn point_inside_candidate(point: TargetClickPoint, frame: &CandidateInput) -> bool {
    point.x >= frame.x
        && point.x <= frame.x + frame.width
        && point.y >= frame.y
        && point.y <= frame.y + frame.height
}

fn choose_recovery_click_point(
    recorded_click_point: Option<TargetClickPoint>,
    pointer_click_point: Option<TargetClickPoint>,
    target_window_frame: Option<&CandidateInput>,
    fallback_click_point: Option<TargetClickPoint>,
) -> Option<TargetClickPoint> {
    recorded_click_point
        .or_else(|| {
            let point = pointer_click_point?;
            let frame = target_window_frame?;
            point_inside_candidate(point, frame).then_some(point)
        })
        .or(fallback_click_point)
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
            show_button: "显示 Prompt Drawer",
            hide_button: "隐藏 Prompt Drawer",
            open_accessibility: "打开辅助功能设置",
            quit: "退出 Prompt Drawer",
        },
        _ => MenuLabels {
            open_main: "Manage Prompts...",
            open_settings: "Settings...",
            show_button: "Show Prompt Drawer",
            hide_button: "Hide Prompt Drawer",
            open_accessibility: "Open Accessibility Settings",
            quit: "Quit Prompt Drawer",
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

fn set_accessibility_prompt_requested(
    settings_state: &SettingsFileState,
    requested: bool,
) -> Result<(), String> {
    settings_state.patch_bool(&["permissions", "accessibilityPromptRequested"], requested)
}

#[tauri::command]
fn read_settings_text(
    settings_state: tauri::State<SettingsFileState>,
) -> Result<Option<String>, String> {
    settings_state.read_text()
}

#[tauri::command]
fn write_settings_text(
    value: String,
    settings_state: tauri::State<SettingsFileState>,
    visibility_state: tauri::State<PromptButtonVisibilityState>,
) -> Result<(), String> {
    settings_state.write_frontend_text(&value, visibility_state.inner())
}

#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct PromptButtonVisibilityOutcome {
    visible: bool,
    applied: bool,
    persisted: bool,
    error: Option<String>,
}

fn apply_prompt_button_visibility(
    visible: bool,
    app: &tauri::AppHandle,
    settings_state: &SettingsFileState,
    visibility_state: &PromptButtonVisibilityState,
) -> PromptButtonVisibilityOutcome {
    visibility_state.set(visible);

    let runtime_result = if visible {
        let position = prompt_button_position_cmd(app.clone()).ok().flatten();
        let (x, y) = position
            .map(|point| (point.x, point.y))
            .unwrap_or_else(|| startup_prompt_button_position(settings_state));
        show_prompt_button(x, y, app.clone())
    } else {
        hide_prompt_popover(app.clone()).and_then(|_| hide_prompt_button(app.clone()))
    };
    let persistence_result = settings_state.patch_bool(&["floatingButton", "visible"], visible);
    let _ = app.emit("prompt-button-visibility-changed", visible);

    let error = match (&runtime_result, &persistence_result) {
        (Err(runtime), Err(persist)) => Some(format!("{runtime}; {persist}")),
        (Err(runtime), Ok(())) => Some(runtime.clone()),
        (Ok(()), Err(persist)) => Some(persist.clone()),
        (Ok(()), Ok(())) => None,
    };

    PromptButtonVisibilityOutcome {
        visible,
        applied: runtime_result.is_ok(),
        persisted: persistence_result.is_ok(),
        error,
    }
}

#[tauri::command]
fn set_prompt_button_visibility(
    visible: bool,
    app: tauri::AppHandle,
    settings_state: tauri::State<SettingsFileState>,
    visibility_state: tauri::State<PromptButtonVisibilityState>,
) -> PromptButtonVisibilityOutcome {
    apply_prompt_button_visibility(
        visible,
        &app,
        settings_state.inner(),
        visibility_state.inner(),
    )
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

fn startup_prompt_button_position(settings_state: &SettingsFileState) -> (f64, f64) {
    let fallback = (960.0, 700.0);
    let Ok(Some(contents)) = settings_state.read_text() else {
        return fallback;
    };
    parse_saved_button_position(&contents).unwrap_or(fallback)
}

fn setup_menu_bar_app(app_handle: &tauri::AppHandle) -> Result<(), String> {
    let settings_state = app_handle.state::<SettingsFileState>();
    let menu = build_menu_bar_menu(app_handle, settings_language(&settings_state.read_value()))?;

    let tray_builder = TrayIconBuilder::with_id(TRAY_ID)
        .menu(&menu)
        .tooltip("Prompt Drawer")
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
                let settings_state = app.state::<SettingsFileState>();
                let visibility_state = app.state::<PromptButtonVisibilityState>();
                let _ = apply_prompt_button_visibility(
                    true,
                    app,
                    settings_state.inner(),
                    visibility_state.inner(),
                );
            }
            TrayMenuAction::HideFloatingButton => {
                let settings_state = app.state::<SettingsFileState>();
                let visibility_state = app.state::<PromptButtonVisibilityState>();
                let _ = apply_prompt_button_visibility(
                    false,
                    app,
                    settings_state.inner(),
                    visibility_state.inner(),
                );
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
    let builder = tauri::Builder::default()
        .manage(LastInputTargetState::default())
        .manage(PromptPickSessionState::default())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_shell::init())
        .on_page_load(|webview, payload| {
            if is_prompt_button_webview(webview.label())
                && payload.event() == tauri::webview::PageLoadEvent::Started
            {
                if let Some(window) = webview.app_handle().get_webview_window(webview.label()) {
                    let _ = window.hide();
                }
            }
        });

    #[cfg(target_os = "macos")]
    let builder = builder.on_web_content_process_terminate(|webview| {
        handle_prompt_button_webcontent_termination(webview.app_handle(), webview.label());
    });

    builder
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
            paste_prompt_and_submit_to_last_target,
            paste_prompt_sequence_and_submit_to_last_target,
            show_prompt_button,
            hide_prompt_button,
            show_prompt_popover,
            hide_prompt_popover,
            show_prompt_popover_from_button,
            toggle_prompt_popover_from_button,
            show_prompt_button_controls_from_button,
            acknowledge_prompt_popover_mode,
            prompt_button_position_cmd,
            move_prompt_button_to,
            open_main_window,
            open_settings_window,
            set_menu_language,
            quit_prompt_picker,
            read_prompt_library_file,
            write_prompt_library_file,
            prompt_library_file_metadata,
            read_settings_text,
            write_settings_text,
            set_prompt_button_visibility,
            set_prompt_button_renderer_ready,
            #[cfg(debug_assertions)]
            calico_probe::record_calico_surface_probe,
            #[cfg(debug_assertions)]
            simulate_prompt_button_webcontent_termination
        ])
        .setup(|app| {
            #[cfg(debug_assertions)]
            if calico_probe::enabled() {
                return calico_probe::setup(app.handle()).map_err(Into::into);
            }

            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            let path = settings_path(app.handle())
                .ok_or_else(|| "Could not resolve settings path.".to_string())?;
            let settings_state = SettingsFileState::new(path);
            let initial_settings = settings_state.read_value();
            let visibility_state = PromptButtonVisibilityState::from_settings(&initial_settings);
            app.manage(settings_state);
            app.manage(visibility_state);
            app.manage(PromptButtonRendererState::default());
            app.manage(PromptButtonRecoveryUrlState::default());
            app.manage(crate::windows::PopoverModeRequestState::default());

            setup_menu_bar_app(app.handle())?;

            let window = app.get_webview_window("main").unwrap();
            window.set_title("Prompt Drawer").unwrap();
            let main_window = window.clone();
            window.on_window_event(move |event| {
                if let WindowEvent::CloseRequested { api, .. } = event {
                    api.prevent_close();
                    let _ = main_window.hide();
                }
            });
            let visibility_state = app.state::<PromptButtonVisibilityState>();
            if visibility_state.desired_visible() {
                let settings_state = app.state::<SettingsFileState>();
                let (x, y) = startup_prompt_button_position(settings_state.inner());
                let _ = show_prompt_button(x, y, app.handle().clone());
            }

            let monitor_app = app.handle().clone();
            std::thread::spawn(move || loop {
                std::thread::sleep(std::time::Duration::from_secs(15));

                let visibility = monitor_app.state::<PromptButtonVisibilityState>();
                let expected_visible = visibility.desired_visible();
                let requested_generation = visibility.generation();
                let button = monitor_app.get_webview_window(crate::windows::BUTTON_WINDOW_LABEL);
                let renderer_ready = monitor_app.state::<PromptButtonRendererState>().is_ready();
                let action = prompt_button_ensure_action(
                    expected_visible,
                    button.is_some(),
                    button
                        .as_ref()
                        .and_then(|window| window.is_visible().ok())
                        .unwrap_or(false),
                    renderer_ready,
                );
                if action == PromptButtonEnsureAction::None {
                    continue;
                }

                let ensure_app = monitor_app.clone();
                let _ = monitor_app.run_on_main_thread(move || {
                    let visibility = ensure_app.state::<PromptButtonVisibilityState>();
                    if !visibility.may_show(requested_generation) {
                        return;
                    }
                    let settings_state = ensure_app.state::<SettingsFileState>();
                    let (x, y) = startup_prompt_button_position(settings_state.inner());
                    let _ = show_prompt_button(x, y, ensure_app.clone());
                });
            });
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod prompt_button_renderer_tests {
    use super::*;

    #[test]
    fn allocates_nonzero_instances_and_resets_readiness() {
        let state = PromptButtonRendererState::default();
        let first = state.allocate_instance();
        let visibility = PromptButtonVisibilityState::new(true);
        assert!(state.accept(first, 1, true, &visibility).is_some());

        let second = state.allocate_instance();

        assert_ne!(first, second);
        assert_ne!(second, 0);
        assert!(!state.is_ready());
        assert_eq!(state.current(), (second, 0, false));
    }

    #[test]
    fn rejects_stale_instances_and_out_of_order_transitions() {
        let state = PromptButtonRendererState::default();
        let visibility = PromptButtonVisibilityState::new(true);
        let old = state.allocate_instance();
        let current = state.allocate_instance();

        assert!(state.accept(old, 1, true, &visibility).is_none());
        assert!(state.accept(current, 2, true, &visibility).is_some());
        assert!(state.accept(current, 1, false, &visibility).is_none());
        assert_eq!(state.current(), (current, 2, true));
    }

    #[test]
    fn newer_transition_invalidates_an_older_lock_free_action() {
        let state = PromptButtonRendererState::default();
        let visibility = PromptButtonVisibilityState::new(true);
        let instance = state.allocate_instance();
        let old_hide = state.accept(instance, 1, false, &visibility).unwrap();
        let new_show = state.accept(instance, 2, true, &visibility).unwrap();

        assert!(!state.action_is_current(old_hide, &visibility));
        assert!(state.action_is_current(new_show, &visibility));
    }

    #[test]
    fn close_racing_with_ready_true_prevents_show() {
        let state = PromptButtonRendererState::default();
        let visibility = PromptButtonVisibilityState::new(true);
        let instance = state.allocate_instance();
        let show = state.accept(instance, 1, true, &visibility).unwrap();

        visibility.set(false);

        assert!(!state.action_is_current(show, &visibility));
    }

    #[test]
    fn only_prompt_button_termination_is_owned_by_this_recovery() {
        assert!(is_prompt_button_webview(
            crate::windows::BUTTON_WINDOW_LABEL
        ));
        assert!(!is_prompt_button_webview(
            crate::windows::POPOVER_WINDOW_LABEL
        ));
        assert!(!is_prompt_button_webview("main"));
    }

    #[test]
    fn termination_recovery_hides_and_navigates_the_existing_window() {
        let app = tauri::test::mock_builder()
            .build(tauri::test::mock_context(tauri::test::noop_assets()))
            .unwrap();
        app.manage(PromptButtonRendererState::default());
        app.manage(PromptButtonRecoveryUrlState::default());
        let window = tauri::WebviewWindowBuilder::new(
            &app,
            crate::windows::BUTTON_WINDOW_LABEL,
            tauri::WebviewUrl::App("overlay.html?rendererInstanceId=1".into()),
        )
        .visible(true)
        .build()
        .unwrap();
        app.state::<PromptButtonRecoveryUrlState>()
            .store(window.url().unwrap());
        let instance_id = app.state::<PromptButtonRendererState>().allocate_instance();

        recover_prompt_button_webview_once(app.handle(), instance_id).unwrap();

        let recovered = app
            .get_webview_window(crate::windows::BUTTON_WINDOW_LABEL)
            .unwrap();
        let url = recovered.url().unwrap();
        assert_eq!(url.path(), "/overlay.html");
        assert_eq!(
            url.query_pairs()
                .find(|(key, _)| key == "rendererInstanceId")
                .map(|(_, value)| value.into_owned()),
            Some(instance_id.to_string())
        );
    }

    #[test]
    fn termination_recovery_url_falls_back_to_the_stored_creation_url() {
        let hidden = std::cell::Cell::new(false);
        let navigated = std::cell::RefCell::new(None);
        let stored =
            tauri::Url::parse("tauri://localhost/overlay.html?rendererInstanceId=1&source=stored")
                .unwrap();

        let recovered = perform_prompt_button_webview_recovery(
            Some(stored),
            7,
            || {
                hidden.set(true);
                Ok(())
            },
            || Err("terminated webview URL unavailable".to_string()),
            |url| {
                *navigated.borrow_mut() = Some(url);
                Ok(())
            },
        )
        .unwrap();

        assert!(hidden.get());
        assert_eq!(navigated.borrow().as_ref(), Some(&recovered));
        assert_eq!(recovered.path(), "/overlay.html");
        assert_eq!(recovered.query(), Some("rendererInstanceId=7"));
        assert!(prompt_button_recovery_url(Err("unavailable".into()), None, 7).is_err());
    }

    #[test]
    fn termination_recovery_has_three_bounded_attempts() {
        assert_eq!(prompt_button_termination_retry_delay(0), Some(0));
        assert_eq!(prompt_button_termination_retry_delay(1), Some(100));
        assert_eq!(prompt_button_termination_retry_delay(2), Some(400));
        assert_eq!(prompt_button_termination_retry_delay(3), None);
    }

    #[test]
    fn termination_recovery_propagates_hide_and_navigation_failures() {
        let stored = tauri::Url::parse("tauri://localhost/overlay.html").unwrap();
        let hide_error = perform_prompt_button_webview_recovery(
            Some(stored.clone()),
            2,
            || Err("hide failed".to_string()),
            || Ok(stored.clone()),
            |_| Ok(()),
        )
        .unwrap_err();
        assert!(hide_error.contains("hide failed"));

        let navigate_error = perform_prompt_button_webview_recovery(
            Some(stored.clone()),
            2,
            || Ok(()),
            || Ok(stored),
            |_| Err("navigate failed".to_string()),
        )
        .unwrap_err();
        assert!(navigate_error.contains("navigate failed"));
    }
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

    fn captured_identity(bundle_id: &str, pid: u32) -> CapturedTargetIdentity {
        CapturedTargetIdentity {
            application: TargetApplicationIdentity {
                bundle_id: bundle_id.to_string(),
                main_pid: pid,
                launch_identity: platform::macos::process_launch_identity(pid)
                    .expect("test process should have launch metadata"),
            },
            window: None,
        }
    }

    #[test]
    fn target_application_identity_rejects_restarted_process() {
        let identity = TargetApplicationIdentity {
            bundle_id: "com.anthropic.claudefordesktop".to_string(),
            main_pid: 42,
            launch_identity: platform::ProcessLaunchIdentity {
                seconds: 100,
                microseconds: 200,
            },
        };

        assert!(target_application_identity_matches(
            &identity,
            Some(platform::ProcessLaunchIdentity {
                seconds: 100,
                microseconds: 200,
            })
        ));
        assert!(!target_application_identity_matches(
            &identity,
            Some(platform::ProcessLaunchIdentity {
                seconds: 101,
                microseconds: 0,
            })
        ));
    }

    #[test]
    fn target_window_identity_uses_owner_and_tolerant_fingerprint() {
        let captured = TargetWindowIdentity {
            owner_pid: 42,
            frame: CandidateInput {
                x: 10.0,
                y: 20.0,
                width: 800.0,
                height: 600.0,
            },
            role: Some("AXWindow".to_string()),
            title_hash: Some("abc".to_string()),
            cg_window_id: None,
        };
        let mut current = captured.clone();
        current.frame.x += 1.5;
        assert!(window_identity_matches(&captured, &current));

        current.frame.x += 1.0;
        assert!(!window_identity_matches(&captured, &current));
        current = captured.clone();
        current.owner_pid = 43;
        assert!(!window_identity_matches(&captured, &current));
    }

    #[test]
    fn claude_and_wechat_require_window_identity_but_codex_does_not() {
        assert!(bundle_requires_exact_window(
            "com.anthropic.claudefordesktop"
        ));
        assert!(bundle_requires_exact_window("com.tencent.xinWeChat"));
        assert!(!bundle_requires_exact_window("com.openai.codex"));
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
        assert_eq!(state.get().unwrap().pid, None);
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
    fn begin_if_new_preserves_target_for_current_session() {
        let state = PromptPickSessionState::default();
        state.begin(7);
        state.set(PromptPickSessionTarget {
            app: FrontmostApp {
                name: "Codex".to_string(),
                bundle_id: "com.openai.codex".to_string(),
            },
            pid: None,
            observed_at_ms: now_ms(),
            click_point: Some(TargetClickPoint { x: 640.0, y: 720.0 }),
        });

        state.begin_if_new(7);
        assert_eq!(state.get().unwrap().app.bundle_id, "com.openai.codex");

        state.begin_if_new(8);
        assert!(state.get().is_none());
    }

    #[test]
    fn autosend_and_prompt_capture_commands_use_spawn_blocking() {
        let source = include_str!("lib.rs");

        assert!(source.contains("async fn begin_prompt_pick_session"));
        assert!(source.contains("async fn paste_prompt_and_submit_to_last_target"));
        assert!(source.contains("async fn paste_prompt_sequence_and_submit_to_last_target"));
        assert!(
            source
                .matches("tauri::async_runtime::spawn_blocking(move ||")
                .count()
                >= 3
        );
    }

    #[test]
    fn prompt_capture_command_begins_session_before_recording_target() {
        let source = include_str!("lib.rs");
        let start = source
            .find("async fn begin_prompt_pick_session")
            .expect("begin_prompt_pick_session should exist");
        let end = source[start..]
            .find("#[tauri::command]\nfn paste_prompt")
            .expect("next command should follow begin_prompt_pick_session");
        let command_source = &source[start..start + end];

        assert!(command_source.contains("session_state.begin(session_id);"));
        assert!(command_source.contains("record_prompt_pick_session_target_if_valid"));
    }

    #[test]
    fn autosend_command_impls_use_activating_session_sender() {
        let source = include_str!("lib.rs");
        let single_start = source
            .find("fn paste_prompt_and_submit_to_last_target_impl")
            .expect("single autosend impl should exist");
        let single_end = source[single_start..]
            .find("fn copy_text_to_clipboard")
            .expect("copy helper should follow single autosend impl");
        let single_source = &source[single_start..single_start + single_end];

        assert!(single_source.contains("paste_prompt_and_submit_to_session_target_with_senders"));
        assert!(single_source.contains("paste_prompt_and_submit_to_app_clipboard_with_copier"));
        assert!(!single_source.contains("focus_preserving_prompt_to_last_target_impl("));

        let sequence_start = source
            .find("fn paste_prompt_sequence_and_submit_to_last_target_impl")
            .expect("sequence autosend impl should exist");
        let sequence_end = source[sequence_start..]
            .find("fn paste_prompt_and_submit_to_last_target_impl")
            .expect("single autosend impl should follow sequence autosend impl");
        let sequence_source = &source[sequence_start..sequence_start + sequence_end];

        assert!(sequence_source
            .contains("paste_prompt_sequence_and_submit_to_session_target_with_senders"));
        assert!(sequence_source.contains("paste_prompt_and_submit_to_app_clipboard_with_copier"));
        assert!(!sequence_source.contains("focus_preserving_prompt_sequence_to_last_target_impl("));
    }

    #[test]
    fn legacy_activating_paste_commands_are_not_registered() {
        let source = include_str!("lib.rs");
        let handler_start = source
            .find("tauri::generate_handler![")
            .expect("invoke handler should be registered");
        let handler_rest = &source[handler_start..];
        let handler_end = handler_rest
            .find("])")
            .expect("invoke handler should close");
        let handler_source = &handler_rest[..handler_end];

        assert!(!handler_source.contains(concat!("paste_prompt", "_to_app,")));
        assert!(!handler_source.contains(concat!("paste_prompt", "_to_last_target,")));
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
            menu_labels_for_language("zh-CN").show_button,
            "显示 Prompt Drawer"
        );
        assert_eq!(
            menu_labels_for_language("zh-CN").hide_button,
            "隐藏 Prompt Drawer"
        );
        assert_eq!(
            menu_labels_for_language("en-US").open_main,
            "Manage Prompts..."
        );
        assert_eq!(
            menu_labels_for_language("en-US").show_button,
            "Show Prompt Drawer"
        );
        assert_eq!(
            menu_labels_for_language("en-US").hide_button,
            "Hide Prompt Drawer"
        );
        assert_eq!(menu_labels_for_language("bad").quit, "Quit Prompt Drawer");
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
            pid: std::process::id(),
        };

        record_last_input_target_if_valid(&state, &target);

        assert_eq!(state.get().unwrap().app.bundle_id, "com.apple.Notes");
        assert_eq!(state.get().unwrap().pid, Some(std::process::id()));
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
            pid: std::process::id(),
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
            pid: std::process::id(),
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
            pid: std::process::id(),
        };

        record_last_input_target_if_valid(&state, &target);

        assert!(state.get().is_none());
    }

    #[test]
    fn records_recovery_fallback_target_without_focused_input_frame() {
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
                name: "Claude".to_string(),
                bundle_id: "com.anthropic.claudefordesktop".to_string(),
            }),
            pid: std::process::id(),
        };

        record_last_input_target_if_valid(&state, &target);

        assert_eq!(
            state.get().unwrap().app.bundle_id,
            "com.anthropic.claudefordesktop"
        );
        assert!(state.get().unwrap().click_point.is_none());
    }

    #[test]
    fn recovery_click_point_prefers_recorded_input_point_over_pointer_fallback() {
        let recorded = Some(TargetClickPoint { x: 400.0, y: 700.0 });
        let pointer = Some(TargetClickPoint { x: 100.0, y: 100.0 });
        let window = CandidateInput {
            x: 0.0,
            y: 0.0,
            width: 1000.0,
            height: 1000.0,
        };

        assert_eq!(
            choose_recovery_click_point(recorded, pointer, Some(&window), None),
            recorded
        );
    }

    #[test]
    fn recovery_click_point_uses_pointer_only_inside_target_window() {
        let pointer = Some(TargetClickPoint { x: 100.0, y: 100.0 });
        let fallback = Some(TargetClickPoint { x: 500.0, y: 735.0 });
        let window = CandidateInput {
            x: 0.0,
            y: 0.0,
            width: 200.0,
            height: 200.0,
        };

        assert_eq!(
            choose_recovery_click_point(None, pointer, Some(&window), fallback),
            pointer
        );
    }

    #[test]
    fn recovery_click_point_uses_pointer_before_generic_fallback() {
        let pointer = Some(TargetClickPoint { x: 120.0, y: 140.0 });
        let fallback = Some(TargetClickPoint { x: 500.0, y: 735.0 });
        let window = CandidateInput {
            x: 100.0,
            y: 100.0,
            width: 600.0,
            height: 500.0,
        };

        assert_eq!(
            choose_recovery_click_point(None, pointer, Some(&window), fallback),
            pointer
        );
    }

    #[test]
    fn recovery_click_point_rejects_pointer_outside_target_window() {
        let pointer = Some(TargetClickPoint { x: 300.0, y: 300.0 });
        let fallback = Some(TargetClickPoint { x: 500.0, y: 735.0 });
        let window = CandidateInput {
            x: 0.0,
            y: 0.0,
            width: 200.0,
            height: 200.0,
        };

        assert_eq!(
            choose_recovery_click_point(None, pointer, Some(&window), fallback),
            fallback
        );
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
                name: "Prompt Drawer".to_string(),
                bundle_id: "local.promptpicker.dev".to_string(),
            }),
            pid: std::process::id(),
        };

        record_last_input_target_if_valid(&state, &target);

        assert!(state.get().is_none());
    }

    #[test]
    fn prompt_pick_session_uses_frontmost_business_app() {
        let target = prompt_pick_session_target(
            Some(frontmost_target(
                "WeChat",
                "com.tencent.xinWeChat",
                Some(123),
            )),
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
    fn classifies_frontmost_target_status() {
        let target = PromptPickSessionTarget {
            app: FrontmostApp {
                name: "Codex".to_string(),
                bundle_id: "com.openai.codex".to_string(),
            },
            pid: Some(123),
            observed_at_ms: now_ms(),
            click_point: None,
        };

        assert_eq!(
            classify_target_frontmost(
                &target,
                Some(&frontmost_target("Codex", "com.openai.codex", Some(123)))
            ),
            TargetFrontmostStatus::Target
        );
        assert_eq!(
            classify_target_frontmost(
                &target,
                Some(&frontmost_target(
                    "Prompt Drawer",
                    "local.promptpicker.dev",
                    Some(1)
                ))
            ),
            TargetFrontmostStatus::PromptPicker
        );
        assert_eq!(
            classify_target_frontmost(
                &target,
                Some(&frontmost_target("Notes", "com.apple.Notes", Some(456)))
            ),
            TargetFrontmostStatus::OtherOrUnknown
        );
        assert_eq!(
            classify_target_frontmost(&target, None),
            TargetFrontmostStatus::OtherOrUnknown
        );
    }

    #[test]
    fn autosend_diagnostic_line_includes_classification_and_click_point_state() {
        let line = autosend_diagnostic_line(
            "before-paste",
            Some("com.openai.codex"),
            true,
            Some("local.promptpicker.dev"),
            Some(TargetFrontmostStatus::PromptPicker),
        );

        assert!(line.contains("before-paste"));
        assert!(line.contains("target=com.openai.codex"));
        assert!(line.contains("has_click_point=true"));
        assert!(line.contains("frontmost=local.promptpicker.dev"));
        assert!(line.contains("classification=PromptPicker"));
        assert!(!line.contains("prompt body"));
    }

    #[test]
    fn submit_key_parser_accepts_known_values() {
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
    fn submit_key_parser_rejects_unknown_values() {
        assert_eq!(
            native_submit_key_from_arg(Some("space".to_string())).unwrap_err(),
            "Invalid submit key: space"
        );
    }

    #[test]
    fn global_submit_behavior_fails_closed_when_settings_cannot_be_read() {
        assert_eq!(
            authoritative_submit_key(Err("settings unavailable".to_string())),
            platform::macos::NativeSubmitKey::None
        );
        assert_eq!(
            authoritative_submit_key(Ok(Some("not json".to_string()))),
            platform::macos::NativeSubmitKey::None
        );
        assert_eq!(
            authoritative_submit_key(Ok(None)),
            platform::macos::NativeSubmitKey::None
        );
    }

    #[test]
    fn global_submit_behavior_reads_all_supported_settings() {
        let settings_for = |mode| {
            Ok(Some(
                serde_json::json!({
                    "promptInsertion": { "mode": mode }
                })
                .to_string(),
            ))
        };

        assert_eq!(
            authoritative_submit_key(settings_for("paste_only")),
            platform::macos::NativeSubmitKey::None
        );
        assert_eq!(
            authoritative_submit_key(settings_for("paste_enter")),
            platform::macos::NativeSubmitKey::Enter
        );
        assert_eq!(
            authoritative_submit_key(settings_for("paste_command_enter")),
            platform::macos::NativeSubmitKey::CommandEnter
        );
        assert_eq!(
            authoritative_submit_key(settings_for("paste_and_submit")),
            platform::macos::NativeSubmitKey::Enter
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
            |_| Err("repair unavailable".to_string()),
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
    fn target_recovery_can_recover_before_paste_if_picker_is_frontmost() {
        let target = prompt_target("WeChat", "com.tencent.xinWeChat", Some(123));
        let events = RefCell::new(Vec::new());
        let mut frontmost = VecDeque::from([
            frontmost_target("Prompt Drawer", "local.promptpicker.dev", Some(1)),
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
            |_| {
                events.borrow_mut().push("recover");
                Ok(())
            },
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

        assert!(outcome.sent);
        assert_eq!(
            &*events.borrow(),
            &["copy", "recover", "paste", "sleep", "submit"]
        );
    }

    #[test]
    fn autosend_recovers_app_only_target_with_recorded_click_point_when_picker_is_frontmost() {
        let target = PromptPickSessionTarget {
            app: FrontmostApp {
                name: "Codex".to_string(),
                bundle_id: "com.openai.codex".to_string(),
            },
            pid: None,
            observed_at_ms: now_ms(),
            click_point: Some(TargetClickPoint { x: 640.0, y: 720.0 }),
        };
        let mut frontmost = vec![
            Some(frontmost_target(
                "Prompt Drawer",
                "local.promptpicker.dev",
                Some(1),
            )),
            Some(frontmost_target("Codex", "com.openai.codex", Some(42))),
            Some(frontmost_target("Codex", "com.openai.codex", Some(42))),
        ]
        .into_iter();
        let recovered = RefCell::new(false);
        let pasted = RefCell::new(false);
        let submitted = RefCell::new(false);

        let outcome = guarded_focus_preserving_autosend_with_senders(
            "hello",
            &target,
            platform::macos::NativeSubmitKey::Enter,
            |_| Ok(()),
            || frontmost.next().flatten(),
            |recover_target| {
                recovered.replace(true);
                assert_eq!(recover_target.app.bundle_id, "com.openai.codex");
                assert_eq!(recover_target.click_point.unwrap().x, 640.0);
                Ok(())
            },
            || {
                pasted.replace(true);
                Ok(())
            },
            |_| {
                submitted.replace(true);
                Ok(())
            },
            |_| {},
        );

        assert!(outcome.sent);
        assert!(*recovered.borrow());
        assert!(*pasted.borrow());
        assert!(*submitted.borrow());
    }

    #[test]
    fn autosend_refuses_when_another_app_is_frontmost_before_paste() {
        let target = PromptPickSessionTarget {
            app: FrontmostApp {
                name: "Codex".to_string(),
                bundle_id: "com.openai.codex".to_string(),
            },
            pid: Some(42),
            observed_at_ms: now_ms(),
            click_point: None,
        };
        let outcome = guarded_focus_preserving_autosend_with_senders(
            "hello",
            &target,
            platform::macos::NativeSubmitKey::Enter,
            |_| Ok(()),
            || Some(frontmost_target("Notes", "com.apple.Notes", Some(9))),
            |_| panic!("must not recover when a third-party app is frontmost"),
            || panic!("must not paste into the wrong app"),
            |_| panic!("must not submit into the wrong app"),
            |_| {},
        );

        assert_eq!(outcome.reason, Some(AutosendFailureReason::NoSafeTarget));
        assert!(outcome.copied);
        assert!(!outcome.sent);
    }

    #[test]
    fn autosend_refuses_when_prompt_picker_recovery_does_not_restore_target() {
        let target = PromptPickSessionTarget {
            app: FrontmostApp {
                name: "Codex".to_string(),
                bundle_id: "com.openai.codex".to_string(),
            },
            pid: Some(42),
            observed_at_ms: now_ms(),
            click_point: None,
        };
        let mut frontmost = vec![
            Some(frontmost_target(
                "Prompt Drawer",
                "local.promptpicker.dev",
                Some(1),
            )),
            Some(frontmost_target(
                "Prompt Drawer",
                "local.promptpicker.dev",
                Some(1),
            )),
        ]
        .into_iter();
        let outcome = guarded_focus_preserving_autosend_with_senders(
            "hello",
            &target,
            platform::macos::NativeSubmitKey::Enter,
            |_| Ok(()),
            || frontmost.next().flatten(),
            |_| Ok(()),
            || panic!("must not paste before target is restored"),
            |_| panic!("must not submit before target is restored"),
            |_| {},
        );

        assert_eq!(outcome.reason, Some(AutosendFailureReason::NoSafeTarget));
        assert!(outcome.copied);
        assert!(!outcome.sent);
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
            |_| Err("repair unavailable".to_string()),
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
            |_| {
                events.borrow_mut().push("repair");
                Err("repair must not run".to_string())
            },
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
        assert_eq!(
            *submitted.borrow(),
            Some(platform::macos::NativeSubmitKey::Enter)
        );
        assert_eq!(&*events.borrow(), &["copy", "paste", "sleep", "submit"]);
    }

    #[test]
    fn sequence_autosend_recovers_target_when_prompt_picker_is_frontmost() {
        let target = PromptPickSessionTarget {
            app: FrontmostApp {
                name: "Codex".to_string(),
                bundle_id: "com.openai.codex".to_string(),
            },
            pid: Some(42),
            observed_at_ms: now_ms(),
            click_point: Some(TargetClickPoint { x: 640.0, y: 720.0 }),
        };
        let bodies = vec!["one".to_string(), "two".to_string()];
        let mut frontmost = vec![
            Some(frontmost_target(
                "Prompt Drawer",
                "local.promptpicker.dev",
                Some(1),
            )),
            Some(frontmost_target("Codex", "com.openai.codex", Some(42))),
            Some(frontmost_target("Codex", "com.openai.codex", Some(42))),
            Some(frontmost_target("Codex", "com.openai.codex", Some(42))),
            Some(frontmost_target("Codex", "com.openai.codex", Some(42))),
        ]
        .into_iter();
        let recovered_count = RefCell::new(0);
        let pasted_count = RefCell::new(0);
        let submitted_count = RefCell::new(0);

        let outcome = focus_preserving_prompt_sequence_for_target_with_senders(
            &bodies,
            700,
            &target,
            platform::macos::NativeSubmitKey::Enter,
            |_| Ok(()),
            || frontmost.next().flatten(),
            |_| {
                *recovered_count.borrow_mut() += 1;
                Ok(())
            },
            || {
                *pasted_count.borrow_mut() += 1;
                Ok(())
            },
            |_| {
                *submitted_count.borrow_mut() += 1;
                Ok(())
            },
            |_| {},
        );

        assert!(outcome.sent);
        assert_eq!(outcome.sent_count, 2);
        assert_eq!(*recovered_count.borrow(), 1);
        assert_eq!(*pasted_count.borrow(), 2);
        assert_eq!(*submitted_count.borrow(), 2);
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
            |_| Err("repair unavailable".to_string()),
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
            |_| Err("repair unavailable".to_string()),
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
            |_| Err("repair unavailable".to_string()),
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
    fn prompt_pick_session_rejects_visible_app_without_recent_exact_target() {
        let target = prompt_pick_session_target(
            Some(frontmost_target(
                "Prompt Drawer",
                "local.promptpicker.dev",
                Some(1),
            )),
            None,
        );

        assert!(target.is_none());
    }

    #[test]
    fn prompt_pick_session_uses_recent_target_without_considering_visible_apps() {
        let target = prompt_pick_session_target(
            Some(frontmost_target(
                "Prompt Drawer",
                "local.promptpicker.dev",
                Some(1),
            )),
            Some(LastInputTarget {
                app: FrontmostApp {
                    name: "Codex".to_string(),
                    bundle_id: "com.openai.codex".to_string(),
                },
                pid: None,
                observed_at_ms: now_ms(),
                click_point: None,
            }),
        )
        .unwrap();

        assert_eq!(target.app.bundle_id, "com.openai.codex");
        assert!(target.click_point.is_none());
    }

    #[test]
    fn prompt_pick_session_uses_recent_target_when_prompt_picker_has_no_visible_app() {
        let target = prompt_pick_session_target(
            Some(frontmost_target(
                "Prompt Drawer",
                "local.promptpicker.dev",
                Some(1),
            )),
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
    fn prompt_pick_session_uses_recent_target_when_picker_is_frontmost() {
        let target = prompt_pick_session_target(
            Some(frontmost_target(
                "Prompt Drawer",
                "local.promptpicker.dev",
                Some(1),
            )),
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
    fn prompt_pick_session_uses_recent_target_when_frontmost_pid_is_picker_process() {
        // Regression: when PP's own popover becomes frontmost, lsappinfo may
        // report PP's PID (== std::process::id()) even if name/bundle look
        // foreign (binary launch mode). The function must NOT return None
        // early — it must fall through to the recent_target fallback so the
        // user's last used app (e.g. Codex) is picked.
        let target = prompt_pick_session_target(
            Some(frontmost_target(
                "UnknownApp",
                "unknown.bundle.id",
                Some(std::process::id()),
            )),
            Some(LastInputTarget {
                app: FrontmostApp {
                    name: "Codex".to_string(),
                    bundle_id: "com.openai.codex".to_string(),
                },
                pid: Some(456),
                observed_at_ms: now_ms(),
                click_point: None,
            }),
        )
        .unwrap();

        assert_eq!(target.app.bundle_id, "com.openai.codex");
        assert_eq!(target.pid, Some(456));
    }

    #[test]
    fn prompt_pick_session_rejects_visible_app_when_picker_pid_is_frontmost() {
        let target = prompt_pick_session_target(
            Some(frontmost_target(
                "UnknownApp",
                "unknown.bundle.id",
                Some(std::process::id()),
            )),
            None,
        );

        assert!(target.is_none());
    }

    #[test]
    fn autosend_without_last_target_copies_without_sending() {
        let state = PromptPickSessionState::default();
        let result = paste_prompt_and_submit_to_session_target_with_senders(
            "hello",
            &state,
            None,
            platform::macos::NativeSubmitKey::Enter,
            |_, _, _, _| panic!("app sender must not run without a target"),
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
            platform::macos::NativeSubmitKey::Enter,
            |body, bundle_id, click_point, submit_key| {
                assert_eq!(body, "hello");
                assert_eq!(bundle_id, "com.openai.codex");
                assert_eq!(click_point.unwrap().x, 640.0);
                assert_eq!(submit_key, platform::macos::NativeSubmitKey::Enter);
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
                pid: Some(std::process::id()),
                observed_at_ms: now_ms(),
                click_point: None,
            },
            captured_identity("com.tencent.xinWeChat", std::process::id()),
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
                pid: Some(std::process::id()),
                observed_at_ms: now_ms(),
                click_point: None,
            },
            captured_identity("com.tencent.xinWeChat", std::process::id()),
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
            platform::macos::NativeSubmitKey::Enter,
            |_, bundle_id, _, submit_key| {
                assert_eq!(bundle_id, "com.openai.codex");
                assert_eq!(submit_key, platform::macos::NativeSubmitKey::Enter);
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
            platform::macos::NativeSubmitKey::Enter,
            |body, bundle_id, click_point, submit_key| {
                assert_eq!(body, "hello");
                assert_eq!(bundle_id, "com.tencent.xinWeChat");
                assert_eq!(click_point.unwrap().y, 720.0);
                assert_eq!(submit_key, platform::macos::NativeSubmitKey::Enter);
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
            platform::macos::NativeSubmitKey::Enter,
            |body, bundle_id, click_point, submit_key| {
                assert_eq!(body, "hello");
                assert_eq!(bundle_id, "com.openai.codex");
                assert!(click_point.is_none());
                assert_eq!(submit_key, platform::macos::NativeSubmitKey::Enter);
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
            platform::macos::NativeSubmitKey::Enter,
            |_, _, _, _| AutosendOutcome::paste_event_failed("app paste failed".to_string()),
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
            platform::macos::NativeSubmitKey::Enter,
            |body, bundle_id, _, submit_key| {
                assert_eq!(submit_key, platform::macos::NativeSubmitKey::Enter);
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
    fn paste_only_sequence_joins_bodies_and_reports_processed_without_submit() {
        let state = PromptPickSessionState::default();
        state.set(prompt_target(
            "Codex",
            "com.openai.codex",
            Some(std::process::id()),
        ));
        let bodies = vec!["first".to_string(), "second".to_string()];
        let outcome = paste_prompt_sequence_and_submit_to_session_target_with_senders(
            &bodies,
            700,
            &state,
            None,
            platform::macos::NativeSubmitKey::None,
            |body, bundle_id, _, submit_key| {
                assert_eq!(body, "first\n\nsecond");
                assert_eq!(bundle_id, "com.openai.codex");
                assert_eq!(submit_key, platform::macos::NativeSubmitKey::None);
                AutosendOutcome::pasted_only()
            },
            |_| panic!("copy fallback must not run"),
            |_| panic!("joined paste-only group must not sleep between bodies"),
        )
        .unwrap();

        assert!(!outcome.sent);
        assert_eq!(outcome.sent_count, 0);
        assert_eq!(outcome.processed_count, 2);
        assert_eq!(
            outcome.completion,
            Some(platform::AutosendCompletion::PastedOnly)
        );
        assert_eq!(outcome.failed_index, None);
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
            platform::macos::NativeSubmitKey::Enter,
            |_, _, _, _| AutosendOutcome::sent(),
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
            platform::macos::NativeSubmitKey::Enter,
            |body, _, _, _| {
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
            platform::macos::NativeSubmitKey::Enter,
            |_, _, _, _| panic!("app sender must not run without target"),
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
            platform::macos::NativeSubmitKey::Enter,
            |body, bundle_id, click_point, submit_key| {
                assert_eq!(submit_key, platform::macos::NativeSubmitKey::Enter);
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
        let mut child = std::process::Command::new("sleep")
            .arg("2")
            .spawn()
            .expect("test helper process should start");
        let pid = child.id();
        record_last_app_if_valid(
            &state,
            frontmost_target("WeChat", "com.tencent.xinWeChat", Some(pid)),
        );

        assert_eq!(state.get().unwrap().app.bundle_id, "com.tencent.xinWeChat");
        assert_eq!(state.get().unwrap().pid, Some(pid));
        child.kill().expect("test helper process should stop");
        child.wait().expect("test helper process should be reaped");
    }

    #[test]
    fn skips_prompt_picker_as_frontmost_app_fallback() {
        let state = LastInputTargetState::default();
        record_last_app_if_valid(
            &state,
            frontmost_target("Prompt Drawer", "local.promptpicker.dev", Some(1)),
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

    #[test]
    fn prompt_button_monitor_builds_when_enabled_button_is_missing() {
        assert_eq!(
            prompt_button_ensure_action(true, false, false, false),
            PromptButtonEnsureAction::BuildMissing
        );
    }

    #[test]
    fn prompt_button_monitor_shows_enabled_hidden_button() {
        assert_eq!(
            prompt_button_ensure_action(true, true, false, true),
            PromptButtonEnsureAction::ShowExisting
        );
    }

    #[test]
    fn prompt_button_monitor_leaves_enabled_visible_button_alone() {
        assert_eq!(
            prompt_button_ensure_action(true, true, true, true),
            PromptButtonEnsureAction::None
        );
    }

    #[test]
    fn prompt_button_monitor_keeps_unready_window_hidden() {
        assert_eq!(
            prompt_button_ensure_action(true, true, false, false),
            PromptButtonEnsureAction::None
        );
    }

    #[test]
    fn prompt_button_monitor_never_revives_user_disabled_button() {
        assert_eq!(
            prompt_button_ensure_action(false, false, false, false),
            PromptButtonEnsureAction::None
        );
        assert_eq!(
            prompt_button_ensure_action(false, true, false, true),
            PromptButtonEnsureAction::None
        );
    }

    #[test]
    fn disabled_visibility_wins_over_an_in_flight_show() {
        let state = PromptButtonVisibilityState::new(true);
        let show_generation = state.generation();
        state.set(false);

        assert!(!state.may_show(show_generation));
    }

    #[test]
    fn saved_visibility_initializes_state_once() {
        let settings = serde_json::json!({ "floatingButton": { "visible": false } });
        let state = PromptButtonVisibilityState::from_settings(&settings);

        assert!(!state.desired_visible());
    }

    fn temporary_settings_path(name: &str) -> std::path::PathBuf {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock should be after epoch")
            .as_nanos();
        std::env::temp_dir()
            .join(format!("prompt-picker-{name}-{nonce}"))
            .join("settings.json")
    }

    #[test]
    fn malformed_frontend_settings_leave_existing_file_intact() {
        let path = temporary_settings_path("malformed");
        let state = SettingsFileState::new(path.clone());
        let visibility = PromptButtonVisibilityState::new(true);
        state
            .write_value_unlocked(&serde_json::json!({ "preserved": true }))
            .unwrap();

        assert!(state.write_frontend_text("{", &visibility).is_err());
        assert_eq!(state.read_value()["preserved"], true);
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn frontend_settings_write_preserves_unknown_fields_and_native_visibility() {
        let path = temporary_settings_path("unknown-fields");
        let state = SettingsFileState::new(path.clone());
        let visibility = PromptButtonVisibilityState::new(false);

        state
            .write_frontend_text(
                r#"{"version":1,"future":{"enabled":true},"floatingButton":{"visible":true}}"#,
                &visibility,
            )
            .unwrap();

        let saved = state.read_value();
        assert_eq!(
            saved.pointer("/future/enabled"),
            Some(&serde_json::json!(true))
        );
        assert_eq!(
            saved.pointer("/floatingButton/visible"),
            Some(&serde_json::json!(false))
        );
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn accessibility_patch_does_not_overwrite_visibility() {
        let path = temporary_settings_path("permission-patch");
        let state = SettingsFileState::new(path.clone());
        state
            .patch_bool(&["floatingButton", "visible"], false)
            .unwrap();

        set_accessibility_prompt_requested(&state, true).unwrap();

        let saved = state.read_value();
        assert_eq!(
            saved.pointer("/floatingButton/visible"),
            Some(&serde_json::json!(false))
        );
        assert_eq!(
            saved.pointer("/permissions/accessibilityPromptRequested"),
            Some(&serde_json::json!(true))
        );
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn settings_write_creates_missing_app_data_directory() {
        let path = temporary_settings_path("missing-directory");
        let state = SettingsFileState::new(path.clone());

        state
            .patch_bool(&["floatingButton", "visible"], true)
            .unwrap();

        assert!(path.is_file());
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }
}
