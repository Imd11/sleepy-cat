pub mod macos;

pub use macos::{
    accessibility_status, frontmost_app, request_accessibility_permission, AccessibilityStatus,
    AutosendOutcome, CandidateInput, FrontmostApp, InputTarget,
};
