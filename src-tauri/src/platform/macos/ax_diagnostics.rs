use serde::Serialize;
use std::collections::VecDeque;
use std::process::Command;
use std::thread;
use std::time::{Duration, Instant};

use super::ax_client::{
    attribute_is_settable, bool_attribute, cf_string_value, children, copy_attribute, frame,
    owner_pid, set_ax_bool_attribute, string_attribute, AxTraversalBudget, AxTraversalLimits,
};
use super::{
    app_info_for_pid, frontmost_app_info, AXUIElementCreateApplication, CandidateInput, OwnedCf,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct DiagnosticTargetSelector {
    pub bundle_id: String,
    pub pid: Option<u32>,
    pub wait_ms: u64,
}

impl DiagnosticTargetSelector {
    pub(super) fn from_values(
        bundle_id: Option<&str>,
        pid: Option<&str>,
        wait_ms: Option<&str>,
    ) -> Result<Self, String> {
        let bundle_id = bundle_id
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| "PROMPT_DRAWER_AX_TARGET_BUNDLE_ID is required.".to_string())?;
        let pid = pid
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(|value| {
                value.parse::<u32>().map_err(|_| {
                    "PROMPT_DRAWER_AX_TARGET_PID must be a positive integer.".to_string()
                })
            })
            .transpose()?;
        let wait_ms = wait_ms
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(|value| {
                value.parse::<u64>().map_err(|_| {
                    "PROMPT_DRAWER_AX_WAIT_MS must be a non-negative integer.".to_string()
                })
            })
            .transpose()?
            .unwrap_or(3_000);

        Ok(Self {
            bundle_id: bundle_id.to_string(),
            pid,
            wait_ms,
        })
    }

    pub(super) fn from_env() -> Result<Self, String> {
        Self::from_values(
            std::env::var("PROMPT_DRAWER_AX_TARGET_BUNDLE_ID")
                .ok()
                .as_deref(),
            std::env::var("PROMPT_DRAWER_AX_TARGET_PID").ok().as_deref(),
            std::env::var("PROMPT_DRAWER_AX_WAIT_MS").ok().as_deref(),
        )
    }
}

#[derive(Clone, Debug, Serialize)]
pub(super) struct AxProcessDiagnostic {
    pub pid: u32,
    pub parent_pid: Option<u32>,
    pub command_hash: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub(super) struct AxCandidateDiagnostic {
    pub owner_pid: Option<u32>,
    pub role: Option<String>,
    pub subrole: Option<String>,
    pub frame: Option<CandidateInput>,
    pub enabled: Option<bool>,
    pub focused: Option<bool>,
    pub focused_settable: bool,
    pub value_settable: bool,
    pub text_length: Option<usize>,
    pub semantic_hash: Option<String>,
    pub depth: usize,
}

#[derive(Clone, Debug, Serialize)]
pub(super) struct AxDiagnosticReport {
    pub bundle_id: String,
    pub app_name: String,
    pub app_version: Option<String>,
    pub main_pid: u32,
    pub descendants: Vec<AxProcessDiagnostic>,
    pub window_frame: Option<CandidateInput>,
    pub candidates: Vec<AxCandidateDiagnostic>,
    pub visited_nodes: usize,
    pub deepest_level: usize,
    pub stopped_by_budget: bool,
    pub elapsed_ms: u128,
}

pub(super) fn diagnostics_enabled(value: Option<&str>) -> bool {
    value == Some("1")
}

pub(super) fn manual_accessibility_comparison_enabled(
    diagnostics: Option<&str>,
    manual_accessibility: Option<&str>,
) -> bool {
    diagnostics_enabled(diagnostics) && manual_accessibility == Some("1")
}

fn stable_hash(value: &str) -> String {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for byte in value.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{hash:016x}")
}

fn semantic_hash(element: super::AXUIElementRef, timeout: f32) -> Option<String> {
    [
        "AXIdentifier",
        "AXPlaceholderValue",
        "AXTitle",
        "AXDescription",
    ]
    .into_iter()
    .filter_map(|attribute| string_attribute(element, attribute, timeout))
    .find(|value| !value.trim().is_empty())
    .map(|value| stable_hash(value.trim()))
}

fn text_length(element: super::AXUIElementRef, timeout: f32) -> Option<usize> {
    let value = copy_attribute(element, "AXValue", timeout)?;
    cf_string_value(value.as_ptr()).map(|value| value.chars().count())
}

fn app_version(bundle_id: &str) -> Option<String> {
    let query = format!("kMDItemCFBundleIdentifier == '{bundle_id}'");
    let output = Command::new("mdfind").arg(query).output().ok()?;
    let app_path = String::from_utf8_lossy(&output.stdout)
        .lines()
        .find(|path| path.ends_with(".app"))?
        .to_string();
    let plist_path = format!("{app_path}/Contents/Info.plist");
    let output = Command::new("plutil")
        .args(["-extract", "CFBundleShortVersionString", "raw", &plist_path])
        .output()
        .ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn descendant_processes(main_pid: u32) -> Vec<AxProcessDiagnostic> {
    let Ok(output) = Command::new("ps")
        .args(["-axo", "pid=,ppid=,command="])
        .output()
    else {
        return Vec::new();
    };
    if !output.status.success() {
        return Vec::new();
    }

    let mut rows = Vec::new();
    for line in String::from_utf8_lossy(&output.stdout).lines() {
        let mut parts = line.split_whitespace();
        let Some(pid) = parts.next().and_then(|value| value.parse::<u32>().ok()) else {
            continue;
        };
        let Some(parent_pid) = parts.next().and_then(|value| value.parse::<u32>().ok()) else {
            continue;
        };
        let command = parts.collect::<Vec<_>>().join(" ");
        rows.push((pid, parent_pid, command));
    }

    let mut accepted = vec![main_pid];
    let mut diagnostics = Vec::new();
    loop {
        let mut changed = false;
        for (pid, parent_pid, command) in &rows {
            if accepted.contains(pid) || !accepted.contains(parent_pid) {
                continue;
            }
            accepted.push(*pid);
            diagnostics.push(AxProcessDiagnostic {
                pid: *pid,
                parent_pid: Some(*parent_pid),
                command_hash: (!command.is_empty()).then(|| stable_hash(command)),
            });
            changed = true;
        }
        if !changed {
            break;
        }
    }
    diagnostics
}

pub(super) fn collect_target_input_diagnostics(
    selector: DiagnosticTargetSelector,
    limits: AxTraversalLimits,
) -> Result<AxDiagnosticReport, String> {
    if !diagnostics_enabled(
        std::env::var("PROMPT_DRAWER_AX_DIAGNOSTICS")
            .ok()
            .as_deref(),
    ) {
        return Err("AX diagnostics are disabled.".to_string());
    }

    thread::sleep(Duration::from_millis(selector.wait_ms));
    let app = match selector.pid {
        Some(pid) => app_info_for_pid(pid)
            .ok_or_else(|| format!("No running application was found for PID {pid}."))?,
        None => frontmost_app_info().ok_or_else(|| "No frontmost app is available.".to_string())?,
    };
    if app.app.bundle_id != selector.bundle_id {
        return Err(format!(
            "Resolved app does not match diagnostic target: expected {}, got {} {}.",
            selector.bundle_id, app.app.bundle_id, app.pid
        ));
    }

    let started = Instant::now();
    let root = OwnedCf::created(unsafe { AXUIElementCreateApplication(app.pid as i32) })
        .ok_or_else(|| "Could not create target AX root.".to_string())?;
    let window = copy_attribute(root.as_ptr(), "AXFocusedWindow", limits.per_element_timeout)
        .or_else(|| copy_attribute(root.as_ptr(), "AXMainWindow", limits.per_element_timeout));
    let window_frame = window
        .as_ref()
        .and_then(|window| frame(window.as_ptr(), limits.per_element_timeout));

    let mut candidates = Vec::new();
    let mut budget = AxTraversalBudget::new(limits);
    let mut queue = VecDeque::new();
    if let Some(window) = window.as_ref() {
        queue.extend(
            children(window.as_ptr(), limits.per_element_timeout)
                .into_iter()
                .map(|child| (child, 1_usize)),
        );
    }

    while let Some((element, depth)) = queue.pop_front() {
        if !budget.try_visit(depth) {
            break;
        }
        let role = string_attribute(element.as_ptr(), "AXRole", limits.per_element_timeout);
        if !budget.has_time_remaining() {
            break;
        }
        if matches!(
            role.as_deref(),
            Some("AXTextArea" | "AXTextField" | "AXComboBox" | "AXWebArea")
        ) {
            candidates.push(AxCandidateDiagnostic {
                owner_pid: owner_pid(element.as_ptr(), limits.per_element_timeout),
                role,
                subrole: string_attribute(
                    element.as_ptr(),
                    "AXSubrole",
                    limits.per_element_timeout,
                ),
                frame: frame(element.as_ptr(), limits.per_element_timeout),
                enabled: bool_attribute(element.as_ptr(), "AXEnabled", limits.per_element_timeout),
                focused: bool_attribute(element.as_ptr(), "AXFocused", limits.per_element_timeout),
                focused_settable: attribute_is_settable(
                    element.as_ptr(),
                    "AXFocused",
                    limits.per_element_timeout,
                ),
                value_settable: attribute_is_settable(
                    element.as_ptr(),
                    "AXValue",
                    limits.per_element_timeout,
                ),
                text_length: text_length(element.as_ptr(), limits.per_element_timeout),
                semantic_hash: semantic_hash(element.as_ptr(), limits.per_element_timeout),
                depth,
            });
        }
        if !budget.has_time_remaining() {
            break;
        }
        if depth < limits.max_depth {
            queue.extend(
                children(element.as_ptr(), limits.per_element_timeout)
                    .into_iter()
                    .map(|child| (child, depth + 1)),
            );
        }
    }
    let stats = budget.stats();

    Ok(AxDiagnosticReport {
        bundle_id: app.app.bundle_id.clone(),
        app_name: app.app.name,
        app_version: app_version(&app.app.bundle_id),
        main_pid: app.pid,
        descendants: descendant_processes(app.pid),
        window_frame,
        candidates,
        visited_nodes: stats.visited_nodes,
        deepest_level: stats.deepest_level,
        stopped_by_budget: stats.stopped_by_budget,
        elapsed_ms: started.elapsed().as_millis(),
    })
}

#[derive(Serialize)]
struct AxManualAccessibilityComparison {
    before: AxDiagnosticReport,
    after: AxDiagnosticReport,
}

fn collect_manual_accessibility_comparison(
    selector: DiagnosticTargetSelector,
    limits: AxTraversalLimits,
) -> Result<AxManualAccessibilityComparison, String> {
    if !manual_accessibility_comparison_enabled(
        std::env::var("PROMPT_DRAWER_AX_DIAGNOSTICS")
            .ok()
            .as_deref(),
        std::env::var("PROMPT_DRAWER_AX_ALLOW_MANUAL_ACCESSIBILITY")
            .ok()
            .as_deref(),
    ) {
        return Err("AX manual accessibility comparison is disabled.".to_string());
    }

    let before = collect_target_input_diagnostics(selector.clone(), limits)?;
    let root = OwnedCf::created(unsafe { AXUIElementCreateApplication(before.main_pid as i32) })
        .ok_or_else(|| "Could not create target AX root.".to_string())?;
    if !set_ax_bool_attribute(root.as_ptr(), "AXManualAccessibility", true) {
        return Err("Target app rejected AXManualAccessibility.".to_string());
    }
    thread::sleep(Duration::from_millis(120));

    let mut after_selector = selector;
    after_selector.wait_ms = 0;
    let after = collect_target_input_diagnostics(after_selector, limits)?;
    Ok(AxManualAccessibilityComparison { before, after })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diagnostics_are_disabled_unless_explicitly_enabled() {
        assert!(!diagnostics_enabled(None));
        assert!(!diagnostics_enabled(Some("0")));
        assert!(diagnostics_enabled(Some("1")));
    }

    #[test]
    fn manual_accessibility_comparison_requires_separate_opt_in() {
        assert!(!manual_accessibility_comparison_enabled(Some("1"), None));
        assert!(!manual_accessibility_comparison_enabled(None, Some("1")));
        assert!(manual_accessibility_comparison_enabled(
            Some("1"),
            Some("1")
        ));
    }

    #[test]
    fn diagnostic_target_requires_bundle_id_and_valid_numbers() {
        assert!(DiagnosticTargetSelector::from_values(None, None, None).is_err());
        assert!(DiagnosticTargetSelector::from_values(Some("com.test"), Some("no"), None).is_err());
        assert!(DiagnosticTargetSelector::from_values(Some("com.test"), None, Some("no")).is_err());

        assert_eq!(
            DiagnosticTargetSelector::from_values(Some("com.test"), Some("42"), Some("1500"))
                .unwrap(),
            DiagnosticTargetSelector {
                bundle_id: "com.test".to_string(),
                pid: Some(42),
                wait_ms: 1500,
            }
        );
    }

    #[test]
    fn diagnostic_json_contains_no_raw_text_or_clipboard_fields() {
        let report = AxDiagnosticReport {
            bundle_id: "com.test".to_string(),
            app_name: "Test".to_string(),
            app_version: Some("1.0".to_string()),
            main_pid: 42,
            descendants: Vec::new(),
            window_frame: None,
            candidates: vec![AxCandidateDiagnostic {
                owner_pid: Some(42),
                role: Some("AXTextArea".to_string()),
                subrole: None,
                frame: None,
                enabled: Some(true),
                focused: Some(false),
                focused_settable: true,
                value_settable: true,
                text_length: Some(12),
                semantic_hash: Some("deadbeef".to_string()),
                depth: 2,
            }],
            visited_nodes: 1,
            deepest_level: 2,
            stopped_by_budget: false,
            elapsed_ms: 3,
        };

        let json = serde_json::to_string(&report).unwrap();
        assert!(!json.contains("raw_value"));
        assert!(!json.contains("clipboard"));
        assert!(!json.contains("conversation"));
        assert!(json.contains("text_length"));
        assert!(json.contains("semantic_hash"));
    }

    #[test]
    #[ignore = "requires a frontmost real app and accessibility permission"]
    fn print_target_ax_input_diagnostics() {
        assert!(diagnostics_enabled(
            std::env::var("PROMPT_DRAWER_AX_DIAGNOSTICS")
                .ok()
                .as_deref()
        ));
        let selector = DiagnosticTargetSelector::from_env().unwrap();
        let report = collect_target_input_diagnostics(selector, AxTraversalLimits::diagnostic());
        println!(
            "{}",
            serde_json::to_string_pretty(&report.unwrap()).unwrap()
        );
    }

    #[test]
    #[ignore = "mutates target Electron accessibility state for the process lifetime"]
    fn print_target_ax_manual_accessibility_comparison() {
        assert!(manual_accessibility_comparison_enabled(
            std::env::var("PROMPT_DRAWER_AX_DIAGNOSTICS")
                .ok()
                .as_deref(),
            std::env::var("PROMPT_DRAWER_AX_ALLOW_MANUAL_ACCESSIBILITY")
                .ok()
                .as_deref(),
        ));
        let selector = DiagnosticTargetSelector::from_env().unwrap();
        let report =
            collect_manual_accessibility_comparison(selector, AxTraversalLimits::diagnostic());
        println!(
            "{}",
            serde_json::to_string_pretty(&report.unwrap()).unwrap()
        );
    }
}
