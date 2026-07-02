#![cfg(target_os = "macos")]

use serde::Serialize;
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

// ── Accessibility ──────────────────────────────────────────────────────────────

pub fn accessibility_status() -> AccessibilityStatus {
    AccessibilityStatus {
        trusted: is_accessibility_trusted(),
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
        set focusedElem to focused UI element of frontWin
        set elemPos to {{0, 0}}
        set elemSize to {{0, 0}}
        try
            set elemPos to position of focusedElem
            set elemSize to size of focusedElem
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
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }
    Ok(())
}

pub fn paste_prompt_and_submit_to_app(body: &str, bundle_id: &str) -> Result<(), String> {
    copy_to_clipboard(body)?;
    let script = paste_and_submit_to_app_script(bundle_id);
    let output = Command::new("osascript")
        .arg("-e")
        .arg(&script)
        .output()
        .map_err(|e| e.to_string())?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
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

    // Button anchors at bottom-right of focused element
    let button_x = elem_pos.0 + elem_size.0;
    let button_y = elem_pos.1 + elem_size.1;

    Some(InputTarget {
        frame,
        window_frame,
        button_position: (button_x, button_y),
        app: Some(app.clone()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn paste_to_app_script_activates_target_bundle_before_paste() {
        let script = paste_to_app_script("com.apple.Notes");

        assert!(script.contains("tell application id \"com.apple.Notes\" to activate"));
        assert!(script.contains("keystroke \"v\" using command down"));
    }
}
