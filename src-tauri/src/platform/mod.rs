#[cfg(target_os = "macos")]
pub mod macos;

#[cfg(not(target_os = "macos"))]
pub mod unsupported;
#[cfg(not(target_os = "macos"))]
pub use unsupported as macos;

pub use macos::{
    accessibility_status, frontmost_app, frontmost_app_with_pid, request_accessibility_permission,
    AccessibilityStatus, AutosendCompletion, AutosendOutcome, CandidateInput, FrontmostApp,
    FrontmostAppWithPid, InputTarget, ProcessLaunchIdentity,
};
