#![cfg(target_os = "macos")]

use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct FrontmostApp {
    pub name: String,
    pub bundle_id: String,
}

#[derive(Debug, Serialize)]
pub struct AccessibilityStatus {
    pub trusted: bool,
}

pub fn accessibility_status() -> AccessibilityStatus {
    // TODO: Implement real accessibility check
    AccessibilityStatus { trusted: false }
}

pub fn frontmost_app() -> Option<FrontmostApp> {
    // TODO: Implement real frontmost app detection
    None
}