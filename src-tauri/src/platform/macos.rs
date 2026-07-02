#![cfg(target_os = "macos")]

use serde::Serialize;
use std::ffi::c_void;
use std::process::Command;

#[derive(Clone, Debug, Serialize)]
pub struct FrontmostApp {
    pub name: String,
    pub bundle_id: String,
}

#[derive(Debug, Serialize)]
pub struct AccessibilityStatus {
    pub trusted: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct AutosendOutcome {
    pub copied: bool,
    pub sent: bool,
    pub error: Option<String>,
}

impl AutosendOutcome {
    pub fn sent() -> Self {
        Self {
            copied: true,
            sent: true,
            error: None,
        }
    }

    pub fn copy_failed(error: String) -> Self {
        Self {
            copied: false,
            sent: false,
            error: Some(error),
        }
    }

    pub fn keyboard_failed(error: String) -> Self {
        Self {
            copied: true,
            sent: false,
            error: Some(error),
        }
    }
}

// ── Accessibility ──────────────────────────────────────────────────────────────

pub fn accessibility_status() -> AccessibilityStatus {
    AccessibilityStatus {
        trusted: is_accessibility_trusted(),
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

fn is_accessibility_trusted() -> bool {
    unsafe { ax_is_process_trusted() }
}

// ── Frontmost App ─────────────────────────────────────────────────────────────

pub fn frontmost_app() -> Option<FrontmostApp> {
    frontmost_app_info().map(|info| info.app)
}

struct FrontmostAppInfo {
    app: FrontmostApp,
    pid: u32,
}

fn frontmost_app_info() -> Option<FrontmostAppInfo> {
    let front = Command::new("lsappinfo").arg("front").output().ok()?;
    let asn = parse_front_asn(String::from_utf8_lossy(&front.stdout).as_ref())?;

    let info = Command::new("lsappinfo")
        .args(["info", &asn])
        .output()
        .ok()?;

    let info_stdout = String::from_utf8_lossy(&info.stdout);
    let info_trimmed = info_stdout.trim();

    let name = parse_app_name(info_trimmed).unwrap_or_else(|| "Unknown".to_string());
    let bundle_id = parse_bundle_id(info_trimmed).unwrap_or_else(|| format!("unknown.{}", &asn));
    let pid = parse_pid(info_trimmed)?;

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
}

#[derive(Clone, Debug, Serialize)]
pub struct CandidateInput {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

pub fn current_input_target() -> Option<InputTarget> {
    let app_info = frontmost_app_info()?;

    if app_info.app.bundle_id == "local.promptpicker.dev" || app_info.app.name == "Prompt Picker" {
        return None;
    }

    get_focused_input_element(app_info.pid, app_info.app.clone())
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
    parse_focused_input_output(stdout.trim(), &app)
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

type CGEventSourceRef = *mut c_void;
type CGEventRef = *mut c_void;
type CGEventFlags = u64;
type CGKeyCode = u16;

const CG_EVENT_FLAG_MASK_COMMAND: CGEventFlags = 1 << 20;
const KEY_CODE_V: CGKeyCode = 9;
const KEY_CODE_RETURN: CGKeyCode = 36;
const KEY_CODE_COMMAND: CGKeyCode = 55;
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

fn post_key_tap(key_code: CGKeyCode, flags: CGEventFlags) -> Result<(), String> {
    post_key_event(key_code, true, flags)?;
    post_key_event(key_code, false, flags)
}

fn post_paste_shortcut() -> Result<(), String> {
    post_key_event(KEY_CODE_COMMAND, true, CG_EVENT_FLAG_MASK_COMMAND)?;
    post_key_tap(KEY_CODE_V, CG_EVENT_FLAG_MASK_COMMAND)?;
    post_key_event(KEY_CODE_COMMAND, false, 0)
}

fn post_return_key() -> Result<(), String> {
    post_key_tap(KEY_CODE_RETURN, 0)
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

pub fn paste_prompt(body: &str) -> Result<(), String> {
    copy_to_clipboard(body)?;
    Command::new("osascript")
        .args([
            "-e",
            "tell application \"System Events\" to keystroke \"v\" using command down",
        ])
        .output()
        .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn paste_prompt_to_app(body: &str, bundle_id: &str) -> Result<(), String> {
    copy_to_clipboard(body)?;
    let script = paste_to_app_script(bundle_id);
    let output = Command::new("osascript")
        .arg("-e")
        .arg(&script)
        .output()
        .map_err(|e| e.to_string())?;
    if !output.status.success() {
        return Err(format_autosend_error(
            "paste",
            String::from_utf8_lossy(&output.stderr).as_ref(),
        ));
    }
    Ok(())
}

#[allow(dead_code)]
pub fn paste_prompt_and_submit_to_app(body: &str, bundle_id: &str) -> Result<(), String> {
    copy_to_clipboard(body)?;
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

#[allow(dead_code)]
pub fn type_or_paste_prompt_and_submit_to_app(body: &str, bundle_id: &str) -> Result<(), String> {
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

    paste_prompt_and_submit_to_app(body, bundle_id).map_err(|paste_error| {
        if let Some(direct_error) = direct_type_error {
            format!("{} Fallback also failed: {}", direct_error, paste_error)
        } else {
            paste_error
        }
    })
}

pub fn paste_prompt_and_submit_to_foreground(body: &str) -> Result<AutosendOutcome, String> {
    if let Err(error) = copy_to_clipboard(body) {
        return Ok(AutosendOutcome::copy_failed(error));
    }
    refocus_previous_app_if_prompt_picker_frontmost();

    let script = foreground_paste_and_submit_script();
    let output = Command::new("osascript")
        .arg("-e")
        .arg(script)
        .output()
        .map_err(|e| e.to_string())?;
    if !output.status.success() {
        return Ok(AutosendOutcome::keyboard_failed(format_autosend_error(
            "foreground-paste-and-submit",
            String::from_utf8_lossy(&output.stderr).as_ref(),
        )));
    }
    Ok(AutosendOutcome::sent())
}

#[allow(dead_code)]
pub fn paste_prompt_and_submit_to_app_at_point(
    body: &str,
    bundle_id: &str,
    x: f64,
    y: f64,
) -> Result<(), String> {
    copy_to_clipboard(body)?;
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

fn paste_to_app_script(bundle_id: &str) -> String {
    format!(
        r#"tell application id "{}" to activate
delay 0.1
tell application "System Events" to keystroke "v" using command down"#,
        bundle_id
    )
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

fn should_direct_type(body: &str) -> bool {
    !body.contains('\n') && body.chars().count() <= DIRECT_TYPE_MAX_CHARS
}

fn escape_applescript_string(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
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

fn should_cmd_tab_refocus_before_autosend(frontmost: Option<&FrontmostApp>) -> bool {
    frontmost
        .map(|app| app.bundle_id == "local.promptpicker.dev" || app.name == "Prompt Picker")
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

#[allow(dead_code)]
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

fn copy_to_clipboard(body: &str) -> Result<(), String> {
    use std::io::Write;
    let mut child = Command::new("pbcopy")
        .stdin(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| e.to_string())?;
    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(body.as_bytes())
            .map_err(|e| e.to_string())?;
    }
    child.wait().map_err(|e| e.to_string())?;
    Ok(())
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

/// Parse bundle ID from lsappinfo info output (any format).
pub fn parse_bundle_id(s: &str) -> Option<String> {
    for line in s.lines() {
        let line = line.trim();
        // Handle bundleID="..." or bundleID = "..."
        if line.starts_with("bundleID") || line.starts_with("CFBundleIdentifier") {
            if let Some(eq) = line.find('=') {
                let val = &line[eq + 1..].trim().trim_matches('"').trim_matches('\'');
                if !val.is_empty() {
                    return Some(val.to_string());
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
    })
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TargetClickPoint {
    pub x: f64,
    pub y: f64,
}

pub fn fallback_click_point_for_app(
    app: &FrontmostApp,
    window_frame: &CandidateInput,
) -> TargetClickPoint {
    if app.bundle_id == "com.openai.codex" || app.name == "Codex" {
        return TargetClickPoint {
            x: window_frame.x + (window_frame.width / 2.0),
            y: window_frame.y + window_frame.height - 65.0,
        };
    }

    TargetClickPoint {
        x: window_frame.x + (window_frame.width / 2.0),
        y: window_frame.y + (window_frame.height / 2.0),
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

    #[test]
    fn autosend_outcome_reports_copy_failure() {
        let outcome = AutosendOutcome::copy_failed("pbcopy failed".to_string());

        assert!(!outcome.copied);
        assert!(!outcome.sent);
        assert_eq!(outcome.error.as_deref(), Some("pbcopy failed"));
    }

    #[test]
    fn autosend_outcome_reports_keyboard_failure_after_copy() {
        let outcome = AutosendOutcome::keyboard_failed("System Events denied".to_string());

        assert!(outcome.copied);
        assert!(!outcome.sent);
        assert_eq!(outcome.error.as_deref(), Some("System Events denied"));
    }

    #[test]
    fn autosend_outcome_reports_sent_after_keyboard_success() {
        let outcome = AutosendOutcome::sent();

        assert!(outcome.copied);
        assert!(outcome.sent);
        assert!(outcome.error.is_none());
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
    fn codex_fallback_click_point_uses_bottom_center_of_window() {
        let point = fallback_click_point_for_app(
            &FrontmostApp {
                name: "Codex".to_string(),
                bundle_id: "com.openai.codex".to_string(),
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
    fn direct_type_strategy_allows_short_single_line_text() {
        assert!(should_direct_type(
            "使用 brainstorming skill，先和我讨论方案。"
        ));
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
                name: "Prompt Picker".to_string(),
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
    fn paste_to_app_script_activates_target_bundle_before_paste() {
        let script = paste_to_app_script("com.apple.Notes");

        assert!(script.contains("tell application id \"com.apple.Notes\" to activate"));
        assert!(script.contains("keystroke \"v\" using command down"));
    }
}
