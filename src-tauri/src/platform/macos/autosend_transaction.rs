use super::NativeSubmitKey;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum AutosendPhase {
    ValidateTarget,
    ResolveComposer,
    FocusComposer,
    VerifyFocus,
    Paste,
    VerifyAfterPaste,
    Submit,
    Complete,
    Aborted,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum TransactionFailure {
    TargetChanged,
    ComposerUnavailable,
    FocusNotAcquired,
    ClipboardWriteFailed,
    PasteEventFailed,
    PasteNotConfirmed,
    SubmitEventFailed,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct TransactionResult {
    pub phases: Vec<AutosendPhase>,
    pub clipboard_written: bool,
    pub paste_posted: bool,
    pub submit_posted: bool,
    pub failure: Option<TransactionFailure>,
}

#[allow(clippy::too_many_arguments)]
pub(super) fn run_transaction<V, R, F, C, K, P, E, S>(
    submit_key: NativeSubmitKey,
    mut validate_target: V,
    mut resolve_composer: R,
    mut focus_composer: F,
    mut write_clipboard: C,
    mut revalidate_focus: K,
    mut post_paste: P,
    mut verify_paste: E,
    mut post_submit: S,
) -> TransactionResult
where
    V: FnMut() -> bool,
    R: FnMut() -> bool,
    F: FnMut() -> bool,
    C: FnMut() -> bool,
    K: FnMut() -> bool,
    P: FnMut() -> bool,
    E: FnMut() -> bool,
    S: FnMut(NativeSubmitKey) -> bool,
{
    let mut result = TransactionResult {
        phases: Vec::new(),
        clipboard_written: false,
        paste_posted: false,
        submit_posted: false,
        failure: None,
    };
    macro_rules! require {
        ($phase:expr, $operation:expr, $failure:expr) => {{
            result.phases.push($phase);
            if !$operation {
                result.phases.push(AutosendPhase::Aborted);
                result.failure = Some($failure);
                return result;
            }
        }};
    }

    require!(
        AutosendPhase::ValidateTarget,
        validate_target(),
        TransactionFailure::TargetChanged
    );
    require!(
        AutosendPhase::ResolveComposer,
        resolve_composer(),
        TransactionFailure::ComposerUnavailable
    );
    require!(
        AutosendPhase::FocusComposer,
        focus_composer(),
        TransactionFailure::FocusNotAcquired
    );
    require!(
        AutosendPhase::VerifyFocus,
        revalidate_focus(),
        TransactionFailure::FocusNotAcquired
    );
    if !write_clipboard() {
        result.phases.push(AutosendPhase::Aborted);
        result.failure = Some(TransactionFailure::ClipboardWriteFailed);
        return result;
    }
    result.clipboard_written = true;
    require!(
        AutosendPhase::VerifyFocus,
        validate_target() && revalidate_focus(),
        TransactionFailure::TargetChanged
    );
    result.phases.push(AutosendPhase::Paste);
    if !post_paste() {
        result.phases.push(AutosendPhase::Aborted);
        result.failure = Some(TransactionFailure::PasteEventFailed);
        return result;
    }
    result.paste_posted = true;

    if submit_key != NativeSubmitKey::None {
        require!(
            AutosendPhase::VerifyAfterPaste,
            verify_paste(),
            TransactionFailure::PasteNotConfirmed
        );
        require!(
            AutosendPhase::VerifyFocus,
            validate_target() && revalidate_focus(),
            TransactionFailure::TargetChanged
        );
        result.phases.push(AutosendPhase::Submit);
        if !post_submit(submit_key) {
            result.phases.push(AutosendPhase::Aborted);
            result.failure = Some(TransactionFailure::SubmitEventFailed);
            return result;
        }
        result.submit_posted = true;
    }

    result.phases.push(AutosendPhase::Complete);
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    #[test]
    fn autosend_transaction_orders_focus_before_clipboard_and_paste() {
        let events = std::cell::RefCell::new(Vec::new());
        let result = run_transaction(
            NativeSubmitKey::Enter,
            || {
                events.borrow_mut().push("validate");
                true
            },
            || {
                events.borrow_mut().push("resolve");
                true
            },
            || {
                events.borrow_mut().push("focus");
                true
            },
            || {
                events.borrow_mut().push("copy");
                true
            },
            || {
                events.borrow_mut().push("verify-focus");
                true
            },
            || {
                events.borrow_mut().push("paste");
                true
            },
            || {
                events.borrow_mut().push("verify-paste");
                true
            },
            |_| {
                events.borrow_mut().push("submit");
                true
            },
        );
        assert_eq!(result.failure, None);
        let events = events.borrow();
        assert!(
            events.iter().position(|event| *event == "focus").unwrap()
                < events.iter().position(|event| *event == "copy").unwrap()
        );
        assert!(
            events.iter().position(|event| *event == "copy").unwrap()
                < events.iter().position(|event| *event == "paste").unwrap()
        );
    }

    #[test]
    fn pre_paste_failure_changes_neither_clipboard_nor_keyboard() {
        let copy = Cell::new(0);
        let paste = Cell::new(0);
        let submit = Cell::new(0);
        let result = run_transaction(
            NativeSubmitKey::Enter,
            || false,
            || true,
            || true,
            || {
                copy.set(copy.get() + 1);
                true
            },
            || true,
            || {
                paste.set(paste.get() + 1);
                true
            },
            || true,
            |_| {
                submit.set(submit.get() + 1);
                true
            },
        );
        assert_eq!(result.failure, Some(TransactionFailure::TargetChanged));
        assert_eq!((copy.get(), paste.get(), submit.get()), (0, 0, 0));
    }

    #[test]
    fn paste_unknown_is_not_retried_and_never_submits() {
        let paste = Cell::new(0);
        let submit = Cell::new(0);
        let result = run_transaction(
            NativeSubmitKey::Enter,
            || true,
            || true,
            || true,
            || true,
            || true,
            || {
                paste.set(paste.get() + 1);
                true
            },
            || false,
            |_| {
                submit.set(submit.get() + 1);
                true
            },
        );
        assert_eq!(result.failure, Some(TransactionFailure::PasteNotConfirmed));
        assert_eq!(paste.get(), 1);
        assert_eq!(submit.get(), 0);
    }

    #[test]
    fn paste_only_structurally_skips_verification_and_submit() {
        let verify_paste = Cell::new(0);
        let submit = Cell::new(0);
        let result = run_transaction(
            NativeSubmitKey::None,
            || true,
            || true,
            || true,
            || true,
            || true,
            || true,
            || {
                verify_paste.set(verify_paste.get() + 1);
                false
            },
            |_| {
                submit.set(submit.get() + 1);
                true
            },
        );
        assert_eq!(result.failure, None);
        assert!(result.paste_posted);
        assert!(!result.submit_posted);
        assert_eq!((verify_paste.get(), submit.get()), (0, 0));
        assert!(!result.phases.contains(&AutosendPhase::Submit));
    }

    #[test]
    fn frontmost_or_session_change_cancels_before_submit() {
        let validation_count = Cell::new(0);
        let submit = Cell::new(0);
        let result = run_transaction(
            NativeSubmitKey::Enter,
            || {
                validation_count.set(validation_count.get() + 1);
                validation_count.get() < 3
            },
            || true,
            || true,
            || true,
            || true,
            || true,
            || true,
            |_| {
                submit.set(submit.get() + 1);
                true
            },
        );

        assert_eq!(result.failure, Some(TransactionFailure::TargetChanged));
        assert_eq!(submit.get(), 0);
        assert!(result.paste_posted);
    }

    #[test]
    fn repeated_fixture_transactions_never_duplicate_paste_or_submit() {
        for submit_key in [NativeSubmitKey::None, NativeSubmitKey::Enter] {
            for _ in 0..100 {
                let paste = Cell::new(0);
                let submit = Cell::new(0);
                let result = run_transaction(
                    submit_key,
                    || true,
                    || true,
                    || true,
                    || true,
                    || true,
                    || {
                        paste.set(paste.get() + 1);
                        true
                    },
                    || true,
                    |_| {
                        submit.set(submit.get() + 1);
                        true
                    },
                );
                assert_eq!(result.failure, None);
                assert_eq!(paste.get(), 1);
                assert_eq!(
                    submit.get(),
                    if submit_key == NativeSubmitKey::None {
                        0
                    } else {
                        1
                    }
                );
            }
        }
    }
}
