use serde::Serialize;

#[derive(Clone, Debug, Serialize)]
pub struct FrontmostApp {
    pub name: String,
    pub bundle_id: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct FrontmostAppWithPid {
    pub app: FrontmostApp,
    pub pid: Option<u32>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct ProcessLaunchIdentity {
    pub seconds: u64,
    pub microseconds: u64,
}

#[derive(Debug, Serialize)]
pub struct AccessibilityStatus {
    pub trusted: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AutosendFailureReason {
    CopyFailed,
    MissingAccessibilityPermission,
    NoSafeTarget,
    PasteEventFailed,
    ReturnEventFailed,
    TargetFocusFailed,
    TargetChanged,
    ComposerNotFound,
    ComposerAmbiguous,
    FocusNotAcquired,
    PasteNotConfirmed,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AutosendCompletion {
    PastedOnly,
    Submitted,
}

#[derive(Clone, Debug, Serialize)]
pub struct AutosendOutcome {
    pub copied: bool,
    pub sent: bool,
    pub completion: Option<AutosendCompletion>,
    pub error: Option<String>,
    pub reason: Option<AutosendFailureReason>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NativeSubmitKey {
    None,
    Enter,
    CommandEnter,
}

impl AutosendOutcome {
    pub fn sent() -> Self {
        Self {
            copied: true,
            sent: true,
            completion: Some(AutosendCompletion::Submitted),
            error: None,
            reason: None,
        }
    }

    pub fn pasted_only() -> Self {
        Self {
            copied: true,
            sent: false,
            completion: Some(AutosendCompletion::PastedOnly),
            error: None,
            reason: None,
        }
    }

    pub fn copy_failed(error: String) -> Self {
        Self {
            copied: false,
            sent: false,
            completion: None,
            error: Some(error),
            reason: Some(AutosendFailureReason::CopyFailed),
        }
    }

    pub fn keyboard_failed(error: String) -> Self {
        Self::paste_event_failed(error)
    }

    pub fn missing_accessibility_permission() -> Self {
        Self {
            copied: false,
            sent: false,
            completion: None,
            error: Some("Accessibility permission is only available on macOS.".to_string()),
            reason: Some(AutosendFailureReason::MissingAccessibilityPermission),
        }
    }

    pub fn copied_without_send(error: String) -> Self {
        Self {
            copied: true,
            sent: false,
            completion: None,
            error: Some(error),
            reason: Some(AutosendFailureReason::NoSafeTarget),
        }
    }

    pub fn paste_event_failed(error: String) -> Self {
        Self {
            copied: true,
            sent: false,
            completion: None,
            error: Some(error),
            reason: Some(AutosendFailureReason::PasteEventFailed),
        }
    }

    pub fn return_event_failed(error: String) -> Self {
        Self {
            copied: true,
            sent: false,
            completion: None,
            error: Some(error),
            reason: Some(AutosendFailureReason::ReturnEventFailed),
        }
    }

    pub fn target_focus_failed(error: String) -> Self {
        Self {
            copied: false,
            sent: false,
            completion: None,
            error: Some(error),
            reason: Some(AutosendFailureReason::TargetFocusFailed),
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct InputTarget {
    pub frame: CandidateInput,
    pub window_frame: CandidateInput,
    pub button_position: (f64, f64),
    pub click_point: (f64, f64),
    pub app: Option<FrontmostApp>,
    pub pid: u32,
}

#[derive(Clone, Debug, PartialEq, Serialize, serde::Deserialize)]
pub struct CandidateInput {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

pub fn process_launch_identity(_pid: u32) -> Option<ProcessLaunchIdentity> {
    None
}

pub fn accessibility_status() -> AccessibilityStatus {
    AccessibilityStatus { trusted: false }
}

pub fn request_accessibility_permission() -> AccessibilityStatus {
    AccessibilityStatus { trusted: false }
}

pub fn open_accessibility_settings() -> Result<(), String> {
    Err("Accessibility settings are only available on macOS.".to_string())
}

pub fn frontmost_app() -> Option<FrontmostApp> {
    None
}

pub fn frontmost_app_with_pid() -> Option<FrontmostAppWithPid> {
    None
}

pub fn current_input_target() -> Option<InputTarget> {
    None
}

pub fn current_pointer_location() -> Option<(f64, f64)> {
    None
}

pub fn paste_prompt_with_copier<C>(body: &str, copy_sender: C) -> Result<(), String>
where
    C: FnOnce(&str) -> Result<(), String>,
{
    copy_sender(body)
}

pub fn post_focus_preserving_paste() -> Result<(), String> {
    Err("Focus-preserving paste is only implemented for macOS targets.".to_string())
}

pub fn post_focus_preserving_submit_key(_submit_key: NativeSubmitKey) -> Result<(), String> {
    Err("Focus-preserving submit is only implemented for macOS targets.".to_string())
}

pub fn repair_focus_to_editable_element(_pid: u32) -> Result<(), String> {
    Err("AX focus repair is only implemented for macOS targets.".to_string())
}

pub fn recover_target_app_for_autosend(
    _bundle_id: &str,
    _click_point: Option<(f64, f64)>,
) -> Result<(), String> {
    Err("Target recovery is only implemented for macOS targets.".to_string())
}

pub fn paste_prompt_and_submit_to_app_clipboard_with_copier<C, A>(
    body: &str,
    _bundle_id: &str,
    _target_pid: u32,
    _target_launch_identity: ProcessLaunchIdentity,
    _click_point: Option<(f64, f64)>,
    _captured_window: Option<&CandidateInput>,
    _submit_key: NativeSubmitKey,
    _activate_target: A,
    copy_sender: C,
) -> AutosendOutcome
where
    C: FnOnce(&str) -> Result<(), String>,
    A: FnMut(u32) -> Result<(), String>,
{
    match copy_sender(body) {
        Ok(()) => AutosendOutcome::copied_without_send(
            "Autosend is only implemented for macOS targets.".to_string(),
        ),
        Err(error) => AutosendOutcome::copy_failed(error),
    }
}
