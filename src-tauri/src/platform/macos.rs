#![cfg(target_os = "macos")]

mod autosend_transaction;
mod ax_client;
mod ax_diagnostics;
mod composer_resolver;
mod focus_controller;
mod input_profiles;
mod process_group;

use autosend_transaction::{run_transaction, TransactionFailure};
use ax_client::{
    ax_attribute_is_settable, ax_bool_attribute, ax_element_frame, ax_element_pid,
    ax_range_attribute, ax_string_attribute, copy_ax_attribute, elements_equal,
    set_ax_bool_attribute, traversal_children, OwnedCfValue as OwnedCf,
};
use composer_resolver::{resolve_composer, ComposerCandidate};
use focus_controller::ComposerFingerprint;
use input_profiles::{input_capability_profile, InputCapabilityProfile};
use process_group::discover_trusted_candidate_pids;
use serde::Serialize;
use std::collections::{hash_map::DefaultHasher, VecDeque};
use std::ffi::c_void;
use std::hash::{Hash, Hasher};
use std::process::Command;
use std::time::{Duration, Instant};

use core_graphics::event::CGEvent;
use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};

const ACCESSIBILITY_PERMISSION_REQUIRED_ERROR: &str =
    "Accessibility permission required for prompt insertion.";

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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum InputFocusPolicy {
    PreserveApplicationFirstResponder,
    ResolveEditableElement,
}

fn input_focus_policy(bundle_id: &str) -> InputFocusPolicy {
    match input_capability_profile(bundle_id, None) {
        InputCapabilityProfile::CodexFirstResponder
        | InputCapabilityProfile::LegacyCapturedTarget => {
            InputFocusPolicy::PreserveApplicationFirstResponder
        }
        InputCapabilityProfile::Accessibility(_) => InputFocusPolicy::ResolveEditableElement,
    }
}

impl AutosendOutcome {
    fn failed(copied: bool, reason: AutosendFailureReason, error: String) -> Self {
        Self {
            copied,
            sent: false,
            completion: None,
            error: Some(error),
            reason: Some(reason),
        }
    }

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
            error: Some(ACCESSIBILITY_PERMISSION_REQUIRED_ERROR.to_string()),
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

// ── Accessibility ──────────────────────────────────────────────────────────────

pub fn accessibility_status() -> AccessibilityStatus {
    AccessibilityStatus {
        trusted: is_accessibility_trusted(),
    }
}

pub fn request_accessibility_permission() -> AccessibilityStatus {
    AccessibilityStatus {
        trusted: is_accessibility_trusted_with_prompt(),
    }
}

pub fn accessibility_settings_url() -> &'static str {
    "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility"
}

pub fn open_accessibility_settings() -> Result<(), String> {
    let output = Command::new("open")
        .arg(accessibility_settings_url())
        .output()
        .map_err(|e| e.to_string())?;
    if output.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).to_string())
    }
}

// SAFETY: AXIsProcessTrusted is a macOS public API, safe to call from the main thread
unsafe fn ax_is_process_trusted() -> bool {
    #[link(name = "ApplicationServices", kind = "framework")]
    extern "C" {
        fn AXIsProcessTrusted() -> bool;
    }
    AXIsProcessTrusted()
}

// SAFETY: AXIsProcessTrustedWithOptions and CoreFoundation dictionary creation are
// public macOS APIs. Static key/value pointers are provided by the frameworks.
unsafe fn ax_is_process_trusted_with_prompt() -> bool {
    #[link(name = "ApplicationServices", kind = "framework")]
    extern "C" {
        static kAXTrustedCheckOptionPrompt: *const c_void;
        fn AXIsProcessTrustedWithOptions(options: *const c_void) -> bool;
    }
    #[link(name = "CoreFoundation", kind = "framework")]
    extern "C" {
        static kCFBooleanTrue: *const c_void;
        fn CFDictionaryCreate(
            allocator: *const c_void,
            keys: *const *const c_void,
            values: *const *const c_void,
            num_values: isize,
            key_callbacks: *const c_void,
            value_callbacks: *const c_void,
        ) -> *const c_void;
    }

    let keys = [kAXTrustedCheckOptionPrompt];
    let values = [kCFBooleanTrue];
    let options = CFDictionaryCreate(
        std::ptr::null(),
        keys.as_ptr(),
        values.as_ptr(),
        1,
        std::ptr::null(),
        std::ptr::null(),
    );
    if options.is_null() {
        return ax_is_process_trusted();
    }
    let trusted = AXIsProcessTrustedWithOptions(options);
    CFRelease(options);
    trusted
}

fn is_accessibility_trusted() -> bool {
    unsafe { ax_is_process_trusted() }
}

fn is_accessibility_trusted_with_prompt() -> bool {
    unsafe { ax_is_process_trusted_with_prompt() }
}

fn accessibility_permission_required_error() -> String {
    ACCESSIBILITY_PERMISSION_REQUIRED_ERROR.to_string()
}

fn ensure_accessibility_trusted_with<T>(is_trusted: T) -> Result<(), String>
where
    T: FnOnce() -> bool,
{
    if is_trusted() {
        Ok(())
    } else {
        Err(accessibility_permission_required_error())
    }
}

fn missing_accessibility_outcome_if_untrusted_with<T>(is_trusted: T) -> Option<AutosendOutcome>
where
    T: FnOnce() -> bool,
{
    if is_trusted() {
        None
    } else {
        Some(AutosendOutcome::missing_accessibility_permission())
    }
}

// ── Frontmost App ─────────────────────────────────────────────────────────────

pub fn frontmost_app() -> Option<FrontmostApp> {
    frontmost_app_info().map(|info| info.app)
}

pub fn frontmost_app_with_pid() -> Option<FrontmostAppWithPid> {
    frontmost_app_info().map(|info| FrontmostAppWithPid {
        app: info.app,
        pid: Some(info.pid),
    })
}

struct FrontmostAppInfo {
    app: FrontmostApp,
    pid: u32,
}

fn frontmost_app_info() -> Option<FrontmostAppInfo> {
    let front = Command::new("lsappinfo").arg("front").output().ok()?;
    let asn = parse_front_asn(String::from_utf8_lossy(&front.stdout).as_ref())?;
    app_info_for_asn(&asn)
}

fn app_info_for_asn(asn: &str) -> Option<FrontmostAppInfo> {
    let info = Command::new("lsappinfo")
        .args(["info", asn])
        .output()
        .ok()?;

    app_info_from_lsappinfo_output(String::from_utf8_lossy(&info.stdout).as_ref())
}

fn app_info_for_pid(pid: u32) -> Option<FrontmostAppInfo> {
    let info = Command::new("lsappinfo")
        .args(["info", "-pid", &pid.to_string()])
        .output()
        .ok()?;
    info.status
        .success()
        .then(|| app_info_from_lsappinfo_output(String::from_utf8_lossy(&info.stdout).as_ref()))
        .flatten()
}

fn app_info_from_lsappinfo_output(info: &str) -> Option<FrontmostAppInfo> {
    let info_trimmed = info.trim();

    let name = parse_app_name(info_trimmed).unwrap_or_else(|| "Unknown".to_string());
    let pid = parse_pid(info_trimmed)?;
    let bundle_id = parse_bundle_id(info_trimmed).unwrap_or_else(|| format!("unknown.pid.{pid}"));

    Some(FrontmostAppInfo {
        app: FrontmostApp { name, bundle_id },
        pid,
    })
}

// ── Current Input Target ──────────────────────────────────────────────────────

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

#[repr(C)]
#[derive(Clone, Copy)]
struct ProcBsdInfo {
    flags: u32,
    status: u32,
    xstatus: u32,
    pid: u32,
    ppid: u32,
    uid: u32,
    gid: u32,
    ruid: u32,
    rgid: u32,
    svuid: u32,
    svgid: u32,
    rfu_1: u32,
    comm: [u8; 16],
    name: [u8; 32],
    nfiles: u32,
    pgid: u32,
    pjobc: u32,
    e_tdev: u32,
    e_tpgid: u32,
    nice: i32,
    start_tvsec: u64,
    start_tvusec: u64,
}

const PROC_PIDTBSDINFO: i32 = 3;

unsafe extern "C" {
    fn proc_pidinfo(pid: i32, flavor: i32, arg: u64, buffer: *mut c_void, buffersize: i32) -> i32;
}

pub fn process_launch_identity(pid: u32) -> Option<ProcessLaunchIdentity> {
    let mut info = std::mem::MaybeUninit::<ProcBsdInfo>::zeroed();
    let expected = std::mem::size_of::<ProcBsdInfo>() as i32;
    let written = unsafe {
        proc_pidinfo(
            pid.try_into().ok()?,
            PROC_PIDTBSDINFO,
            0,
            info.as_mut_ptr().cast(),
            expected,
        )
    };
    if written != expected {
        return None;
    }
    let info = unsafe { info.assume_init() };
    Some(ProcessLaunchIdentity {
        seconds: info.start_tvsec,
        microseconds: info.start_tvusec,
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum EditableRole {
    TextArea,
    TextField,
    SearchField,
    ComboBox,
    WebArea,
}

#[derive(Clone, Debug)]
struct EditableCandidate {
    role: EditableRole,
    frame: CandidateInput,
    enabled: bool,
    focused: bool,
    depth: usize,
}

fn editable_candidate_score(candidate: &EditableCandidate, window: &CandidateInput) -> i32 {
    if !candidate.enabled || candidate.frame.width <= 1.0 || candidate.frame.height <= 1.0 {
        return i32::MIN;
    }

    let mut score = match candidate.role {
        EditableRole::TextArea => 100,
        EditableRole::TextField => 60,
        EditableRole::ComboBox => 45,
        EditableRole::WebArea => 20,
        EditableRole::SearchField => -60,
    };
    if candidate.focused {
        score += 200;
    }
    if candidate.frame.y + (candidate.frame.height / 2.0) >= window.y + (window.height * 0.45) {
        score += 35;
    }
    if candidate.frame.width >= window.width * 0.35 {
        score += 25;
    }
    if candidate.frame.height >= 40.0 {
        score += 20;
    }
    score - i32::try_from(candidate.depth.min(20)).unwrap_or(20)
}

fn select_editable_candidate(
    candidates: &[EditableCandidate],
    window: &CandidateInput,
) -> Option<usize> {
    const MIN_SCORE: i32 = 80;
    const MIN_SCORE_MARGIN: i32 = 15;

    let mut ranked: Vec<(usize, i32)> = candidates
        .iter()
        .enumerate()
        .map(|(index, candidate)| (index, editable_candidate_score(candidate, window)))
        .filter(|(_, score)| *score >= MIN_SCORE)
        .collect();
    ranked.sort_unstable_by(|left, right| right.1.cmp(&left.1));

    let (best_index, best_score) = *ranked.first()?;
    if ranked
        .get(1)
        .is_some_and(|(_, next_score)| best_score - *next_score < MIN_SCORE_MARGIN)
    {
        return None;
    }
    Some(best_index)
}

struct NativeEditableCandidate {
    resolver: ComposerCandidate,
    element: OwnedCf,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct PasteEvidenceSnapshot {
    value_hash: Option<u64>,
    selected_range: Option<(isize, isize)>,
}

enum NativeFocusAttempt {
    Focused(ComposerFingerprint),
    SparseTree,
    Rejected,
}

fn identifier_hash(value: Option<&str>) -> Option<String> {
    value.map(|value| {
        let mut hasher = DefaultHasher::new();
        value.hash(&mut hasher);
        format!("{:016x}", hasher.finish())
    })
}

fn candidate_fingerprint(candidate: &ComposerCandidate) -> ComposerFingerprint {
    ComposerFingerprint {
        owner_pid: candidate.owner_pid,
        role: candidate.role.clone(),
        subrole: candidate.subrole.clone(),
        identifier_hash: identifier_hash(candidate.identifier.as_deref()),
        frame: candidate.frame.clone(),
    }
}

fn fingerprints_match(expected: &ComposerFingerprint, current: &ComposerFingerprint) -> bool {
    const FRAME_TOLERANCE: f64 = 2.0;
    expected.owner_pid == current.owner_pid
        && expected.role == current.role
        && expected.subrole == current.subrole
        && expected.identifier_hash == current.identifier_hash
        && (expected.frame.x - current.frame.x).abs() <= FRAME_TOLERANCE
        && (expected.frame.y - current.frame.y).abs() <= FRAME_TOLERANCE
        && (expected.frame.width - current.frame.width).abs() <= FRAME_TOLERANCE
        && (expected.frame.height - current.frame.height).abs() <= FRAME_TOLERANCE
}

fn editable_role(role: &str) -> Option<EditableRole> {
    match role {
        "AXTextArea" => Some(EditableRole::TextArea),
        "AXTextField" => Some(EditableRole::TextField),
        "AXSearchField" => Some(EditableRole::SearchField),
        "AXComboBox" => Some(EditableRole::ComboBox),
        "AXWebArea" => Some(EditableRole::WebArea),
        _ => None,
    }
}

fn role_can_receive_prompt(role: EditableRole, _ax_editable: bool, _value_settable: bool) -> bool {
    match role {
        EditableRole::SearchField => false,
        EditableRole::WebArea => false,
        EditableRole::TextArea | EditableRole::TextField | EditableRole::ComboBox => true,
    }
}

fn native_editable_candidate(element: OwnedCf, depth: usize) -> Option<NativeEditableCandidate> {
    let role_name = ax_string_attribute(element.as_ptr(), "AXRole")?;
    let role = editable_role(&role_name)?;
    if !role_can_receive_prompt(
        role,
        ax_bool_attribute(element.as_ptr(), "AXEditable").unwrap_or(false),
        ax_attribute_is_settable(element.as_ptr(), "AXValue"),
    ) {
        return None;
    }
    let frame = ax_element_frame(element.as_ptr())?;
    let enabled = ax_bool_attribute(element.as_ptr(), "AXEnabled").unwrap_or(true);
    let focused = ax_bool_attribute(element.as_ptr(), "AXFocused").unwrap_or(false);
    let subrole = ax_string_attribute(element.as_ptr(), "AXSubrole");
    if matches!(subrole.as_deref(), Some("AXSecureTextField" | "AXSearchField")) {
        return None;
    }
    Some(NativeEditableCandidate {
        resolver: ComposerCandidate {
            owner_pid: ax_element_pid(element.as_ptr())?,
            role: role_name,
            secure: subrole.as_deref() == Some("AXSecureTextField"),
            subrole,
            identifier: ax_string_attribute(element.as_ptr(), "AXIdentifier"),
            title: ax_string_attribute(element.as_ptr(), "AXTitle"),
            description: ax_string_attribute(element.as_ptr(), "AXDescription"),
            placeholder: ax_string_attribute(element.as_ptr(), "AXPlaceholderValue"),
            help: ax_string_attribute(element.as_ptr(), "AXHelp"),
            frame,
            enabled,
            visible: !ax_bool_attribute(element.as_ptr(), "AXHidden").unwrap_or(false),
            focused,
            window_matches: true,
            editable: true,
            depth,
        },
        element,
    })
}

fn focused_editable_candidate(app: AXUIElementRef) -> Option<NativeEditableCandidate> {
    let focused = copy_ax_attribute(app, "AXFocusedUIElement")?;
    native_editable_candidate(focused, 0)
}

fn focused_editable_pid(app: AXUIElementRef) -> Option<u32> {
    focused_editable_candidate(app).map(|candidate| candidate.resolver.owner_pid)
}

fn focused_window(app: AXUIElementRef) -> Option<OwnedCf> {
    copy_ax_attribute(app, "AXFocusedWindow").or_else(|| copy_ax_attribute(app, "AXMainWindow"))
}

fn collect_editable_candidates(window: AXUIElementRef) -> (Vec<NativeEditableCandidate>, usize) {
    const MAX_ELEMENTS: usize = 600;
    const MAX_DEPTH: usize = 14;
    const MAX_SCAN_TIME: Duration = Duration::from_millis(220);

    let started = Instant::now();
    let mut queue: VecDeque<(OwnedCf, usize)> = traversal_children(window)
        .into_iter()
        .map(|child| (child, 1))
        .collect();
    let mut candidates = Vec::new();
    let mut visited = 0;

    while let Some((element, depth)) = queue.pop_front() {
        if visited >= MAX_ELEMENTS || started.elapsed() >= MAX_SCAN_TIME {
            break;
        }
        visited += 1;

        if depth < MAX_DEPTH {
            queue.extend(
                traversal_children(element.as_ptr())
                    .into_iter()
                    .map(|child| (child, depth + 1)),
            );
        }
        if let Some(candidate) = native_editable_candidate(element, depth) {
            candidates.push(candidate);
        }
    }
    (candidates, visited)
}

fn focus_editable_input_once(
    app: AXUIElementRef,
    trusted_pids: &[u32],
    captured_window: &CandidateInput,
) -> Result<NativeFocusAttempt, String> {
    let window = focused_window(app)
        .ok_or_else(|| "The target app does not expose a focused window.".to_string())?;
    let window_frame = ax_element_frame(window.as_ptr())
        .ok_or_else(|| "The target window does not expose its frame.".to_string())?;
    if !candidate_frames_overlap(&window_frame, captured_window) {
        return Ok(NativeFocusAttempt::Rejected);
    }
    if let Some(candidate) = focused_editable_candidate(app) {
        if resolve_composer(
            std::slice::from_ref(&candidate.resolver),
            trusted_pids,
            captured_window,
        ) == Ok(0)
        {
            return Ok(NativeFocusAttempt::Focused(candidate_fingerprint(
                &candidate.resolver,
            )));
        }
    }

    let (candidates, visited) = collect_editable_candidates(window.as_ptr());
    if candidates.is_empty() && visited <= 2 {
        return Ok(NativeFocusAttempt::SparseTree);
    }
    let resolver_candidates: Vec<ComposerCandidate> = candidates
        .iter()
        .map(|candidate| candidate.resolver.clone())
        .collect();
    let Ok(index) = resolve_composer(&resolver_candidates, trusted_pids, captured_window) else {
        return Ok(NativeFocusAttempt::Rejected);
    };
    let candidate = &candidates[index];

    if ax_attribute_is_settable(candidate.element.as_ptr(), "AXFocused")
        && set_ax_bool_attribute(candidate.element.as_ptr(), "AXFocused", true)
    {
        std::thread::sleep(Duration::from_millis(50));
        let first = copy_ax_attribute(app, "AXFocusedUIElement")
            .is_some_and(|focused| elements_equal(focused.as_ptr(), candidate.element.as_ptr()));
        std::thread::sleep(Duration::from_millis(35));
        let stable = copy_ax_attribute(app, "AXFocusedUIElement")
            .is_some_and(|focused| elements_equal(focused.as_ptr(), candidate.element.as_ptr()));
        if first && stable {
            return Ok(NativeFocusAttempt::Focused(candidate_fingerprint(
                &candidate.resolver,
            )));
        }
    }
    Ok(NativeFocusAttempt::Rejected)
}

fn focus_editable_input_for_pid(
    pid: u32,
    bundle_id: &str,
    captured_window: &CandidateInput,
) -> Result<Option<ComposerFingerprint>, String> {
    let trusted_pids = discover_trusted_candidate_pids(pid, bundle_id);
    let profile = input_capability_profile(bundle_id, None);
    for candidate_pid in &trusted_pids {
        let Some(app) =
            OwnedCf::created(unsafe { AXUIElementCreateApplication(*candidate_pid as i32) })
        else {
            continue;
        };
        unsafe {
            AXUIElementSetMessagingTimeout(app.as_ptr(), 0.25);
        }
        match focus_editable_input_once(app.as_ptr(), &trusted_pids, captured_window) {
            Ok(NativeFocusAttempt::Focused(fingerprint)) => return Ok(Some(fingerprint)),
            Ok(NativeFocusAttempt::Rejected) => {}
            Ok(NativeFocusAttempt::SparseTree) => {
                let permits_manual_accessibility = matches!(
                    profile,
                    InputCapabilityProfile::Accessibility(accessibility)
                        if accessibility.manual_accessibility
                            == input_profiles::ManualAccessibilityPolicy::OnlyWhenTreeSparse
                );
                if !permits_manual_accessibility || *candidate_pid != pid {
                    continue;
                }
                set_ax_bool_attribute(app.as_ptr(), "AXManualAccessibility", true);
                std::thread::sleep(Duration::from_millis(100));
                if let NativeFocusAttempt::Focused(fingerprint) =
                    focus_editable_input_once(app.as_ptr(), &trusted_pids, captured_window)?
                {
                    return Ok(Some(fingerprint));
                }
            }
            Err(error) => eprintln!("Initial AX input resolution failed: {}", error),
        }
    }
    Ok(None)
}

fn verify_focused_editable_for_pid(pid: u32) -> Result<Option<u32>, String> {
    let app = OwnedCf::created(unsafe { AXUIElementCreateApplication(pid as i32) })
        .ok_or_else(|| "Could not create the target accessibility element.".to_string())?;
    unsafe {
        AXUIElementSetMessagingTimeout(app.as_ptr(), 0.2);
    }
    Ok(focused_editable_pid(app.as_ptr()))
}

fn focused_composer_matching(
    trusted_pids: &[u32],
    expected: &ComposerFingerprint,
) -> Option<NativeEditableCandidate> {
    trusted_pids.iter().find_map(|pid| {
        let app = OwnedCf::created(unsafe { AXUIElementCreateApplication(*pid as i32) })?;
        unsafe {
            AXUIElementSetMessagingTimeout(app.as_ptr(), 0.2);
        }
        let candidate = focused_editable_candidate(app.as_ptr())?;
        fingerprints_match(expected, &candidate_fingerprint(&candidate.resolver))
            .then_some(candidate)
    })
}

fn paste_evidence_snapshot(element: AXUIElementRef) -> PasteEvidenceSnapshot {
    PasteEvidenceSnapshot {
        value_hash: ax_string_attribute(element, "AXValue").map(|value| {
            let mut hasher = DefaultHasher::new();
            value.hash(&mut hasher);
            hasher.finish()
        }),
        selected_range: ax_range_attribute(element, "AXSelectedTextRange"),
    }
}

fn paste_evidence_changed(
    policy: input_profiles::PasteVerificationPolicy,
    before: &PasteEvidenceSnapshot,
    after: &PasteEvidenceSnapshot,
) -> bool {
    match policy {
        input_profiles::PasteVerificationPolicy::ValueLengthOrHashChange => {
            before.value_hash.is_some()
                && after.value_hash.is_some()
                && before.value_hash != after.value_hash
        }
        input_profiles::PasteVerificationPolicy::SelectionRangeChange => {
            before.selected_range.is_some()
                && after.selected_range.is_some()
                && before.selected_range != after.selected_range
        }
        input_profiles::PasteVerificationPolicy::FocusStableAfterProfiledDelay { .. }
        | input_profiles::PasteVerificationPolicy::PasteOnlyWithoutSubmitEvidence => false,
    }
}

pub fn current_input_target() -> Option<InputTarget> {
    let app_info = frontmost_app_info()?;

    // Reject PP itself. PID comparison is authoritative regardless of how PP was
    // launched (.app bundle vs raw binary). The string checks remain as a
    // secondary defense for callers that might construct FrontmostAppInfo without
    // a real pid.
    if app_info.pid == std::process::id()
        || app_info.app.bundle_id == "local.promptpicker.dev"
        || app_info.app.name == "Prompt Drawer"
    {
        return None;
    }

    get_focused_input_element(app_info.pid, app_info.app.clone())
}

fn pointer_location_from_quartz_point(x: f64, y: f64) -> (f64, f64) {
    (x, y)
}

pub fn current_pointer_location() -> Option<(f64, f64)> {
    let source = CGEventSource::new(CGEventSourceStateID::CombinedSessionState).ok()?;
    let event = CGEvent::new(source).ok()?;
    let point = event.location();

    Some(pointer_location_from_quartz_point(point.x, point.y))
}

fn get_focused_input_element(pid: u32, app: FrontmostApp) -> Option<InputTarget> {
    let script = format!(
        r#"on run
tell application "System Events"
    tell (first process whose unix id is {})
        set frontWin to front window
        set winPos to position of frontWin
        set winSize to size of frontWin
        set elemPos to {{0, 0}}
        set elemSize to {{0, 0}}
        try
            set focusedElem to value of attribute "AXFocusedUIElement" of frontWin
            if focusedElem is not missing value then
                set elemPos to position of focusedElem
                set elemSize to size of focusedElem
            end if
        end try
        return (item 1 of winPos as string) & "," & (item 2 of winPos as string) & "|" & (item 1 of winSize as string) & "," & (item 2 of winSize as string) & "|" & (item 1 of elemPos as string) & "," & (item 2 of elemPos as string) & "|" & (item 1 of elemSize as string) & "," & (item 2 of elemSize as string)
    end tell
end tell
end run"#,
        pid
    );

    let output = Command::new("osascript")
        .arg("-e")
        .arg(&script)
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut target = parse_focused_input_output(stdout.trim(), &app)?;
    target.pid = pid;
    Some(target)
}

fn parse_xy(s: &str) -> Option<(f64, f64)> {
    let parts: Vec<&str> = s.split(',').collect();
    if parts.len() != 2 {
        return None;
    }
    let x: f64 = parts[0].trim().parse().ok()?;
    let y: f64 = parts[1].trim().parse().ok()?;
    Some((x, y))
}

// ── Paste ─────────────────────────────────────────────────────────────────────

const DIRECT_TYPE_MAX_CHARS: usize = 500;

#[allow(dead_code)]
type CGEventSourceRef = *mut c_void;
#[allow(dead_code)]
type CGEventRef = *mut c_void;
#[allow(dead_code)]
type CGEventFlags = u64;
#[allow(dead_code)]
type CGKeyCode = u16;
type AXUIElementRef = *const c_void;
type CFTypeRef = *const c_void;

#[allow(dead_code)]
const CG_EVENT_FLAG_MASK_COMMAND: CGEventFlags = 1 << 20;
#[allow(dead_code)]
const KEY_CODE_V: CGKeyCode = 9;
#[allow(dead_code)]
const KEY_CODE_RETURN: CGKeyCode = 36;
#[allow(dead_code)]
const KEY_CODE_COMMAND: CGKeyCode = 55;
#[allow(dead_code)]
const CG_HID_EVENT_TAP: u32 = 0;

#[link(name = "ApplicationServices", kind = "framework")]
extern "C" {
    fn CGEventCreateKeyboardEvent(
        source: CGEventSourceRef,
        virtual_key: CGKeyCode,
        key_down: bool,
    ) -> CGEventRef;
    fn CGEventSetFlags(event: CGEventRef, flags: CGEventFlags);
    fn CGEventPost(tap: u32, event: CGEventRef);
    fn CFRelease(cf: *const c_void);
}

#[link(name = "ApplicationServices", kind = "framework")]
extern "C" {
    fn AXUIElementCreateApplication(pid: i32) -> AXUIElementRef;
    fn AXUIElementSetMessagingTimeout(element: AXUIElementRef, timeout: f32) -> i32;
}

#[allow(dead_code)]
fn post_key_event(key_code: CGKeyCode, key_down: bool, flags: CGEventFlags) -> Result<(), String> {
    unsafe {
        let event = CGEventCreateKeyboardEvent(std::ptr::null_mut(), key_code, key_down);
        if event.is_null() {
            return Err("CGEventCreateKeyboardEvent returned null".to_string());
        }
        CGEventSetFlags(event, flags);
        CGEventPost(CG_HID_EVENT_TAP, event);
        CFRelease(event.cast_const());
    }
    Ok(())
}

#[allow(dead_code)]
fn post_key_tap(key_code: CGKeyCode, flags: CGEventFlags) -> Result<(), String> {
    post_key_event(key_code, true, flags)?;
    post_key_event(key_code, false, flags)
}

#[allow(dead_code)]
fn post_paste_shortcut() -> Result<(), String> {
    post_key_event(KEY_CODE_COMMAND, true, CG_EVENT_FLAG_MASK_COMMAND)?;
    post_key_tap(KEY_CODE_V, CG_EVENT_FLAG_MASK_COMMAND)?;
    post_key_event(KEY_CODE_COMMAND, false, 0)
}

#[allow(dead_code)]
fn post_return_key() -> Result<(), String> {
    post_key_tap(KEY_CODE_RETURN, 0)
}

pub fn post_focus_preserving_paste() -> Result<(), String> {
    post_paste_shortcut()
}

pub fn post_focus_preserving_submit_key(submit_key: NativeSubmitKey) -> Result<(), String> {
    match submit_key {
        NativeSubmitKey::None => Ok(()),
        NativeSubmitKey::Enter => post_return_key(),
        NativeSubmitKey::CommandEnter => post_command_return_key(),
    }
}

#[allow(dead_code)]
pub fn focus_preserving_paste_and_submit(submit_key: NativeSubmitKey) -> AutosendOutcome {
    if let Err(error) = post_focus_preserving_paste() {
        return AutosendOutcome::paste_event_failed(format_autosend_error(
            "focus-preserving-paste",
            &error,
        ));
    }
    if let Err(error) = post_focus_preserving_submit_key(submit_key) {
        return AutosendOutcome::return_event_failed(format_autosend_error(
            "focus-preserving-submit",
            &error,
        ));
    }
    if submit_key == NativeSubmitKey::None {
        AutosendOutcome::pasted_only()
    } else {
        AutosendOutcome::sent()
    }
}

fn recover_target_after_activation(
    bundle_id: &str,
    target_pid: u32,
    target_launch_identity: ProcessLaunchIdentity,
    click_point: Option<(f64, f64)>,
    captured_window: Option<&CandidateInput>,
) -> Result<Option<ComposerFingerprint>, String> {
    if !wait_for_frontmost_target(
        bundle_id,
        target_pid,
        target_launch_identity,
        Duration::from_millis(1_500),
    ) {
        return Err(format!(
            "Target app did not become frontmost: {}",
            bundle_id
        ));
    }

    std::thread::sleep(Duration::from_millis(160));

    let _app_info = frontmost_app_info()
        .filter(|info| info.app.bundle_id == bundle_id && info.pid == target_pid)
        .ok_or_else(|| {
            format!(
                "Target app is not frontmost after activation: {}",
                bundle_id
            )
        })?;
    verify_captured_window_for_policy(
        input_focus_policy(bundle_id),
        target_pid,
        captured_window,
    )?;
    match input_focus_policy(bundle_id) {
        InputFocusPolicy::PreserveApplicationFirstResponder => {
            if let Some((x, y)) = click_point {
                click_target_point(x, y)
                    .map_err(|error| format_autosend_error("click-input-target", &error))?;
            }
            Ok(None)
        }
        InputFocusPolicy::ResolveEditableElement => {
            let captured_window = captured_window
                .ok_or_else(|| "The captured target window is no longer available.".to_string())?;
            focus_editable_input_for_pid(target_pid, bundle_id, captured_window)?.ok_or_else(|| {
                "No unambiguous editable composer was found in the captured window.".to_string()
            }).map(Some)
        }
    }
}

fn prepare_focus_for_policy_with_ops<N, C, V>(
    policy: InputFocusPolicy,
    app_pid: u32,
    click_point: Option<(f64, f64)>,
    native_focus: N,
    click_target: C,
    verify_focus: V,
) -> Result<Option<u32>, String>
where
    N: FnOnce(u32) -> Result<Option<u32>, String>,
    C: FnOnce(f64, f64) -> Result<(), String>,
    V: FnOnce(u32) -> Result<Option<u32>, String>,
{
    if policy == InputFocusPolicy::PreserveApplicationFirstResponder {
        if let Some((x, y)) = click_point {
            click_target(x, y)?;
        }
        return Ok(None);
    }

    match native_focus(app_pid) {
        Ok(Some(element_pid)) => return Ok(Some(element_pid)),
        Ok(None) => Err("No editable input element was found in the target window.".to_string()),
        Err(error) => Err(error),
    }
}

#[allow(dead_code)]
pub fn repair_focus_to_editable_element(pid: u32) -> Result<(), String> {
    ensure_accessibility_trusted_with(is_accessibility_trusted)?;
    run_system_events_script(&repair_focus_to_editable_element_script(pid))
        .map_err(|error| format_autosend_error("ax-focus-repair", &error))
}

fn repair_focus_to_editable_element_script(pid: u32) -> String {
    format!(
        r#"on run
	tell application "System Events"
	    tell (first process whose unix id is {})
	        set frontWin to front window
	        set editableRoles to {{"AXTextArea", "AXTextField", "AXSearchField", "AXComboBox", "AXWebArea"}}
	        try
	            set focusedElem to value of attribute "AXFocusedUIElement" of frontWin
	            if focusedElem is not missing value then
	                try
	                    set focusedRole to role of focusedElem as string
	                    if focusedRole is in editableRoles then
	                        set focused of focusedElem to true
	                        return "focused-current-editable"
	                    end if
	                end try
	            end if
	        end try
	        try
	            set winPos to position of frontWin
	            set winSize to size of frontWin
	        on error
	            set winPos to {{0, 0}}
	            set winSize to {{0, 0}}
	        end try
	        set bestElem to missing value
	        set bestScore to -100000
	        repeat with elem in entire contents of frontWin
	            try
	                set elemRole to role of elem as string
	                if elemRole is in editableRoles then
	                    set elemScore to 0
	                    if elemRole is "AXTextArea" then
	                        set elemScore to elemScore + 60
	                    else if elemRole is "AXTextField" then
	                        set elemScore to elemScore + 35
	                    else if elemRole is "AXComboBox" then
	                        set elemScore to elemScore + 25
	                    else if elemRole is "AXWebArea" then
	                        set elemScore to elemScore + 20
	                    else if elemRole is "AXSearchField" then
	                        set elemScore to elemScore - 35
	                    end if
	                    try
	                        if enabled of elem is true then set elemScore to elemScore + 10
	                    end try
	                    try
	                        set elemPos to position of elem
	                        set elemSize to size of elem
	                        if item 1 of elemSize > 80 and item 2 of elemSize > 18 then set elemScore to elemScore + 10
	                        if item 2 of elemSize > 40 then set elemScore to elemScore + 20
	                        if item 2 of elemPos > (item 2 of winPos + (item 2 of winSize * 0.45)) then set elemScore to elemScore + 12
	                    end try
	                    if elemScore > bestScore then
	                        set bestScore to elemScore
	                        set bestElem to elem
	                    end if
	                end if
	            end try
	        end repeat
	        if bestElem is not missing value then
	            set focused of bestElem to true
	            return "focused-best-editable"
	        end if
	    end tell
	end tell
	error "No editable AX element found"
	end run"#,
        pid
    )
}

fn post_command_return_key() -> Result<(), String> {
    post_key_event(KEY_CODE_COMMAND, true, CG_EVENT_FLAG_MASK_COMMAND)?;
    post_key_tap(KEY_CODE_RETURN, CG_EVENT_FLAG_MASK_COMMAND)?;
    post_key_event(KEY_CODE_COMMAND, false, 0)
}

#[cfg(test)]
fn native_autosend_event_sequence() -> Vec<&'static str> {
    vec![
        "cmd-down",
        "v-down",
        "v-up",
        "cmd-up",
        "return-down",
        "return-up",
    ]
}

#[cfg(test)]
fn native_autosend_uses_osascript() -> bool {
    false
}

fn paste_prompt_with_accessibility_gate<C, T>(
    body: &str,
    copy_sender: C,
    is_trusted: T,
) -> Result<(), String>
where
    C: FnOnce(&str) -> Result<(), String>,
    T: FnOnce() -> bool,
{
    ensure_accessibility_trusted_with(is_trusted)?;
    copy_sender(body)?;
    Command::new("osascript")
        .args([
            "-e",
            "tell application \"System Events\" to keystroke \"v\" using command down",
        ])
        .output()
        .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn paste_prompt_with_copier<C>(body: &str, copy_sender: C) -> Result<(), String>
where
    C: FnOnce(&str) -> Result<(), String>,
{
    paste_prompt_with_accessibility_gate(body, copy_sender, is_accessibility_trusted)
}

#[allow(dead_code)]
pub fn paste_prompt_and_submit_to_app_with_copier<C>(
    body: &str,
    bundle_id: &str,
    copy_sender: C,
) -> Result<(), String>
where
    C: FnOnce(&str) -> Result<(), String>,
{
    ensure_accessibility_trusted_with(is_accessibility_trusted)?;
    copy_sender(body)?;
    let script = paste_and_submit_to_app_script(bundle_id);
    let output = Command::new("osascript")
        .arg("-e")
        .arg(&script)
        .output()
        .map_err(|e| e.to_string())?;
    if !output.status.success() {
        return Err(format_autosend_error(
            "paste-and-submit",
            String::from_utf8_lossy(&output.stderr).as_ref(),
        ));
    }
    Ok(())
}

pub fn paste_prompt_and_submit_to_app_clipboard_with_copier<C, A>(
    body: &str,
    bundle_id: &str,
    target_pid: u32,
    target_launch_identity: ProcessLaunchIdentity,
    click_point: Option<(f64, f64)>,
    captured_window: Option<&CandidateInput>,
    submit_key: NativeSubmitKey,
    mut activate_target: A,
    copy_sender: C,
) -> AutosendOutcome
where
    C: FnOnce(&str) -> Result<(), String>,
    A: FnMut(u32) -> Result<(), String>,
{
    let focused_composer = std::cell::RefCell::new(None::<ComposerFingerprint>);
    let before_paste = std::cell::RefCell::new(None::<PasteEvidenceSnapshot>);
    let observed_version = app_version_for_pid(target_pid);
    let profile = input_capability_profile(bundle_id, observed_version.as_deref());
    paste_prompt_and_submit_to_app_clipboard_with_ops(
        body,
        bundle_id,
        click_point,
        captured_window,
        submit_key,
        copy_sender,
        is_accessibility_trusted,
        |bundle_id, click_point, captured_window| {
            activate_target(target_pid)?;
            let composer = recover_target_after_activation(
                bundle_id,
                target_pid,
                target_launch_identity,
                click_point,
                captured_window,
            )?;
            *focused_composer.borrow_mut() = composer;
            Ok(())
        },
        |bundle_id, captured_window| {
            let composer = focused_composer.borrow();
            let verified = verify_target_focus_for_autosend(
                bundle_id,
                target_pid,
                target_launch_identity,
                captured_window,
                composer.as_ref(),
            );
            if verified {
                if let Some(composer) = composer.as_ref() {
                    let trusted_pids = discover_trusted_candidate_pids(target_pid, bundle_id);
                    if let Some(candidate) = focused_composer_matching(&trusted_pids, composer) {
                        *before_paste.borrow_mut() =
                            Some(paste_evidence_snapshot(candidate.element.as_ptr()));
                    }
                }
            }
            verified
        },
        post_paste_shortcut,
        || match profile {
            InputCapabilityProfile::CodexFirstResponder
            | InputCapabilityProfile::LegacyCapturedTarget => true,
            InputCapabilityProfile::Accessibility(accessibility) => {
                let Some(composer) = focused_composer.borrow().clone() else {
                    return false;
                };
                let trusted_pids = discover_trusted_candidate_pids(target_pid, bundle_id);
                let Some(candidate) = focused_composer_matching(&trusted_pids, &composer) else {
                    return false;
                };
                let Some(before) = before_paste.borrow().as_ref().cloned() else {
                    return false;
                };
                let after = paste_evidence_snapshot(candidate.element.as_ptr());
                paste_evidence_changed(accessibility.paste_verification, &before, &after)
            }
        },
        post_focus_preserving_submit_key,
        std::thread::sleep,
    )
}

fn paste_prompt_and_submit_to_app_clipboard_with_ops<C, T, R, V, P, E, S, W>(
    body: &str,
    bundle_id: &str,
    click_point: Option<(f64, f64)>,
    captured_window: Option<&CandidateInput>,
    submit_key: NativeSubmitKey,
    copy_sender: C,
    is_trusted: T,
    mut recover_target: R,
    mut verify_target: V,
    mut paste_sender: P,
    mut verify_paste: E,
    mut submit_sender: S,
    mut sleeper: W,
) -> AutosendOutcome
where
    C: FnOnce(&str) -> Result<(), String>,
    T: FnOnce() -> bool,
    R: FnMut(&str, Option<(f64, f64)>, Option<&CandidateInput>) -> Result<(), String>,
    V: FnMut(&str, Option<&CandidateInput>) -> bool,
    P: FnMut() -> Result<(), String>,
    E: FnMut() -> bool,
    S: FnMut(NativeSubmitKey) -> Result<(), String>,
    W: FnMut(Duration),
{
    if !is_trusted() {
        return AutosendOutcome::missing_accessibility_permission();
    }
    let mut copy_sender = Some(copy_sender);
    let transaction = run_transaction(
        submit_key,
        || true,
        || true,
        || recover_target(bundle_id, click_point, captured_window).is_ok(),
        || {
            copy_sender
                .take()
                .is_some_and(|copy_sender| copy_sender(body).is_ok())
        },
        || verify_target(bundle_id, captured_window),
        || paste_sender().is_ok(),
        || {
            sleeper(Duration::from_millis(220));
            verify_paste()
        },
        |key| submit_sender(key).is_ok(),
    );

    match transaction.failure {
        None if submit_key == NativeSubmitKey::None => AutosendOutcome::pasted_only(),
        None => AutosendOutcome::sent(),
        Some(TransactionFailure::TargetChanged) => AutosendOutcome::failed(
            false,
            AutosendFailureReason::TargetChanged,
            "The target app or composer changed during prompt delivery.".to_string(),
        ),
        Some(TransactionFailure::ComposerUnavailable) => AutosendOutcome::failed(
            false,
            AutosendFailureReason::ComposerNotFound,
            "No unambiguous composer was found in the captured window.".to_string(),
        ),
        Some(TransactionFailure::FocusNotAcquired) => AutosendOutcome::failed(
            false,
            AutosendFailureReason::FocusNotAcquired,
            "The target composer could not be focused and verified.".to_string(),
        ),
        Some(TransactionFailure::ClipboardWriteFailed) => {
            AutosendOutcome::copy_failed("Clipboard write failed.".to_string())
        }
        Some(TransactionFailure::PasteEventFailed) => AutosendOutcome::paste_event_failed(
            "The prompt paste event failed; it was not retried.".to_string(),
        ),
        Some(TransactionFailure::PasteNotConfirmed) => AutosendOutcome::failed(
            true,
            AutosendFailureReason::PasteNotConfirmed,
            "The prompt paste could not be confirmed; it was not retried.".to_string(),
        ),
        Some(TransactionFailure::SubmitEventFailed) => AutosendOutcome::return_event_failed(
            "The prompt was pasted, but the submit key failed.".to_string(),
        ),
    }
}

fn app_version_for_pid(pid: u32) -> Option<String> {
    let output = Command::new("lsappinfo")
        .args(["info", "-pid", &pid.to_string()])
        .output()
        .ok()?;
    let info = String::from_utf8_lossy(&output.stdout);
    let app_path = parse_quoted_lsappinfo_value(&info, "bundlepath")
        .or_else(|| parse_quoted_lsappinfo_value(&info, "bundle path"))?;
    let info_path = format!("{app_path}/Contents/Info");
    let output = Command::new("defaults")
        .args(["read", &info_path, "CFBundleShortVersionString"])
        .output()
        .ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).trim().to_string())
        .filter(|version| !version.is_empty())
}

fn parse_quoted_lsappinfo_value(info: &str, key: &str) -> Option<String> {
    let marker = format!("{key}=\"");
    let value = info.split(&marker).nth(1)?;
    Some(value.split('"').next()?.to_string())
}

fn verify_target_focus_for_autosend(
    bundle_id: &str,
    target_pid: u32,
    target_launch_identity: ProcessLaunchIdentity,
    captured_window: Option<&CandidateInput>,
    expected_composer: Option<&ComposerFingerprint>,
) -> bool {
    if !frontmost_target_matches(bundle_id, target_pid, target_launch_identity) {
        return false;
    }
    let policy = input_focus_policy(bundle_id);
    if verify_captured_window_for_policy(policy, target_pid, captured_window).is_err() {
        return false;
    }
    match policy {
        InputFocusPolicy::PreserveApplicationFirstResponder => true,
        InputFocusPolicy::ResolveEditableElement => {
            let Some(expected_composer) = expected_composer else {
                return false;
            };
            let trusted_pids = discover_trusted_candidate_pids(target_pid, bundle_id);
            focused_composer_matching(&trusted_pids, expected_composer).is_some()
        }
    }
}

fn verify_captured_window_for_policy(
    policy: InputFocusPolicy,
    pid: u32,
    captured_window: Option<&CandidateInput>,
) -> Result<(), String> {
    if policy == InputFocusPolicy::PreserveApplicationFirstResponder {
        return Ok(());
    }
    let captured_window = captured_window
        .ok_or_else(|| "The captured target window is no longer available.".to_string())?;
    let app = OwnedCf::created(unsafe { AXUIElementCreateApplication(pid as i32) })
        .ok_or_else(|| "Could not create the target accessibility element.".to_string())?;
    let current = focused_window(app.as_ptr())
        .and_then(|window| ax_element_frame(window.as_ptr()))
        .ok_or_else(|| "The target app does not expose its focused window.".to_string())?;
    window_frames_match(captured_window, &current)
        .then_some(())
        .ok_or_else(|| "The target window changed before prompt delivery.".to_string())
}

fn window_frames_match(captured: &CandidateInput, current: &CandidateInput) -> bool {
    const TOLERANCE: f64 = 3.0;
    (captured.x - current.x).abs() <= TOLERANCE
        && (captured.y - current.y).abs() <= TOLERANCE
        && (captured.width - current.width).abs() <= TOLERANCE
        && (captured.height - current.height).abs() <= TOLERANCE
}

fn candidate_frames_overlap(left: &CandidateInput, right: &CandidateInput) -> bool {
    let width = (left.x + left.width).min(right.x + right.width) - left.x.max(right.x);
    let height = (left.y + left.height).min(right.y + right.height) - left.y.max(right.y);
    width > 0.0 && height > 0.0
}

#[allow(dead_code)]
pub fn type_or_paste_prompt_and_submit_to_app_with_copier<C>(
    body: &str,
    bundle_id: &str,
    copy_sender: C,
) -> Result<(), String>
where
    C: FnOnce(&str) -> Result<(), String>,
{
    ensure_accessibility_trusted_with(is_accessibility_trusted)?;
    restore_focus_before_autosend(bundle_id);

    let mut direct_type_error = None;
    if should_direct_type(body) {
        let script = direct_type_and_submit_to_app_script(bundle_id, body);
        let output = Command::new("osascript")
            .arg("-e")
            .arg(&script)
            .output()
            .map_err(|e| e.to_string())?;
        if output.status.success() {
            return Ok(());
        }
        direct_type_error = Some(format_autosend_error(
            "direct-type",
            String::from_utf8_lossy(&output.stderr).as_ref(),
        ));
    }

    paste_prompt_and_submit_to_app_with_copier(body, bundle_id, copy_sender).map_err(
        |paste_error| {
            if let Some(direct_error) = direct_type_error {
                format!("{} Fallback also failed: {}", direct_error, paste_error)
            } else {
                paste_error
            }
        },
    )
}

#[allow(dead_code)]
pub fn paste_prompt_and_submit_to_foreground_with_copier<C>(
    body: &str,
    copy_sender: C,
) -> Result<AutosendOutcome, String>
where
    C: FnOnce(&str) -> Result<(), String>,
{
    if let Some(outcome) = missing_accessibility_outcome_if_untrusted_with(is_accessibility_trusted)
    {
        return Ok(outcome);
    }
    if let Err(error) = copy_sender(body) {
        return Ok(AutosendOutcome::copy_failed(error));
    }
    refocus_previous_app_if_prompt_picker_frontmost();
    std::thread::sleep(std::time::Duration::from_millis(280));

    if let Err(error) = post_paste_shortcut() {
        return Ok(AutosendOutcome::paste_event_failed(format_autosend_error(
            "Native paste event failed",
            &error,
        )));
    }

    std::thread::sleep(std::time::Duration::from_millis(320));

    if let Err(error) = post_return_key() {
        return Ok(AutosendOutcome::return_event_failed(format_autosend_error(
            "Native return event failed",
            &error,
        )));
    }

    Ok(AutosendOutcome::sent())
}

#[allow(dead_code)]
pub fn type_or_paste_prompt_and_submit_to_foreground_with_copier<C>(
    body: &str,
    copy_sender: C,
) -> AutosendOutcome
where
    C: FnOnce(&str) -> Result<(), String>,
{
    if let Some(outcome) = missing_accessibility_outcome_if_untrusted_with(is_accessibility_trusted)
    {
        return outcome;
    }
    let copy_result = copy_sender(body);

    refocus_previous_app_if_prompt_picker_frontmost();
    std::thread::sleep(Duration::from_millis(320));

    let mut direct_type_error = None;
    if should_direct_type(body) {
        match run_system_events_script(&foreground_type_and_submit_script(body)) {
            Ok(()) => return AutosendOutcome::sent(),
            Err(error) => {
                direct_type_error = Some(error);
            }
        }
    }

    if let Err(error) = copy_result {
        return AutosendOutcome::copy_failed(error);
    }

    match run_system_events_script(foreground_paste_and_submit_script()) {
        Ok(()) => AutosendOutcome::sent(),
        Err(error) => {
            let paste_error = format_autosend_error("foreground-paste-and-submit", &error);
            let error = direct_type_error
                .map(|direct_error| {
                    format!(
                        "{} Fallback also failed: {}",
                        format_autosend_error("foreground-direct-type", &direct_error),
                        paste_error
                    )
                })
                .unwrap_or(paste_error);
            AutosendOutcome::paste_event_failed(error)
        }
    }
}

#[allow(dead_code)]
pub fn paste_prompt_and_submit_to_app_at_point_with_copier<C>(
    body: &str,
    bundle_id: &str,
    x: f64,
    y: f64,
    copy_sender: C,
) -> Result<(), String>
where
    C: FnOnce(&str) -> Result<(), String>,
{
    ensure_accessibility_trusted_with(is_accessibility_trusted)?;
    copy_sender(body)?;
    let script = paste_and_submit_to_app_at_point_script(bundle_id, x, y);
    let output = Command::new("osascript")
        .arg("-e")
        .arg(&script)
        .output()
        .map_err(|e| e.to_string())?;
    if !output.status.success() {
        return Err(format_autosend_error(
            "paste-and-submit-at-point",
            String::from_utf8_lossy(&output.stderr).as_ref(),
        ));
    }
    Ok(())
}

fn paste_and_submit_to_app_script(bundle_id: &str) -> String {
    format!(
        r#"tell application id "{}" to activate
delay 0.15
tell application "System Events"
    keystroke "v" using command down
    delay 0.1
    key code 36
end tell"#,
        bundle_id
    )
}

fn foreground_paste_and_submit_script() -> &'static str {
    r#"tell application "System Events"
    keystroke "v" using command down
    delay 0.1
    key code 36
end tell"#
}

fn run_system_events_script(script: &str) -> Result<(), String> {
    let output = Command::new("osascript")
        .arg("-e")
        .arg(script)
        .output()
        .map_err(|e| e.to_string())?;
    if output.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).to_string())
    }
}

#[allow(dead_code)]
fn click_target_point(x: f64, y: f64) -> Result<(), String> {
    run_system_events_script(&click_target_point_script(x, y))
}

fn click_target_point_script(x: f64, y: f64) -> String {
    format!(
        r#"tell application "System Events"
    click at {{{:.0}, {:.0}}}
end tell"#,
        x, y
    )
}

fn should_direct_type(body: &str) -> bool {
    body.is_ascii() && !body.contains('\n') && body.chars().count() <= DIRECT_TYPE_MAX_CHARS
}

fn escape_applescript_string(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

fn foreground_type_and_submit_script(body: &str) -> String {
    let escaped = escape_applescript_string(body);
    format!(
        r#"tell application "System Events"
    keystroke "{}"
    delay 0.08
    key code 36
end tell"#,
        escaped
    )
}

fn direct_type_and_submit_to_app_script(bundle_id: &str, body: &str) -> String {
    let escaped = escape_applescript_string(body);
    format!(
        r#"tell application id "{}" to activate
delay 0.15
tell application "System Events"
    keystroke "{}"
    delay 0.08
    key code 36
end tell"#,
        bundle_id, escaped
    )
}

fn format_autosend_error(method: &str, stderr: &str) -> String {
    let trimmed = stderr.trim();
    if trimmed.is_empty() {
        format!("Autosend failed while using {}.", method)
    } else {
        format!("Autosend failed while using {}: {}", method, trimmed)
    }
}

fn wait_for_frontmost_target(
    bundle_id: &str,
    target_pid: u32,
    launch_identity: ProcessLaunchIdentity,
    timeout: Duration,
) -> bool {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if frontmost_target_matches(bundle_id, target_pid, launch_identity) {
            return true;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    frontmost_target_matches(bundle_id, target_pid, launch_identity)
}

fn frontmost_target_matches(
    bundle_id: &str,
    target_pid: u32,
    launch_identity: ProcessLaunchIdentity,
) -> bool {
    frontmost_app_info().is_some_and(|info| {
        info.app.bundle_id == bundle_id
            && info.pid == target_pid
            && process_launch_identity(target_pid) == Some(launch_identity)
    })
}

fn should_cmd_tab_refocus_before_autosend(frontmost: Option<&FrontmostApp>) -> bool {
    frontmost
        .map(|app| app.bundle_id == "local.promptpicker.dev" || app.name == "Prompt Drawer")
        .unwrap_or(false)
}

fn cmd_tab_refocus_previous_app_script() -> &'static str {
    r#"tell application "System Events"
    key down command
    key code 48
    key up command
end tell"#
}

fn refocus_previous_app_if_prompt_picker_frontmost() {
    let frontmost = frontmost_app();
    if should_cmd_tab_refocus_before_autosend(frontmost.as_ref()) {
        match Command::new("osascript")
            .arg("-e")
            .arg(cmd_tab_refocus_previous_app_script())
            .output()
        {
            Ok(output) if output.status.success() => {}
            Ok(output) => eprintln!(
                "{}",
                format_autosend_error(
                    "cmd-tab-refocus",
                    String::from_utf8_lossy(&output.stderr).as_ref(),
                )
            ),
            Err(err) => eprintln!("Autosend failed while using cmd-tab-refocus: {}", err),
        }
    }
}

fn restore_focus_before_autosend(bundle_id: &str) {
    refocus_previous_app_if_prompt_picker_frontmost();
    let script = format!(r#"tell application id "{}" to activate"#, bundle_id);
    match Command::new("osascript").arg("-e").arg(script).output() {
        Ok(output) if output.status.success() => {}
        Ok(output) => eprintln!(
            "{}",
            format_autosend_error(
                "activate-target",
                String::from_utf8_lossy(&output.stderr).as_ref(),
            )
        ),
        Err(err) => eprintln!("Autosend failed while using activate-target: {}", err),
    }

    std::thread::sleep(std::time::Duration::from_millis(120));
}

fn paste_and_submit_to_app_at_point_script(bundle_id: &str, x: f64, y: f64) -> String {
    format!(
        r#"tell application id "{}" to activate
delay 0.2
tell application "System Events"
    click at {{{:.0}, {:.0}}}
    delay 0.12
    keystroke "v" using command down
    delay 0.1
    key code 36
end tell"#,
        bundle_id, x, y
    )
}

// ── Parsing helpers (pub for testing) ─────────────────────────────────────────

/// Parse "ASN:0x0-0x46046:\n" → "ASN:0x0-0x46046"
pub fn parse_front_asn(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.starts_with("ASN:") {
        return Some(trimmed.trim_end_matches(':').to_string());
    }
    None
}

pub fn parse_visible_process_asns(raw: &str) -> Vec<String> {
    raw.split("ASN:")
        .skip(1)
        .filter_map(|part| {
            let candidate = format!("ASN:{}", part.trim_start());
            let end = candidate.find("-\"")?;
            let asn = candidate[..end].trim().trim_end_matches(':');
            if asn.starts_with("ASN:") {
                Some(asn.to_string())
            } else {
                None
            }
        })
        .collect()
}

/// Parse bundle ID from lsappinfo info output (any format).
pub fn parse_bundle_id(s: &str) -> Option<String> {
    for line in s.lines() {
        let line = line.trim();
        // Handle bundleID="..." or bundleID = "..."
        if line.starts_with("bundleID") || line.starts_with("CFBundleIdentifier") {
            if let Some(eq) = line.find('=') {
                let raw = line[eq + 1..].trim();
                // lsappinfo always wraps real bundle ids in double quotes
                // (e.g., `bundleID="com.openai.codex"`). An unquoted value like
                // `bundleID=[ NULL ]` means the bundle id is unavailable — this
                // happens when a process is launched directly from a binary
                // rather than an .app bundle. Returning None here lets the caller
                // fall back to `unknown.{asn}`, which downstream code correctly
                // treats as "not a real target".
                if raw.starts_with('"') {
                    let val = raw.trim_matches('"').trim();
                    if !val.is_empty() {
                        return Some(val.to_string());
                    }
                }
            }
        }
    }
    None
}

/// Parse app name from lsappinfo info output.
pub fn parse_app_name(s: &str) -> Option<String> {
    for line in s.lines() {
        let line = line.trim();
        // Try LSApplicationName / CFBundleName
        if line.starts_with("LSApplicationName") || line.starts_with("CFBundleName") {
            if let Some(eq) = line.find('=') {
                let val = &line[eq + 1..].trim().trim_matches('"').trim_matches('\'');
                if !val.is_empty() {
                    return Some(val.to_string());
                }
            }
        }
    }

    let first_line = s.lines().next()?.trim();
    let rest = first_line.strip_prefix('"')?;
    let end = rest.find('"')?;
    let name = rest[..end].trim();
    if name.is_empty() {
        return None;
    }
    Some(name.to_string())
}

/// Parse pid from lsappinfo info output.
pub fn parse_pid(s: &str) -> Option<u32> {
    for line in s.lines() {
        let line = line.trim();
        let Some(rest) = line.strip_prefix("pid") else {
            continue;
        };
        let rest = rest.trim_start();
        let rest = rest.strip_prefix('=').or_else(|| rest.strip_prefix(':'))?;
        let value = rest.trim_start().split_whitespace().next()?;
        return value.parse::<u32>().ok();
    }
    None
}

/// Parse 4-field focused input output: "wx,wy|ww,wh|ex,ey|ew,eh"
pub fn parse_focused_input_output(raw: &str, app: &FrontmostApp) -> Option<InputTarget> {
    let parts: Vec<&str> = raw.trim().split('|').collect();
    if parts.len() != 4 {
        return None;
    }
    let window_pos = parse_xy(parts[0])?;
    let window_size = parse_xy(parts[1])?;
    let elem_pos = parse_xy(parts[2])?;
    let elem_size = parse_xy(parts[3])?;

    let window_frame = CandidateInput {
        x: window_pos.0,
        y: window_pos.1,
        width: window_size.0,
        height: window_size.1,
    };
    let frame = CandidateInput {
        x: elem_pos.0,
        y: elem_pos.1,
        width: elem_size.0,
        height: elem_size.1,
    };

    let focused_frame_available = frame.width > 1.0 && frame.height > 1.0;
    let button_position = if focused_frame_available {
        // Button anchors at bottom-right of focused element.
        (frame.x + frame.width, frame.y + frame.height)
    } else {
        fallback_button_position_for_window(&window_frame)
    };
    let click_point = if focused_frame_available {
        input_click_point_for_frame(&frame)
    } else {
        let fallback = fallback_click_point_for_app(app, &window_frame);
        (fallback.x, fallback.y)
    };

    Some(InputTarget {
        frame,
        window_frame,
        button_position,
        click_point,
        app: Some(app.clone()),
        pid: 0,
    })
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TargetClickPoint {
    pub x: f64,
    pub y: f64,
}

pub fn fallback_click_point_for_app(
    _app: &FrontmostApp,
    window_frame: &CandidateInput,
) -> TargetClickPoint {
    TargetClickPoint {
        x: (window_frame.x + (window_frame.width / 2.0))
            .clamp(window_frame.x, window_frame.x + window_frame.width),
        y: (window_frame.y + window_frame.height - 65.0)
            .clamp(window_frame.y, window_frame.y + window_frame.height),
    }
}

fn fallback_button_position_for_window(window_frame: &CandidateInput) -> (f64, f64) {
    (
        window_frame.x + window_frame.width - 24.0,
        window_frame.y + window_frame.height - 24.0,
    )
}

fn input_click_point_for_frame(frame: &CandidateInput) -> (f64, f64) {
    let x_offset = (frame.width * 0.12)
        .clamp(12.0, 80.0)
        .min(frame.width / 2.0);
    (frame.x + x_offset, frame.y + (frame.height / 2.0))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    fn run_activating_sender_with_ops(
        submit_key: NativeSubmitKey,
        click_point: Option<(f64, f64)>,
        trusted: bool,
    ) -> (
        AutosendOutcome,
        Vec<String>,
        Option<NativeSubmitKey>,
        Option<Option<(f64, f64)>>,
    ) {
        let events = RefCell::new(Vec::new());
        let submitted_key = RefCell::new(None);
        let recovered_point = RefCell::new(None);
        let outcome = paste_prompt_and_submit_to_app_clipboard_with_ops(
            "hello",
            "com.openai.codex",
            click_point,
            None,
            submit_key,
            |body| {
                events.borrow_mut().push(format!("copy:{body}"));
                Ok(())
            },
            || {
                events.borrow_mut().push("permission".to_string());
                trusted
            },
            |bundle_id, point, _| {
                events.borrow_mut().push(format!("recover:{bundle_id}"));
                recovered_point.replace(Some(point));
                Ok(())
            },
            |_, _| {
                events.borrow_mut().push("verify".to_string());
                true
            },
            || {
                events.borrow_mut().push("paste".to_string());
                Ok(())
            },
            || true,
            |key| {
                events.borrow_mut().push("submit".to_string());
                submitted_key.replace(Some(key));
                Ok(())
            },
            |_| events.borrow_mut().push("sleep".to_string()),
        );

        (
            outcome,
            events.into_inner(),
            submitted_key.into_inner(),
            recovered_point.into_inner(),
        )
    }

    #[test]
    fn autosend_outcome_reports_copy_failure() {
        let outcome = AutosendOutcome::copy_failed("clipboard failed".to_string());

        assert!(!outcome.copied);
        assert!(!outcome.sent);
        assert_eq!(outcome.error.as_deref(), Some("clipboard failed"));
        assert_eq!(outcome.reason, Some(AutosendFailureReason::CopyFailed));
    }

    #[test]
    fn autosend_outcome_reports_keyboard_failure_after_copy() {
        let outcome = AutosendOutcome::keyboard_failed("System Events denied".to_string());

        assert!(outcome.copied);
        assert!(!outcome.sent);
        assert_eq!(outcome.error.as_deref(), Some("System Events denied"));
        assert_eq!(
            outcome.reason,
            Some(AutosendFailureReason::PasteEventFailed)
        );
    }

    #[test]
    fn autosend_outcome_reports_native_keyboard_failure_after_copy() {
        let outcome = AutosendOutcome::keyboard_failed("native key event failed".to_string());

        assert!(outcome.copied);
        assert!(!outcome.sent);
        assert_eq!(outcome.error.as_deref(), Some("native key event failed"));
        assert_eq!(
            outcome.reason,
            Some(AutosendFailureReason::PasteEventFailed)
        );
    }

    #[test]
    fn autosend_outcome_reports_missing_accessibility_permission() {
        let outcome = AutosendOutcome::missing_accessibility_permission();

        assert!(!outcome.copied);
        assert!(!outcome.sent);
        assert_eq!(
            outcome.error.as_deref(),
            Some(ACCESSIBILITY_PERMISSION_REQUIRED_ERROR)
        );
        assert_eq!(
            outcome.reason,
            Some(AutosendFailureReason::MissingAccessibilityPermission)
        );
    }

    #[test]
    fn accessibility_gate_returns_specific_error_when_untrusted() {
        let result = ensure_accessibility_trusted_with(|| false);

        assert_eq!(
            result,
            Err(ACCESSIBILITY_PERMISSION_REQUIRED_ERROR.to_string())
        );
    }

    #[test]
    fn accessibility_gate_allows_work_when_trusted() {
        assert!(ensure_accessibility_trusted_with(|| true).is_ok());
    }

    #[test]
    fn accessibility_outcome_reports_no_copy_before_permission() {
        let outcome = missing_accessibility_outcome_if_untrusted_with(|| false).unwrap();

        assert!(!outcome.copied);
        assert!(!outcome.sent);
        assert_eq!(
            outcome.reason,
            Some(AutosendFailureReason::MissingAccessibilityPermission)
        );
    }

    #[test]
    fn plain_paste_does_not_copy_before_accessibility_permission() {
        let mut copied = false;

        let result = paste_prompt_with_accessibility_gate(
            "hello",
            |_| {
                copied = true;
                Ok(())
            },
            || false,
        );

        assert_eq!(
            result,
            Err(ACCESSIBILITY_PERMISSION_REQUIRED_ERROR.to_string())
        );
        assert!(!copied);
    }

    #[test]
    fn autosend_outcome_reports_return_key_failure_after_copy() {
        let outcome = AutosendOutcome::return_event_failed("return failed".to_string());

        assert!(outcome.copied);
        assert!(!outcome.sent);
        assert_eq!(
            outcome.reason,
            Some(AutosendFailureReason::ReturnEventFailed)
        );
    }

    #[test]
    fn autosend_outcome_reports_sent_after_keyboard_success() {
        let outcome = AutosendOutcome::sent();

        assert!(outcome.copied);
        assert!(outcome.sent);
        assert_eq!(outcome.completion, Some(AutosendCompletion::Submitted));
        assert!(outcome.error.is_none());
        assert!(outcome.reason.is_none());
    }

    #[test]
    fn activating_clipboard_sender_pastes_and_presses_return() {
        let (outcome, events, submitted_key, recovered_point) =
            run_activating_sender_with_ops(NativeSubmitKey::Enter, Some((640.0, 720.0)), true);

        assert!(outcome.sent);
        assert_eq!(outcome.completion, Some(AutosendCompletion::Submitted));
        assert_eq!(
            events,
            vec![
                "permission".to_string(),
                "recover:com.openai.codex".to_string(),
                "verify".to_string(),
                "copy:hello".to_string(),
                "verify".to_string(),
                "paste".to_string(),
                "sleep".to_string(),
                "verify".to_string(),
                "submit".to_string(),
            ]
        );
        assert_eq!(submitted_key, Some(NativeSubmitKey::Enter));
        assert_eq!(recovered_point, Some(Some((640.0, 720.0))));
    }

    #[test]
    fn activating_clipboard_sender_respects_command_enter() {
        let (outcome, _events, submitted_key, _recovered_point) =
            run_activating_sender_with_ops(NativeSubmitKey::CommandEnter, None, true);

        assert!(outcome.sent);
        assert_eq!(submitted_key, Some(NativeSubmitKey::CommandEnter));
    }

    #[test]
    fn activating_clipboard_sender_respects_submit_key_none() {
        let (outcome, events, submitted_key, _recovered_point) =
            run_activating_sender_with_ops(NativeSubmitKey::None, None, true);

        assert!(!outcome.sent);
        assert_eq!(outcome.completion, Some(AutosendCompletion::PastedOnly));
        assert_eq!(
            events,
            vec![
                "permission".to_string(),
                "recover:com.openai.codex".to_string(),
                "verify".to_string(),
                "copy:hello".to_string(),
                "verify".to_string(),
                "paste".to_string(),
            ]
        );
        assert_eq!(submitted_key, None);
    }

    #[test]
    fn activating_clipboard_sender_does_not_copy_without_accessibility_permission() {
        let outcome = paste_prompt_and_submit_to_app_clipboard_with_ops(
            "hello",
            "com.openai.codex",
            None,
            None,
            NativeSubmitKey::Enter,
            |_| panic!("copy must not run before accessibility permission"),
            || false,
            |_, _, _| panic!("recover must not run without accessibility permission"),
            |_, _| panic!("verify must not run without accessibility permission"),
            || panic!("paste must not run without accessibility permission"),
            || panic!("paste evidence must not run without accessibility permission"),
            |_| panic!("submit must not run without accessibility permission"),
            |_| panic!("sleep must not run without accessibility permission"),
        );

        assert!(!outcome.copied);
        assert!(!outcome.sent);
        assert_eq!(
            outcome.reason,
            Some(AutosendFailureReason::MissingAccessibilityPermission)
        );
    }

    #[test]
    fn activating_clipboard_sender_does_not_replace_clipboard_when_focus_fails() {
        let outcome = paste_prompt_and_submit_to_app_clipboard_with_ops(
            "hello",
            "com.anthropic.claudefordesktop",
            None,
            None,
            NativeSubmitKey::Enter,
            |_| panic!("copy must not run before target focus is verified"),
            || true,
            |_, _, _| Err("no editable input".to_string()),
            |_, _| panic!("verify must not run when focus recovery fails"),
            || panic!("paste must not run when focus recovery fails"),
            || panic!("paste evidence must not run when focus recovery fails"),
            |_| panic!("submit must not run when focus recovery fails"),
            |_| panic!("sleep must not run when focus recovery fails"),
        );

        assert!(!outcome.copied);
        assert!(!outcome.sent);
        assert_eq!(
            outcome.reason,
            Some(AutosendFailureReason::FocusNotAcquired)
        );
    }

    #[test]
    fn activating_clipboard_sender_never_retries_unknown_paste() {
        let copy_count = std::cell::Cell::new(0);
        let paste_count = std::cell::Cell::new(0);
        let outcome = paste_prompt_and_submit_to_app_clipboard_with_ops(
            "hello",
            "com.anthropic.claudefordesktop",
            None,
            None,
            NativeSubmitKey::Enter,
            |_| {
                copy_count.set(copy_count.get() + 1);
                Ok(())
            },
            || true,
            |_, _, _| Ok(()),
            |_, _| true,
            || {
                paste_count.set(paste_count.get() + 1);
                Err("paste completion unknown".to_string())
            },
            || panic!("paste evidence must not run after failed paste event"),
            |_| panic!("submit must not run after unknown paste completion"),
            |_| panic!("sleep must not run after a failed paste event"),
        );

        assert_eq!(copy_count.get(), 1);
        assert_eq!(paste_count.get(), 1);
        assert!(!outcome.sent);
        assert_eq!(
            outcome.reason,
            Some(AutosendFailureReason::PasteEventFailed)
        );
    }

    #[test]
    fn autosend_does_not_attempt_clipboard_restoration() {
        let source = include_str!("macos.rs");
        let start = source
            .find("fn paste_prompt_and_submit_to_app_clipboard_with_ops")
            .unwrap();
        let end = source[start..]
            .find("pub fn type_or_paste_prompt_and_submit_to_app_with_copier")
            .unwrap();
        let implementation = &source[start..start + end];

        assert!(!implementation.contains("restore"));
        assert!(!implementation.contains("NSPasteboard"));
    }

    #[test]
    fn exact_composer_verification_rejects_a_different_field() {
        let expected = ComposerFingerprint {
            owner_pid: 42,
            role: "AXTextArea".to_string(),
            subrole: None,
            identifier_hash: Some("composer".to_string()),
            frame: CandidateInput {
                x: 100.0,
                y: 200.0,
                width: 500.0,
                height: 120.0,
            },
        };
        let mut search = expected.clone();
        search.identifier_hash = Some("search".to_string());
        search.frame.y = 40.0;

        assert!(fingerprints_match(&expected, &expected));
        assert!(!fingerprints_match(&expected, &search));
    }

    #[test]
    fn paste_evidence_requires_the_profiled_observation_to_change() {
        let before = PasteEvidenceSnapshot {
            value_hash: Some(1),
            selected_range: Some((0, 0)),
        };
        let after = PasteEvidenceSnapshot {
            value_hash: Some(2),
            selected_range: Some((12, 0)),
        };

        assert!(paste_evidence_changed(
            input_profiles::PasteVerificationPolicy::ValueLengthOrHashChange,
            &before,
            &after,
        ));
        assert!(paste_evidence_changed(
            input_profiles::PasteVerificationPolicy::SelectionRangeChange,
            &before,
            &after,
        ));
        assert!(!paste_evidence_changed(
            input_profiles::PasteVerificationPolicy::PasteOnlyWithoutSubmitEvidence,
            &before,
            &after,
        ));
    }

    #[test]
    fn activating_clipboard_sender_pastes_without_click_point() {
        let (outcome, events, _submitted_key, recovered_point) =
            run_activating_sender_with_ops(NativeSubmitKey::Enter, None, true);

        assert!(outcome.sent);
        assert!(events.contains(&"paste".to_string()));
        assert_eq!(recovered_point, Some(None));
    }

    #[test]
    fn activating_clipboard_sender_does_not_call_ax_repair() {
        let source = include_str!("macos.rs");
        let start = source
            .find("pub fn paste_prompt_and_submit_to_app_clipboard_with_copier")
            .expect("activating sender should exist");
        let end = source[start..]
            .find("#[allow(dead_code)]")
            .expect("next legacy helper should follow activating sender");
        let sender_source = &source[start..start + end];

        assert!(!sender_source.contains("repair_focus_to_editable_element"));
        assert!(!sender_source.contains("AXFocusedUIElement"));
    }

    #[test]
    fn accessibility_settings_url_targets_privacy_accessibility() {
        assert!(accessibility_settings_url().contains("Privacy_Accessibility"));
    }

    #[test]
    fn native_autosend_sequence_uses_paste_then_return() {
        let sequence = native_autosend_event_sequence();

        assert_eq!(
            sequence,
            vec![
                "cmd-down",
                "v-down",
                "v-up",
                "cmd-up",
                "return-down",
                "return-up",
            ]
        );
    }

    #[test]
    fn native_autosend_does_not_depend_on_osascript() {
        assert!(!native_autosend_uses_osascript());
    }

    #[test]
    fn pure_focus_preserving_event_sender_does_not_activate_or_click() {
        let source = include_str!("macos.rs");
        let start = source
            .find("pub fn focus_preserving_paste_and_submit")
            .expect("focus-preserving autosend function should exist");
        let end = source[start..]
            .find("fn recover_target_after_activation")
            .expect("target recovery function should follow pure sender");
        let pure_sender_source = &source[start..start + end];

        assert!(!pure_sender_source.contains("activate_app_by_bundle_id"));
        assert!(!pure_sender_source.contains("click_target_point"));
        assert!(pure_sender_source.contains("post_focus_preserving_paste"));
        assert!(pure_sender_source.contains("post_focus_preserving_submit_key"));
    }

    #[test]
    fn activating_clipboard_sender_is_available_without_prompt_body_scripting() {
        let source = include_str!("macos.rs");
        let start = source
            .find("pub fn paste_prompt_and_submit_to_app_clipboard_with_copier")
            .expect("activating sender should exist");
        let end = source[start..]
            .find("#[allow(dead_code)]")
            .expect("next legacy helper should follow activating sender");
        let sender_source = &source[start..start + end];

        assert!(sender_source.contains("recover_target_app_for_autosend"));
        assert!(sender_source.contains("post_focus_preserving_submit_key"));
        assert!(!sender_source.contains("keystroke \"{body}\""));
        assert!(!sender_source.contains("keystroke \"Test body\""));
    }

    #[test]
    fn native_submit_key_supports_command_enter() {
        assert_eq!(NativeSubmitKey::CommandEnter, NativeSubmitKey::CommandEnter);
    }

    #[test]
    fn codex_keeps_activation_only_focus_policy() {
        assert_eq!(
            input_focus_policy("com.openai.codex"),
            InputFocusPolicy::PreserveApplicationFirstResponder
        );
    }

    #[test]
    fn other_apps_use_verified_editable_focus_policy() {
        assert_eq!(
            input_focus_policy("com.anthropic.claudefordesktop"),
            InputFocusPolicy::ResolveEditableElement
        );
        assert_eq!(
            input_focus_policy("com.tencent.xinWeChat"),
            InputFocusPolicy::ResolveEditableElement
        );
    }

    #[test]
    fn codex_focus_preparation_does_not_invoke_native_ax_resolution() {
        let mut clicked = None;
        let result = prepare_focus_for_policy_with_ops(
            InputFocusPolicy::PreserveApplicationFirstResponder,
            42,
            Some((640.0, 720.0)),
            |_| panic!("Codex must keep the existing activation-only path"),
            |x, y| {
                clicked = Some((x, y));
                Ok(())
            },
            |_| panic!("Codex must not require AX focus verification"),
        );

        assert!(result.is_ok());
        assert_eq!(clicked, Some((640.0, 720.0)));
    }

    #[test]
    fn verified_native_focus_skips_coordinate_click_for_other_apps() {
        let result = prepare_focus_for_policy_with_ops(
            InputFocusPolicy::ResolveEditableElement,
            84,
            Some((640.0, 720.0)),
            |pid| {
                assert_eq!(pid, 84);
                Ok(Some(86))
            },
            |_, _| panic!("verified native focus must avoid coordinate fallback"),
            |_| panic!("native focus result is already verified"),
        );

        assert_eq!(result, Ok(Some(86)));
    }

    #[test]
    fn coordinate_fallback_for_other_apps_requires_focus_verification() {
        let result = prepare_focus_for_policy_with_ops(
            InputFocusPolicy::ResolveEditableElement,
            84,
            Some((640.0, 720.0)),
            |_| Ok(None),
            |_, _| Ok(()),
            |pid| {
                assert_eq!(pid, 84);
                Ok(None)
            },
        );

        assert!(result.is_err());
    }

    #[test]
    fn editable_candidate_scoring_prefers_large_lower_text_area_over_search() {
        let window = CandidateInput {
            x: 0.0,
            y: 0.0,
            width: 1200.0,
            height: 900.0,
        };
        let composer = EditableCandidate {
            role: EditableRole::TextArea,
            frame: CandidateInput {
                x: 220.0,
                y: 650.0,
                width: 760.0,
                height: 150.0,
            },
            enabled: true,
            focused: false,
            depth: 5,
        };
        let search = EditableCandidate {
            role: EditableRole::SearchField,
            frame: CandidateInput {
                x: 20.0,
                y: 40.0,
                width: 280.0,
                height: 32.0,
            },
            enabled: true,
            focused: false,
            depth: 3,
        };

        assert!(
            editable_candidate_score(&composer, &window)
                > editable_candidate_score(&search, &window)
        );
    }

    #[test]
    fn editable_candidate_selection_rejects_ambiguous_fields() {
        let window = CandidateInput {
            x: 0.0,
            y: 0.0,
            width: 1000.0,
            height: 800.0,
        };
        let candidates = vec![
            EditableCandidate {
                role: EditableRole::TextArea,
                frame: CandidateInput {
                    x: 100.0,
                    y: 580.0,
                    width: 700.0,
                    height: 100.0,
                },
                enabled: true,
                focused: false,
                depth: 4,
            },
            EditableCandidate {
                role: EditableRole::TextArea,
                frame: CandidateInput {
                    x: 100.0,
                    y: 590.0,
                    width: 700.0,
                    height: 100.0,
                },
                enabled: true,
                focused: false,
                depth: 4,
            },
        ];

        assert_eq!(select_editable_candidate(&candidates, &window), None);
    }

    #[test]
    fn search_and_non_editable_web_areas_cannot_receive_prompts() {
        assert!(!role_can_receive_prompt(
            EditableRole::SearchField,
            true,
            true
        ));
        assert!(!role_can_receive_prompt(
            EditableRole::WebArea,
            false,
            false
        ));
        assert!(!role_can_receive_prompt(EditableRole::WebArea, true, false));
    }

    #[test]
    fn ax_repair_script_uses_generic_editable_roles_without_app_recipes() {
        let script = repair_focus_to_editable_element_script(123);

        assert!(script.contains("AXTextArea"));
        assert!(script.contains("AXTextField"));
        assert!(script.contains("AXSearchField"));
        assert!(script.contains("AXComboBox"));
        assert!(script.contains("AXWebArea"));
        assert!(script.contains("entire contents of frontWin"));
        assert!(script.contains("focusedRole"));
        assert!(script.contains("bestScore"));
        assert!(script.contains("bestElem"));
        assert!(script.contains("set elemScore to elemScore - 35"));
        assert!(script.contains("item 2 of elemPos >"));
        assert!(script.contains("item 1 of elemSize > 80"));
        assert!(!script.contains("WeChat"));
        assert!(!script.contains("Claude"));
        assert!(!script.contains("Codex"));
    }

    #[test]
    fn clipboard_copy_does_not_shell_out_to_system_command() {
        let source = include_str!("macos.rs");

        assert!(!source.contains(&format!("Command::new(\"{}\")", concat!("pb", "copy"))));
    }

    #[test]
    fn click_target_point_script_only_clicks_recorded_point() {
        let script = click_target_point_script(640.0, 720.0);

        assert!(script.contains("tell application \"System Events\""));
        assert!(script.contains("click at {640, 720}"));
        assert!(!script.contains("keystroke"));
        assert!(!script.contains("key code 36"));
    }

    #[test]
    fn target_recovery_function_waits_for_exact_process_and_clicks_only_optional_point() {
        let source = include_str!("macos.rs");
        let start = source
            .find("fn recover_target_after_activation")
            .expect("target recovery function should exist");
        let end = source[start..]
            .find("fn paste_and_submit_to_app_script")
            .expect("next helper should exist");
        let recovery_source = &source[start..start + end];

        assert!(!recovery_source.contains("activate_app_by_bundle_id"));
        assert!(recovery_source.contains("wait_for_frontmost_target"));
        assert!(recovery_source.contains("click_target_point"));
        assert!(recovery_source.contains("Duration::from_millis(1_500)"));
    }

    #[test]
    fn parses_lsappinfo_front_asn_output() {
        assert_eq!(
            parse_front_asn("ASN:0x0-0x46046:\n").as_deref(),
            Some("ASN:0x0-0x46046")
        );
        // Trailing colon variant
        assert_eq!(
            parse_front_asn("ASN:0x0-0x46046:").as_deref(),
            Some("ASN:0x0-0x46046")
        );
    }

    #[test]
    fn parses_visible_process_list_asns_in_order() {
        let raw =
            r#"ASN:0x0-0x3b03b-"Codex": ASN:0x0-0x10010-"Finder": ASN:0x0-0x1398397-"WeChat":"#;

        assert_eq!(
            parse_visible_process_asns(raw),
            vec![
                "ASN:0x0-0x3b03b".to_string(),
                "ASN:0x0-0x10010".to_string(),
                "ASN:0x0-0x1398397".to_string(),
            ]
        );
    }

    #[test]
    fn parses_bundle_id_various_formats() {
        assert_eq!(
            parse_bundle_id("bundleID=\"com.openai.codex\"\nfoo"),
            Some("com.openai.codex".to_string())
        );
        assert_eq!(
            parse_bundle_id("bundleID = \"com.apple.Safari\"\n"),
            Some("com.apple.Safari".to_string())
        );
        assert_eq!(
            parse_bundle_id("CFBundleIdentifier = \"com.github.GitHubDesktop\"\n"),
            Some("com.github.GitHubDesktop".to_string())
        );
    }

    #[test]
    fn parses_bundle_id_rejects_lsappinfo_null_placeholder() {
        // lsappinfo outputs `bundleID=[ NULL ]` (unquoted) when a process has no
        // registered bundle id — e.g., when launched directly from a binary instead
        // of an .app bundle. The parser must treat this as missing.
        assert_eq!(parse_bundle_id("bundleID=[ NULL ]\n"), None);
        assert_eq!(parse_bundle_id("    bundleID=[ NULL ]\n"), None);
        assert_eq!(parse_bundle_id("bundleID=[NULL]\n"), None);
        assert_eq!(parse_bundle_id("bundleID=\n"), None);
        // Quoted empty string is also missing.
        assert_eq!(parse_bundle_id("bundleID=\"\"\n"), None);
    }

    #[test]
    fn parses_app_name() {
        assert_eq!(
            parse_app_name(
                "\"Finder\" ASN:0x0-0xe00e: (in front)\n    bundleID=\"com.apple.finder\""
            ),
            Some("Finder".to_string())
        );
        // LSApplicationName format
        assert_eq!(
            parse_app_name("bundleID=\"com.apple.finder\"\nLSApplicationName=\"Finder\""),
            Some("Finder".to_string())
        );
        // CFBundleName variant
        assert_eq!(
            parse_app_name("CFBundleName=\"Safari\"\nbundleID=\"com.apple.Safari\""),
            Some("Safari".to_string())
        );
    }

    #[test]
    fn parses_pid_and_rejects_invalid_pid() {
        assert_eq!(
            parse_pid("\"Finder\" ASN:0x0-0xe00e:\n    pid = 650 type=\"Foreground\""),
            Some(650)
        );
        assert_eq!(parse_pid("pid: 12345 type=\"Foreground\""), Some(12345));
        assert!(parse_pid("pid = not-a-number type=\"Foreground\"").is_none());
        assert!(parse_pid("bundleID=\"com.apple.finder\"").is_none());
    }

    #[test]
    fn parses_app_info_from_lsappinfo_pid_output() {
        let info = r#""Claude" ASN:0x0-0xadbbdb1:
    bundleID="com.anthropic.claudefordesktop"
    pid = 67565 type="Foreground""#;

        let parsed = app_info_from_lsappinfo_output(info).unwrap();
        assert_eq!(parsed.pid, 67565);
        assert_eq!(parsed.app.name, "Claude");
        assert_eq!(parsed.app.bundle_id, "com.anthropic.claudefordesktop");
    }

    #[test]
    fn parses_focused_input_output() {
        let app = FrontmostApp {
            name: "Codex".to_string(),
            bundle_id: "com.openai.codex".to_string(),
        };
        let target = parse_focused_input_output("10,20|1200,800|700,680|500,96", &app).unwrap();

        assert_eq!(target.window_frame.x, 10.0);
        assert_eq!(target.window_frame.width, 1200.0);
        assert_eq!(target.frame.x, 700.0);
        assert_eq!(target.frame.height, 96.0);
        // button = elem_pos + elem_size = (700+500, 680+96) = (1200, 776)
        assert_eq!(target.button_position, (1200.0, 776.0));
        assert_eq!(target.click_point, (760.0, 728.0));
    }

    #[test]
    fn parses_focused_input_output_with_fallback_click_point() {
        let app = FrontmostApp {
            name: "Codex".to_string(),
            bundle_id: "com.openai.codex".to_string(),
        };
        let target = parse_focused_input_output("100,200|800,600|0,0|0,0", &app).unwrap();

        assert_eq!(target.window_frame.x, 100.0);
        assert_eq!(target.frame.width, 0.0);
        assert_eq!(target.click_point, (500.0, 735.0));
        assert_eq!(target.button_position, (876.0, 776.0));
    }

    #[test]
    fn generic_fallback_click_point_uses_bottom_center_of_window() {
        let point = fallback_click_point_for_app(
            &FrontmostApp {
                name: "WeChat".to_string(),
                bundle_id: "com.tencent.xinWeChat".to_string(),
            },
            &CandidateInput {
                x: 100.0,
                y: 200.0,
                width: 800.0,
                height: 600.0,
            },
        );

        assert_eq!(point.x, 500.0);
        assert_eq!(point.y, 735.0);
    }

    #[test]
    fn pointer_location_does_not_depend_on_appkit_main_thread_marker() {
        let production_source = include_str!("macos.rs")
            .split("#[cfg(test)]")
            .next()
            .expect("production source should precede tests");

        assert!(!production_source.contains("MainThreadMarker::new()?"));
        assert!(production_source.contains("CGEvent::new"));
        assert!(!production_source.contains("NSEvent::mouseLocation"));
    }

    #[test]
    fn quartz_pointer_location_uses_global_screen_coordinates_without_main_display_shift() {
        assert_eq!(
            pointer_location_from_quartz_point(120.0, 140.0),
            (120.0, 140.0)
        );
    }

    #[test]
    fn focused_input_click_point_is_inside_input_not_button_anchor() {
        let app = FrontmostApp {
            name: "Codex".to_string(),
            bundle_id: "com.openai.codex".to_string(),
        };
        let target = parse_focused_input_output("10,20|1200,800|700,680|500,96", &app).unwrap();

        assert!(target.click_point.0 > target.frame.x);
        assert!(target.click_point.0 < target.button_position.0);
        assert_eq!(
            target.click_point.1,
            target.frame.y + (target.frame.height / 2.0)
        );
    }

    #[test]
    fn parse_focused_input_rejects_wrong_field_count() {
        let app = FrontmostApp {
            name: "Test".to_string(),
            bundle_id: "com.test".to_string(),
        };
        assert!(parse_focused_input_output("10,20|1200,800|700,680", &app).is_none());
        assert!(parse_focused_input_output("10,20|1200,800|700,680|500,96|extra", &app).is_none());
    }

    #[test]
    fn paste_and_submit_to_app_script_activates_target_pastes_and_presses_return() {
        let script = paste_and_submit_to_app_script("com.openai.codex");

        assert!(script.contains("tell application id \"com.openai.codex\" to activate"));
        assert!(script.contains("keystroke \"v\" using command down"));
        assert!(script.contains("key code 36"));
    }

    #[test]
    fn paste_and_submit_to_app_script_uses_clipboard_not_literal_prompt_text() {
        let script = paste_and_submit_to_app_script("com.openai.codex");

        assert!(!script.contains("keystroke \"{body}\""));
        assert!(!script.contains("Test body"));
    }

    #[test]
    fn direct_type_and_submit_script_activates_target_before_typing() {
        let script = direct_type_and_submit_to_app_script("com.openai.codex", "讨论方案");

        assert!(script.contains("tell application id \"com.openai.codex\" to activate"));
        assert!(script.contains("tell application \"System Events\""));
        assert!(script.contains("keystroke \"讨论方案\""));
        assert!(script.contains("key code 36"));
    }

    #[test]
    fn direct_type_and_submit_script_escapes_quotes_and_backslashes() {
        let script = direct_type_and_submit_to_app_script("com.test.App", "say \"hi\" \\ ok");

        assert!(script.contains("keystroke \"say \\\"hi\\\" \\\\ ok\""));
    }

    #[test]
    fn direct_type_strategy_prefers_paste_for_multiline_text() {
        assert!(!should_direct_type("line 1\nline 2"));
    }

    #[test]
    fn direct_type_strategy_prefers_paste_for_long_text() {
        let long = "x".repeat(700);

        assert!(!should_direct_type(&long));
    }

    #[test]
    fn direct_type_strategy_rejects_non_ascii_text() {
        assert!(!should_direct_type(
            "使用 brainstorming skill，先和我讨论方案。"
        ));
    }

    #[test]
    fn direct_type_strategy_allows_short_ascii_single_line_text() {
        assert!(should_direct_type("Use brainstorming skill first."));
    }

    #[test]
    fn autosend_direct_script_does_not_click_coordinates() {
        let script = direct_type_and_submit_to_app_script("com.tencent.xinWeChat", "讨论方案");

        assert!(!script.contains("click at"));
        assert!(script.contains("keystroke \"讨论方案\""));
        assert!(script.contains("key code 36"));
    }

    #[test]
    fn paste_and_submit_script_remains_available_as_fallback() {
        let script = paste_and_submit_to_app_script("com.tencent.xinWeChat");

        assert!(script.contains("keystroke \"v\" using command down"));
        assert!(script.contains("key code 36"));
    }

    #[test]
    fn foreground_paste_and_submit_script_matches_openwhip_focus_model() {
        let script = foreground_paste_and_submit_script();

        assert!(script.contains("tell application \"System Events\""));
        assert!(script.contains("keystroke \"v\" using command down"));
        assert!(script.contains("key code 36"));
        assert!(!script.contains("tell application id"));
        assert!(!script.contains("click at"));
    }

    #[test]
    fn foreground_type_and_submit_script_matches_openwhip_focus_model() {
        let script = foreground_type_and_submit_script("讨论方案");

        assert!(script.contains("tell application \"System Events\""));
        assert!(script.contains("keystroke \"讨论方案\""));
        assert!(script.contains("key code 36"));
        assert!(!script.contains("tell application id"));
        assert!(!script.contains("click at"));
    }

    #[test]
    fn autosend_error_includes_stderr_when_osascript_fails() {
        let err = format_autosend_error("direct-type", "System Events got an error");

        assert!(err.contains("direct-type"));
        assert!(err.contains("System Events got an error"));
    }

    #[test]
    fn autosend_error_handles_empty_stderr() {
        let err = format_autosend_error("direct-type", "");

        assert_eq!(err, "Autosend failed while using direct-type.");
    }

    #[test]
    fn cmd_tab_refocus_script_matches_openwhip_pattern() {
        let script = cmd_tab_refocus_previous_app_script();

        assert!(script.contains("tell application \"System Events\""));
        assert!(script.contains("key down command"));
        assert!(script.contains("key code 48"));
        assert!(script.contains("key up command"));
    }

    #[test]
    fn should_cmd_tab_refocus_only_when_prompt_picker_is_frontmost() {
        assert!(should_cmd_tab_refocus_before_autosend(Some(
            &FrontmostApp {
                name: "Prompt Drawer".to_string(),
                bundle_id: "local.promptpicker.dev".to_string(),
            }
        )));

        assert!(!should_cmd_tab_refocus_before_autosend(Some(
            &FrontmostApp {
                name: "Codex".to_string(),
                bundle_id: "com.openai.codex".to_string(),
            }
        )));

        assert!(!should_cmd_tab_refocus_before_autosend(None));
    }

    #[test]
    fn paste_and_submit_script_clicks_recorded_point_before_paste() {
        let script = paste_and_submit_to_app_at_point_script("com.openai.codex", 640.0, 720.0);

        assert!(script.contains("tell application id \"com.openai.codex\" to activate"));
        assert!(script.contains("click at {640, 720}"));
        assert!(script.contains("keystroke \"v\" using command down"));
        assert!(script.contains("key code 36"));
        assert!(
            script.find("click at {640, 720}").unwrap()
                < script.find("keystroke \"v\" using command down").unwrap()
        );
    }

    #[test]
    fn native_input_diagnostics_registers_reusable_ax_modules() {
        let source = include_str!("macos.rs");
        let production = source
            .split("#[cfg(test)]")
            .next()
            .expect("production source should precede tests");

        assert!(production.contains("mod ax_client;"));
        assert!(production.contains("mod ax_diagnostics;"));
        assert!(!production.contains("pub mod ax_diagnostics;"));
    }
}
