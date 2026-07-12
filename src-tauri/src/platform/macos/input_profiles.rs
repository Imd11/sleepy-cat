use super::process_group::ProcessScope;

const EDITABLE_ROLES: &[&str] = &["AXTextArea", "AXTextField", "AXComboBox"];
const FORBIDDEN_SUBROLES: &[&str] = &["AXSearchField"];
const SEMANTIC_EXCLUSIONS: &[&str] = &["search", "find", "filter"];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum InputCapabilityProfile {
    CodexFirstResponder,
    Accessibility(AccessibilityProfile),
    LegacyCapturedTarget,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct AccessibilityProfile {
    pub process_scope: ProcessScope,
    pub window_identity: WindowIdentityRequirement,
    pub manual_accessibility: ManualAccessibilityPolicy,
    pub allowed_roles: &'static [&'static str],
    pub forbidden_subroles: &'static [&'static str],
    pub semantic_exclusions: &'static [&'static str],
    pub submit_key: SubmitKeyPolicy,
    pub paste_verification: PasteVerificationPolicy,
    pub focus_acquisition: FocusAcquisitionPolicy,
    pub permits_calibrated_window_point: bool,
    pub permits_web_area_composer: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum FocusAcquisitionPolicy {
    ExactAccessibility,
    CalibratedWindowPoint {
        horizontal_percent: u8,
        bottom_offset: u16,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum WindowIdentityRequirement {
    Required,
    BestEffort,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ManualAccessibilityPolicy {
    Never,
    OnlyWhenTreeSparse,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum SubmitKeyPolicy {
    Enter,
    CommandEnter,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum PasteVerificationPolicy {
    ValueLengthOrHashChange,
    SelectionRangeChange,
    FocusStableAfterProfiledDelay { min_ms: u64, max_ms: u64 },
    PasteOnlyWithoutSubmitEvidence,
}

pub(super) fn input_capability_profile(
    bundle_id: &str,
    observed_version: Option<&str>,
) -> InputCapabilityProfile {
    match bundle_id {
        "com.openai.codex" => InputCapabilityProfile::CodexFirstResponder,
        "com.anthropic.claudefordesktop" => calibrated_profile(80),
        "com.tencent.xinWeChat" => InputCapabilityProfile::Accessibility(accessibility_profile(
            ProcessScope::MainAndValidatedBrowserApplications,
            ManualAccessibilityPolicy::Never,
            (observed_version == Some("4.1.2"))
                .then_some(FocusAcquisitionPolicy::CalibratedWindowPoint {
                    horizontal_percent: 50,
                    bottom_offset: 65,
                })
                .unwrap_or(FocusAcquisitionPolicy::ExactAccessibility),
            PasteVerificationPolicy::PasteOnlyWithoutSubmitEvidence,
        )),
        bundle_id if is_supported_browser(bundle_id) => calibrated_profile(80),
        _ => InputCapabilityProfile::LegacyCapturedTarget,
    }
}

pub(super) fn input_capability_profile_for_page(
    bundle_id: &str,
    observed_version: Option<&str>,
    page_url: Option<&str>,
) -> InputCapabilityProfile {
    if is_supported_browser(bundle_id) {
        return calibrated_profile(
            page_url
                .and_then(calibrated_bottom_offset_for_url)
                .unwrap_or(80),
        );
    }
    input_capability_profile(bundle_id, observed_version)
}

fn calibrated_bottom_offset_for_url(url: &str) -> Option<u16> {
    let (_, remainder) = url.trim().split_once("://")?;
    let authority = remainder.split(['/', '?', '#']).next()?;
    let host = authority
        .rsplit('@')
        .next()?
        .split(':')
        .next()?
        .trim_end_matches('.')
        .to_ascii_lowercase();

    [
        ("chatgpt.com", 64),
        ("gemini.google.com", 88),
        ("manus.im", 72),
    ]
    .into_iter()
    .find_map(|(domain, offset)| {
        (host == domain || host.ends_with(&format!(".{domain}"))).then_some(offset)
    })
}

pub(super) fn is_supported_browser(bundle_id: &str) -> bool {
    matches!(
        bundle_id,
        "com.apple.Safari"
            | "com.google.Chrome"
            | "com.microsoft.edgemac"
            | "com.brave.Browser"
            | "company.thebrowser.Browser"
            | "org.mozilla.firefox"
    )
}

fn calibrated_profile(bottom_offset: u16) -> InputCapabilityProfile {
    InputCapabilityProfile::Accessibility(accessibility_profile(
        ProcessScope::MainOnly,
        ManualAccessibilityPolicy::Never,
        FocusAcquisitionPolicy::CalibratedWindowPoint {
            horizontal_percent: 50,
            bottom_offset,
        },
        PasteVerificationPolicy::FocusStableAfterProfiledDelay {
            min_ms: 180,
            max_ms: 420,
        },
    ))
}

fn accessibility_profile(
    process_scope: ProcessScope,
    manual_accessibility: ManualAccessibilityPolicy,
    focus_acquisition: FocusAcquisitionPolicy,
    paste_verification: PasteVerificationPolicy,
) -> AccessibilityProfile {
    AccessibilityProfile {
        process_scope,
        window_identity: WindowIdentityRequirement::Required,
        manual_accessibility,
        allowed_roles: EDITABLE_ROLES,
        forbidden_subroles: FORBIDDEN_SUBROLES,
        semantic_exclusions: SEMANTIC_EXCLUSIONS,
        submit_key: SubmitKeyPolicy::Enter,
        paste_verification,
        focus_acquisition,
        permits_calibrated_window_point: matches!(
            focus_acquisition,
            FocusAcquisitionPolicy::CalibratedWindowPoint { .. }
        ),
        permits_web_area_composer: false,
    }
}

impl AccessibilityProfile {
    pub(super) fn permits_submit(self) -> bool {
        self.paste_verification != PasteVerificationPolicy::PasteOnlyWithoutSubmitEvidence
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn input_profiles_keep_codex_and_unknown_apps_on_compatibility_routes() {
        assert_eq!(
            input_capability_profile("com.openai.codex", None),
            InputCapabilityProfile::CodexFirstResponder
        );
        assert_eq!(
            input_capability_profile("com.apple.Notes", None),
            InputCapabilityProfile::LegacyCapturedTarget
        );
        for bundle_id in [
            "com.todesktop.230313mzl4w4u92",
            "com.apple.Terminal",
            "com.googlecode.iterm2",
            "dev.warp.Warp-Stable",
        ] {
            assert_eq!(
                input_capability_profile(bundle_id, None),
                InputCapabilityProfile::LegacyCapturedTarget
            );
        }
    }

    #[test]
    fn claude_uses_its_calibrated_composer_point_without_changing_codex() {
        let InputCapabilityProfile::Accessibility(claude) =
            input_capability_profile("com.anthropic.claudefordesktop", None)
        else {
            panic!("Claude should use a calibrated accessibility profile");
        };

        assert_eq!(
            claude.focus_acquisition,
            FocusAcquisitionPolicy::CalibratedWindowPoint {
                horizontal_percent: 50,
                bottom_offset: 80,
            }
        );
        assert!(claude.permits_submit());
        assert_eq!(
            input_capability_profile("com.openai.codex", None),
            InputCapabilityProfile::CodexFirstResponder
        );
    }

    #[test]
    fn supported_browsers_use_a_calibrated_composer_point() {
        for bundle_id in [
            "com.apple.Safari",
            "com.google.Chrome",
            "com.microsoft.edgemac",
            "com.brave.Browser",
            "company.thebrowser.Browser",
            "org.mozilla.firefox",
        ] {
            let InputCapabilityProfile::Accessibility(browser) =
                input_capability_profile(bundle_id, None)
            else {
                panic!("{bundle_id} should use a calibrated browser profile");
            };
            assert_eq!(
                browser.focus_acquisition,
                FocusAcquisitionPolicy::CalibratedWindowPoint {
                    horizontal_percent: 50,
                    bottom_offset: 80,
                }
            );
            assert!(browser.permits_submit());
        }
    }

    #[test]
    fn browser_ai_sites_use_their_screenshot_calibrated_offsets() {
        for (url, expected_offset) in [
            ("https://chatgpt.com/c/123", 64),
            ("https://gemini.google.com/app/123", 88),
            ("https://manus.im/app/123", 72),
        ] {
            let InputCapabilityProfile::Accessibility(profile) =
                input_capability_profile_for_page("com.google.Chrome", None, Some(url))
            else {
                panic!("{url} should use a calibrated browser profile");
            };
            assert_eq!(
                profile.focus_acquisition,
                FocusAcquisitionPolicy::CalibratedWindowPoint {
                    horizontal_percent: 50,
                    bottom_offset: expected_offset,
                }
            );
        }
    }

    #[test]
    fn browser_site_matching_accepts_subdomains_and_rejects_lookalikes() {
        assert_eq!(
            calibrated_bottom_offset_for_url("https://www.chatgpt.com/"),
            Some(64)
        );
        assert_eq!(
            calibrated_bottom_offset_for_url("https://chatgpt.com.evil.example/"),
            None
        );
        assert_eq!(calibrated_bottom_offset_for_url("not a URL"), None);
    }

    #[test]
    fn claude_and_wechat_require_exact_windows_with_proven_process_scopes() {
        let InputCapabilityProfile::Accessibility(claude) =
            input_capability_profile("com.anthropic.claudefordesktop", Some("1.18286.0"))
        else {
            panic!("Claude should use accessibility");
        };
        assert_eq!(claude.process_scope, ProcessScope::MainOnly);
        assert_eq!(claude.window_identity, WindowIdentityRequirement::Required);
        assert_eq!(
            claude.manual_accessibility,
            ManualAccessibilityPolicy::Never
        );

        let InputCapabilityProfile::Accessibility(wechat) =
            input_capability_profile("com.tencent.xinWeChat", Some("4.1.2"))
        else {
            panic!("WeChat should use accessibility");
        };
        assert_eq!(
            wechat.process_scope,
            ProcessScope::MainAndValidatedBrowserApplications
        );
        assert_eq!(wechat.window_identity, WindowIdentityRequirement::Required);
        assert_eq!(
            wechat.focus_acquisition,
            FocusAcquisitionPolicy::CalibratedWindowPoint {
                horizontal_percent: 50,
                bottom_offset: 65,
            }
        );
    }

    #[test]
    fn accessibility_profiles_keep_calibrated_points_out_of_the_ax_web_area() {
        for (bundle_id, version) in [
            ("com.anthropic.claudefordesktop", "1.18286.0"),
            ("com.tencent.xinWeChat", "4.1.2"),
        ] {
            let InputCapabilityProfile::Accessibility(profile) =
                input_capability_profile(bundle_id, Some(version))
            else {
                panic!("expected accessibility profile");
            };
            assert!(profile.forbidden_subroles.contains(&"AXSearchField"));
            assert!(!profile.allowed_roles.contains(&"AXWebArea"));
            assert!(profile.permits_calibrated_window_point);
            assert!(!profile.permits_web_area_composer);
            if bundle_id == "com.tencent.xinWeChat" {
                assert!(!profile.permits_submit());
            } else {
                assert!(profile.permits_submit());
            }
        }
    }

    #[test]
    fn unknown_wechat_versions_cannot_enter_submit_phase() {
        let InputCapabilityProfile::Accessibility(profile) =
            input_capability_profile("com.tencent.xinWeChat", Some("99.0"))
        else {
            panic!("expected accessibility profile");
        };
        assert!(!profile.permits_submit());
        assert!(!profile.permits_calibrated_window_point);
    }

    #[test]
    fn uncalibrated_wechat_patch_versions_cannot_inherit_submit_evidence() {
        let InputCapabilityProfile::Accessibility(profile) =
            input_capability_profile("com.tencent.xinWeChat", Some("4.1.3"))
        else {
            panic!("expected accessibility profile");
        };
        assert!(!profile.permits_submit());
    }
}
