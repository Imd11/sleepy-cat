use super::CandidateInput;

#[derive(Clone, Debug, PartialEq)]
pub(super) struct ComposerFingerprint {
    pub owner_pid: u32,
    pub role: String,
    pub subrole: Option<String>,
    pub identifier_hash: Option<String>,
    pub frame: CandidateInput,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum FocusFrontmost {
    Target,
    PromptDrawer,
    Other,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum FocusError {
    UserSwitchedApplication,
    ActivationFailed,
    RaiseFailed,
    FocusRejected,
    CandidateChanged,
    FocusUnstable,
    ClickNotVerified,
    ExistingTextCaretUnknown,
}

pub(super) struct FocusOptions {
    pub max_polls: usize,
    pub stable_reads: usize,
    pub click_fallback_permitted: bool,
    pub candidate_contains_text: bool,
    pub caret_policy_proven: bool,
}

pub(super) fn focus_and_verify<F, A, R, S, V, H, C, W>(
    options: FocusOptions,
    mut frontmost: F,
    mut activate: A,
    mut raise_window: R,
    mut set_focused: S,
    mut exact_focus: V,
    mut hit_test_matches: H,
    mut click_candidate: C,
    mut wait: W,
) -> Result<(), FocusError>
where
    F: FnMut() -> FocusFrontmost,
    A: FnMut() -> bool,
    R: FnMut() -> bool,
    S: FnMut() -> bool,
    V: FnMut() -> bool,
    H: FnMut() -> bool,
    C: FnMut() -> bool,
    W: FnMut(),
{
    match frontmost() {
        FocusFrontmost::Target => {}
        FocusFrontmost::PromptDrawer => {
            if !activate() {
                return Err(FocusError::ActivationFailed);
            }
            if !raise_window() {
                return Err(FocusError::RaiseFailed);
            }
        }
        FocusFrontmost::Other => return Err(FocusError::UserSwitchedApplication),
    }

    if stable_exact_focus(options.stable_reads, &mut exact_focus, &mut wait) {
        return Ok(());
    }

    if set_focused()
        && poll_for_stable_focus(
            options.max_polls,
            options.stable_reads,
            &mut exact_focus,
            &mut wait,
        )
    {
        return Ok(());
    }

    if !options.click_fallback_permitted {
        return Err(FocusError::FocusRejected);
    }
    if options.candidate_contains_text && !options.caret_policy_proven {
        return Err(FocusError::ExistingTextCaretUnknown);
    }
    if !hit_test_matches() || !click_candidate() {
        return Err(FocusError::ClickNotVerified);
    }
    if poll_for_stable_focus(
        options.max_polls,
        options.stable_reads,
        &mut exact_focus,
        &mut wait,
    ) {
        Ok(())
    } else {
        Err(FocusError::FocusUnstable)
    }
}

fn poll_for_stable_focus<V, W>(
    max_polls: usize,
    stable_reads: usize,
    exact_focus: &mut V,
    wait: &mut W,
) -> bool
where
    V: FnMut() -> bool,
    W: FnMut(),
{
    let mut consecutive = 0;
    for _ in 0..max_polls {
        if exact_focus() {
            consecutive += 1;
            if consecutive >= stable_reads.max(1) {
                return true;
            }
        } else {
            consecutive = 0;
        }
        wait();
    }
    false
}

fn stable_exact_focus<V, W>(stable_reads: usize, exact_focus: &mut V, wait: &mut W) -> bool
where
    V: FnMut() -> bool,
    W: FnMut(),
{
    if !exact_focus() {
        return false;
    }
    wait();
    stable_reads <= 1 || exact_focus()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;
    use std::collections::VecDeque;

    fn options() -> FocusOptions {
        FocusOptions {
            max_polls: 6,
            stable_reads: 2,
            click_fallback_permitted: false,
            candidate_contains_text: false,
            caret_policy_proven: false,
        }
    }

    #[test]
    fn focus_controller_preserves_already_focused_composer() {
        let set_calls = Cell::new(0);
        let result = focus_and_verify(
            options(),
            || FocusFrontmost::Target,
            || true,
            || true,
            || {
                set_calls.set(set_calls.get() + 1);
                true
            },
            || true,
            || false,
            || false,
            || {},
        );

        assert_eq!(result, Ok(()));
        assert_eq!(set_calls.get(), 0);
    }

    #[test]
    fn focus_controller_requires_exact_stable_readback() {
        let mut reads = VecDeque::from([false, false, true, true]);
        let result = focus_and_verify(
            options(),
            || FocusFrontmost::Target,
            || true,
            || true,
            || true,
            || reads.pop_front().unwrap_or(false),
            || false,
            || false,
            || {},
        );
        assert_eq!(result, Ok(()));
    }

    #[test]
    fn focus_controller_rejects_focus_that_moves_to_another_field() {
        let result = focus_and_verify(
            options(),
            || FocusFrontmost::Target,
            || true,
            || true,
            || true,
            || false,
            || false,
            || false,
            || {},
        );
        assert_eq!(result, Err(FocusError::FocusRejected));
    }

    #[test]
    fn focus_controller_aborts_when_user_switches_to_third_app() {
        let result = focus_and_verify(
            options(),
            || FocusFrontmost::Other,
            || panic!("must not activate"),
            || panic!("must not raise"),
            || panic!("must not focus"),
            || false,
            || false,
            || false,
            || {},
        );
        assert_eq!(result, Err(FocusError::UserSwitchedApplication));
    }

    #[test]
    fn click_fallback_requires_hit_test_and_known_caret_policy() {
        let mut click_options = options();
        click_options.click_fallback_permitted = true;
        click_options.candidate_contains_text = true;
        assert_eq!(
            focus_and_verify(
                click_options,
                || FocusFrontmost::Target,
                || true,
                || true,
                || false,
                || false,
                || true,
                || true,
                || {},
            ),
            Err(FocusError::ExistingTextCaretUnknown)
        );

        let mut click_options = options();
        click_options.click_fallback_permitted = true;
        assert_eq!(
            focus_and_verify(
                click_options,
                || FocusFrontmost::Target,
                || true,
                || true,
                || false,
                || false,
                || false,
                || true,
                || {},
            ),
            Err(FocusError::ClickNotVerified)
        );
    }

    #[test]
    fn prompt_drawer_frontmost_activates_and_raises_exact_window() {
        let activated = Cell::new(false);
        let raised = Cell::new(false);
        let result = focus_and_verify(
            options(),
            || FocusFrontmost::PromptDrawer,
            || {
                activated.set(true);
                true
            },
            || {
                raised.set(true);
                true
            },
            || true,
            || true,
            || false,
            || false,
            || {},
        );
        assert_eq!(result, Ok(()));
        assert!(activated.get());
        assert!(raised.get());
    }
}
