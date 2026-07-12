use super::CandidateInput;

const ABSOLUTE_THRESHOLD: i32 = 75;
const AMBIGUITY_MARGIN: i32 = 18;

#[derive(Clone, Debug, PartialEq)]
pub(super) struct ComposerCandidate {
    pub owner_pid: u32,
    pub role: String,
    pub subrole: Option<String>,
    pub identifier: Option<String>,
    pub title: Option<String>,
    pub description: Option<String>,
    pub placeholder: Option<String>,
    pub help: Option<String>,
    pub frame: CandidateInput,
    pub enabled: bool,
    pub visible: bool,
    pub focused: bool,
    pub window_matches: bool,
    pub editable: bool,
    pub secure: bool,
    pub depth: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ComposerResolutionError {
    NotFound,
    Ambiguous,
}

pub(super) fn resolve_composer(
    candidates: &[ComposerCandidate],
    trusted_pids: &[u32],
    captured_window: &CandidateInput,
) -> Result<usize, ComposerResolutionError> {
    let mut ranked = candidates
        .iter()
        .enumerate()
        .filter(|(_, candidate)| candidate_allowed(candidate, trusted_pids, captured_window))
        .map(|(index, candidate)| (index, candidate_score(candidate, captured_window)))
        .filter(|(_, score)| *score >= ABSOLUTE_THRESHOLD)
        .collect::<Vec<_>>();
    ranked.sort_unstable_by(|left, right| right.1.cmp(&left.1));
    let Some(&(best_index, best_score)) = ranked.first() else {
        return Err(ComposerResolutionError::NotFound);
    };
    if ranked
        .get(1)
        .is_some_and(|(_, second_score)| best_score - *second_score < AMBIGUITY_MARGIN)
    {
        return Err(ComposerResolutionError::Ambiguous);
    }
    Ok(best_index)
}

fn candidate_allowed(
    candidate: &ComposerCandidate,
    trusted_pids: &[u32],
    captured_window: &CandidateInput,
) -> bool {
    trusted_pids.contains(&candidate.owner_pid)
        && candidate.window_matches
        && candidate.enabled
        && candidate.visible
        && candidate.editable
        && !candidate.secure
        && candidate.frame.width > 1.0
        && candidate.frame.height > 1.0
        && frame_inside(&candidate.frame, captured_window)
        && candidate.role != "AXWebArea"
        && candidate.subrole.as_deref() != Some("AXSearchField")
        && candidate.subrole.as_deref() != Some("AXSecureTextField")
        && !candidate_has_search_semantics(candidate)
}

fn candidate_has_search_semantics(candidate: &ComposerCandidate) -> bool {
    [
        candidate.identifier.as_deref(),
        candidate.title.as_deref(),
        candidate.description.as_deref(),
        candidate.placeholder.as_deref(),
        candidate.help.as_deref(),
    ]
    .into_iter()
    .flatten()
    .map(|value| value.to_lowercase())
    .any(|value| {
        ["search", "find", "filter", "搜索", "查找", "联系人"]
            .iter()
            .any(|excluded| value.contains(excluded))
    })
}

fn candidate_score(candidate: &ComposerCandidate, window: &CandidateInput) -> i32 {
    let role_score = match candidate.role.as_str() {
        "AXTextArea" => 60,
        "AXTextField" => 48,
        "AXComboBox" => 35,
        _ => 0,
    };
    let lower_ratio =
        ((candidate.frame.y + candidate.frame.height - window.y) / window.height).clamp(0.0, 1.0);
    let lower_score = (lower_ratio * 35.0) as i32;
    let area_ratio = ((candidate.frame.width * candidate.frame.height)
        / (window.width * window.height).max(1.0))
    .clamp(0.0, 0.3);
    let area_score = (area_ratio * 100.0) as i32;
    role_score + lower_score + area_score + if candidate.focused { 40 } else { 0 }
        - candidate.depth.min(12) as i32
}

fn frame_inside(frame: &CandidateInput, window: &CandidateInput) -> bool {
    const TOLERANCE: f64 = 2.0;
    frame.x >= window.x - TOLERANCE
        && frame.y >= window.y - TOLERANCE
        && frame.x + frame.width <= window.x + window.width + TOLERANCE
        && frame.y + frame.height <= window.y + window.height + TOLERANCE
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;

    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct ComposerFixture {
        window: CandidateInput,
        trusted_pids: Vec<u32>,
        expected_index: Option<usize>,
        expected_error: Option<String>,
        candidates: Vec<FixtureCandidate>,
    }

    #[derive(Deserialize)]
    struct FixtureCandidate {
        pid: u32,
        role: String,
        y: f64,
        excluded: bool,
    }

    fn window() -> CandidateInput {
        CandidateInput {
            x: 0.0,
            y: 0.0,
            width: 1_000.0,
            height: 800.0,
        }
    }

    fn candidate(pid: u32, role: &str, y: f64) -> ComposerCandidate {
        ComposerCandidate {
            owner_pid: pid,
            role: role.to_string(),
            subrole: None,
            identifier: None,
            title: None,
            description: None,
            placeholder: None,
            help: None,
            frame: CandidateInput {
                x: 180.0,
                y,
                width: 640.0,
                height: 100.0,
            },
            enabled: true,
            visible: true,
            focused: false,
            window_matches: true,
            editable: true,
            secure: false,
            depth: 4,
        }
    }

    #[test]
    fn composer_resolver_prefers_bottom_composer_over_search_fields() {
        let mut search = candidate(10, "AXTextField", 20.0);
        search.subrole = Some("AXSearchField".to_string());
        search.placeholder = Some("Search conversations".to_string());
        let composer = candidate(10, "AXTextArea", 660.0);

        assert_eq!(
            resolve_composer(&[search, composer], &[10], &window()),
            Ok(1)
        );
    }

    #[test]
    fn composer_resolver_rejects_web_area_and_invalid_fields() {
        for mutate in 0..7 {
            let mut invalid = candidate(10, "AXTextArea", 660.0);
            match mutate {
                0 => invalid.role = "AXWebArea".to_string(),
                1 => invalid.enabled = false,
                2 => invalid.visible = false,
                3 => invalid.secure = true,
                4 => invalid.owner_pid = 99,
                5 => invalid.window_matches = false,
                _ => invalid.frame.width = 0.0,
            }
            assert_eq!(
                resolve_composer(&[invalid], &[10], &window()),
                Err(ComposerResolutionError::NotFound)
            );
        }
    }

    #[test]
    fn composer_resolver_rejects_wrong_window_and_semantic_search_fields() {
        let mut wrong_window = candidate(10, "AXTextArea", 900.0);
        wrong_window.placeholder = Some("Message".to_string());
        let mut contact_search = candidate(10, "AXTextArea", 660.0);
        contact_search.description = Some("搜索联系人".to_string());

        assert_eq!(
            resolve_composer(&[wrong_window, contact_search], &[10], &window()),
            Err(ComposerResolutionError::NotFound)
        );
    }

    #[test]
    fn composer_resolver_rejects_ambiguous_text_areas() {
        let first = candidate(10, "AXTextArea", 650.0);
        let mut second = candidate(10, "AXTextArea", 652.0);
        second.frame.x += 5.0;

        assert_eq!(
            resolve_composer(&[first, second], &[10], &window()),
            Err(ComposerResolutionError::Ambiguous)
        );
    }

    #[test]
    fn editable_descendant_is_selected_instead_of_web_area_container() {
        let web_area = candidate(10, "AXWebArea", 0.0);
        let mut composer = candidate(10, "AXTextArea", 660.0);
        composer.focused = true;

        assert_eq!(
            resolve_composer(&[web_area, composer], &[10], &window()),
            Ok(1)
        );
    }

    #[test]
    fn composer_fixtures_select_only_trusted_unambiguous_inputs() {
        let fixtures = [
            include_str!("../../../tests/fixtures/ax/appkit-composer.json"),
            include_str!("../../../tests/fixtures/ax/electron-contenteditable.json"),
            include_str!("../../../tests/fixtures/ax/claude-composer.json"),
            include_str!("../../../tests/fixtures/ax/wechat-composer.json"),
            include_str!("../../../tests/fixtures/ax/ambiguous-inputs.json"),
        ];
        for raw in fixtures {
            assert!(!raw.contains("AXValue"));
            assert!(!raw.contains("clipboard"));
            let fixture: ComposerFixture = serde_json::from_str(raw).unwrap();
            let candidates = fixture
                .candidates
                .iter()
                .map(|item| {
                    let mut candidate = candidate(item.pid, &item.role, item.y);
                    if item.excluded {
                        candidate.subrole = Some("AXSearchField".to_string());
                    }
                    candidate
                })
                .collect::<Vec<_>>();
            let result = resolve_composer(&candidates, &fixture.trusted_pids, &fixture.window);
            match fixture.expected_error.as_deref() {
                Some("ambiguous") => assert_eq!(result, Err(ComposerResolutionError::Ambiguous)),
                None => assert_eq!(result, Ok(fixture.expected_index.unwrap())),
                other => panic!("unexpected fixture error: {other:?}"),
            }
        }
    }
}
