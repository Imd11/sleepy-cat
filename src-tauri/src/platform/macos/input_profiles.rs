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
    pub permits_coordinate_guess: bool,
    pub permits_web_area_composer: bool,
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
        "com.anthropic.claudefordesktop" => {
            InputCapabilityProfile::Accessibility(accessibility_profile(
                ProcessScope::MainOnly,
                ManualAccessibilityPolicy::OnlyWhenTreeSparse,
                (observed_version == Some("1.18286.0"))
                    .then_some(PasteVerificationPolicy::ValueLengthOrHashChange)
                    .unwrap_or(PasteVerificationPolicy::PasteOnlyWithoutSubmitEvidence),
            ))
        }
        "com.tencent.xinWeChat" => InputCapabilityProfile::Accessibility(accessibility_profile(
            ProcessScope::MainAndValidatedBrowserApplications,
            ManualAccessibilityPolicy::Never,
            (observed_version == Some("4.1.2"))
                .then_some(PasteVerificationPolicy::SelectionRangeChange)
                .unwrap_or(PasteVerificationPolicy::PasteOnlyWithoutSubmitEvidence),
        )),
        _ => InputCapabilityProfile::LegacyCapturedTarget,
    }
}

fn accessibility_profile(
    process_scope: ProcessScope,
    manual_accessibility: ManualAccessibilityPolicy,
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
        permits_coordinate_guess: false,
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
            ManualAccessibilityPolicy::OnlyWhenTreeSparse
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
    }

    #[test]
    fn accessibility_profiles_never_guess_or_select_web_areas_and_forbid_search() {
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
            assert!(!profile.permits_coordinate_guess);
            assert!(!profile.permits_web_area_composer);
            assert!(profile.permits_submit());
        }
    }

    #[test]
    fn unknown_target_versions_cannot_enter_submit_phase() {
        for bundle_id in ["com.anthropic.claudefordesktop", "com.tencent.xinWeChat"] {
            let InputCapabilityProfile::Accessibility(profile) =
                input_capability_profile(bundle_id, Some("99.0"))
            else {
                panic!("expected accessibility profile");
            };
            assert!(!profile.permits_submit());
        }
    }

    #[test]
    fn uncalibrated_patch_versions_cannot_inherit_submit_evidence() {
        for (bundle_id, version) in [
            ("com.anthropic.claudefordesktop", "1.18286.1"),
            ("com.tencent.xinWeChat", "4.1.3"),
        ] {
            let InputCapabilityProfile::Accessibility(profile) =
                input_capability_profile(bundle_id, Some(version))
            else {
                panic!("expected accessibility profile");
            };
            assert!(!profile.permits_submit());
        }
    }
}
