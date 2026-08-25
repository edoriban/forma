//! `<Form>` submit wrapper and outcome mapping.

use formars_signals::{FormController, FormSnapshot, FormaError, SubmitError};
use leptos::callback::Callback;
use leptos::prelude::*;
use leptos::task::spawn_local;
use std::future::Future;

/// Mirrors [`SubmitError`] arms plus success, delivered to the caller's
/// `on_outcome` callback.
#[derive(Clone, Debug)]
pub enum SubmitOutcome<T, E> {
    /// The handler resolved `Ok(t)`.
    Success(T),
    /// The handler ran and failed with its own error. The caller MAY map
    /// payloads back with [`FormController::apply_server_errors`].
    HandlerError(E),
    /// Whole-form sync validation rejected the form BEFORE the handler was
    /// constructed. Per-field `visible_errors` already reflect violations;
    /// `<Form>` performs no additional error injection.
    ValidationFailed(FormaError),
}

/// Routes a composed submit result into the caller-facing outcome enum.
/// Pure: no reactivity, no allocation beyond the payload move.
pub(crate) fn map_outcome<T, E>(result: Result<T, SubmitError<E>>) -> SubmitOutcome<T, E> {
    match result {
        Ok(t) => SubmitOutcome::Success(t),
        Err(SubmitError::Validation(e)) => SubmitOutcome::ValidationFailed(e),
        Err(SubmitError::Handler(e)) => SubmitOutcome::HandlerError(e),
    }
}

/// Begins one submit attempt: the in-flight guard (defensive double-submit
/// shield complementing the reactively disabled button) followed by the
/// synchronous `submitted` flip that activates visibility gates BEFORE any
/// async work (FU-FM-2). Returns `false` when an attempt is already running.
///
/// `is_submitting` flips true SYNCHRONOUSLY here — not on the composed
/// future's first poll — so two same-tick submit events cannot both pass the
/// guard. The controller's drop guard (engaged when the future first polls)
/// resets the flag exactly once, on every exit path including cancellation.
pub(crate) fn begin_attempt(controller: &FormController) -> bool {
    if controller.is_submitting().get() {
        return false;
    }
    controller.is_submitting().set(true);
    controller.submitted().set(true);
    true
}

/// Drives one attempt's composed future to its caller-facing outcome:
/// sync validation gate (handler never built on failure), snapshot,
/// `is_submitting` bracketing across all paths, then [`map_outcome`].
pub(crate) async fn finish_attempt<T, E, F, Fut>(
    controller: &FormController,
    handler: F,
) -> SubmitOutcome<T, E>
where
    T: 'static,
    E: 'static,
    F: FnOnce(FormSnapshot) -> Fut,
    Fut: Future<Output = Result<T, E>>,
{
    let result = controller.on_submit(handler).await;
    map_outcome(result)
}

/// Encapsulates async form submission over a [`FormController`].
///
/// Per attempt: `prevent_default()` first, then the in-flight guard, the
/// synchronous `submitted` flip, and the deliberately non-`Send` composed
/// future driven via `spawn_local` (client-local task; NEVER tokio::spawn).
///
/// # SSR exclusion (v0)
///
/// Markup may render server-side, but submit execution is client-path only:
/// `spawn_local` requires a local-task executor unavailable on server
/// threads, and the composed future is intentionally not `Send`, so it can
/// never be tokio-spawned.
#[component]
// reason: typed-builder consumes props by value; component fns are never called directly
#[allow(
    clippy::needless_pass_by_value,
    reason = "leptos typed-builder prop convention"
)]
pub fn Form<T, E, F, Fut>(
    controller: FormController,
    #[prop(into)] on_outcome: Callback<SubmitOutcome<T, E>>,
    on_submit: F,
    #[prop(optional)] submit_label: Option<String>,
    children: Children,
) -> impl IntoView
where
    T: 'static,
    E: 'static,
    F: Fn(FormSnapshot) -> Fut + Clone + 'static,
    Fut: Future<Output = Result<T, E>> + 'static,
{
    let ctrl_for_button = controller.clone();
    view! {
        <form on:submit=move |ev| {
            ev.prevent_default();
            if !begin_attempt(&controller) {
                return;
            }
            // each attempt passes a fresh clone of the persistent `Fn`
            // into a fresh composed future
                let handler = on_submit.clone();
                let outcome_cb = on_outcome;
                let ctrl = controller.clone();
            spawn_local(async move {
                let outcome = finish_attempt(&ctrl, handler).await;
                outcome_cb.run(outcome);
            });
        }>
            {children()}
            {move || {
                submit_label.clone().map(|label| {
                    let is_submitting = ctrl_for_button.is_submitting();
                    view! {
                        <button type="submit" disabled=move || is_submitting.get()>
                            {label}
                        </button>
                    }
                })
            }}
        </form>
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use formars_signals::{FieldPath, IssueCode};
    use std::borrow::Cow;

    fn sample_error() -> FormaError {
        FormaError {
            issues: vec![formars_signals::FormaIssue {
                path: FieldPath::key("email"),
                code: IssueCode::Min,
                message: Cow::Borrowed("too short"),
                params: Vec::new(),
            }],
        }
    }

    #[test]
    fn ok_maps_to_success_preserving_payload() {
        let mapped: SubmitOutcome<u32, String> = map_outcome(Ok(7u32));
        assert!(
            matches!(mapped, SubmitOutcome::Success(7)),
            "Ok must map to Success carrying the payload"
        );
    }

    #[test]
    fn validation_error_maps_to_validation_failed_with_payload() {
        let mapped: SubmitOutcome<(), FormaError> =
            map_outcome(Err(SubmitError::Validation(sample_error())));
        match mapped {
            SubmitOutcome::ValidationFailed(e) => {
                assert_eq!(e.issues.len(), 1, "validation payload must be preserved");
                assert_eq!(e.issues[0].path, FieldPath::key("email"));
            }
            other => panic!("expected ValidationFailed, got {other:?}"),
        }
    }

    #[test]
    fn handler_error_maps_to_handler_error_preserving_e() {
        let mapped: SubmitOutcome<(), FormaError> =
            map_outcome(Err(SubmitError::Handler(sample_error())));
        match mapped {
            SubmitOutcome::HandlerError(e) => {
                assert_eq!(e.issues.len(), 1, "handler error payload E preserved");
            }
            other => panic!("expected HandlerError, got {other:?}"),
        }
    }
}

/// FU-FM-2/3/5 glue-level tests over the composed future, driven by
/// `block_on` — mirroring exactly what `<Form>`'s submit handler does
/// (native-only; `spawn_local` itself is exercised on the wasm target).
#[cfg(test)]
mod glue {
    use super::*;
    use formars_core::prelude::*;
    use formars_signals::ValidateOn;
    use futures::executor::block_on;
    use reactive_graph::traits::Get;
    use std::cell::RefCell;
    use std::rc::Rc;

    fn controller_with_invalid_email() -> FormController {
        let mut c = FormController::new(ValidateOn::Submit);
        c.register(FieldPath::key("email"), Box::new(string().min(8).email()))
            .unwrap();
        c.field(&FieldPath::key("email")).unwrap().set_str("ab");
        c
    }

    fn controller_with_valid_email() -> FormController {
        let mut c = FormController::new(ValidateOn::Submit);
        c.register(FieldPath::key("email"), Box::new(string().min(8).email()))
            .unwrap();
        c.field(&FieldPath::key("email"))
            .unwrap()
            .set_str("user@example.com");
        c
    }

    /// The exact two-step sequence `<Form>` performs per attempt (extracted
    /// so native tests drive the real code path).
    fn form_attempt<T, E, F, Fut>(
        controller: &FormController,
        handler: F,
    ) -> impl Future<Output = SubmitOutcome<T, E>>
    where
        T: 'static,
        E: 'static,
        F: FnOnce(FormSnapshot) -> Fut,
        Fut: Future<Output = Result<T, E>>,
    {
        let proceed = begin_attempt(controller);
        let ctrl = controller.clone();
        async move {
            if proceed {
                finish_attempt(&ctrl, handler).await
            } else {
                unreachable!("fresh controllers are never mid-submit")
            }
        }
    }

    #[test]
    fn fm2_submitted_flip_precedes_future() {
        let controller = controller_with_valid_email();
        let observed = Rc::new(RefCell::new(None));
        let obs = observed.clone();
        let view = controller.clone();
        block_on(form_attempt(&controller, move |_snap| {
            *obs.borrow_mut() = Some(view.submitted().get());
            async { Ok::<(), std::convert::Infallible>(()) }
        }));
        assert_eq!(
            *observed.borrow(),
            Some(true),
            "`submitted` must read true at the first handler await point"
        );
        assert!(controller.submitted().get());
    }

    #[test]
    fn fm3_non_send_handler_accepted() {
        let controller = controller_with_valid_email();
        // Rc-backed non-Send state captured by the handler future.
        let state = Rc::new(RefCell::new(String::from("local")));
        let captured = state.clone();
        let outcome = block_on(finish_attempt(&controller, move |_snap| {
            let captured = captured.clone();
            async move {
                captured.borrow_mut().push_str("-done");
                Ok::<String, std::convert::Infallible>(captured.borrow().clone())
            }
        }));
        match outcome {
            SubmitOutcome::Success(s) => assert_eq!(s, "local-done"),
            other => panic!("non-Send handler must complete, got {other:?}"),
        }
        assert_eq!(state.borrow().as_str(), "local-done");
    }

    #[test]
    fn fm5_invalid_blocks_handler() {
        let controller = controller_with_invalid_email();
        let invoked = Rc::new(RefCell::new(false));
        let flag = invoked.clone();
        let outcome = block_on(finish_attempt(&controller, move |_snap| {
            flag.replace(true); // proof-of-invocation marker
            async { Ok::<(), std::convert::Infallible>(()) }
        }));
        assert!(
            matches!(outcome, SubmitOutcome::ValidationFailed(_)),
            "validation failure must surface as ValidationFailed"
        );
        assert!(
            !*invoked.borrow(),
            "handler must never be constructed when validation fails"
        );
    }

    #[test]
    fn fm5_valid_brackets_is_submitting_on_success_and_error() {
        for expect_ok in [true, false] {
            let controller = controller_with_valid_email();
            let during = Rc::new(RefCell::new(None));
            let d = during.clone();
            let c2 = controller.clone();
            let outcome = block_on(finish_attempt(&controller, move |_snap| {
                let d = d.clone();
                let c3 = c2.clone();
                async move {
                    d.replace(Some(c3.is_submitting().get()));
                    if expect_ok {
                        Ok::<(), String>(())
                    } else {
                        Err(String::from("boom"))
                    }
                }
            }));
            assert_eq!(
                *during.borrow(),
                Some(true),
                "is_submitting true while handler pending"
            );
            assert!(
                !controller.is_submitting().get(),
                "bracket closes after settle"
            );
            if expect_ok {
                assert!(matches!(outcome, SubmitOutcome::Success(())));
            } else {
                assert!(
                    matches!(&outcome, SubmitOutcome::HandlerError(e) if e == "boom"),
                    "handler error path must also close the bracket"
                );
            }
        }
    }

    #[test]
    fn fm5_success_surfaced_once_through_callback() {
        let controller = controller_with_valid_email();
        let collected: std::sync::Arc<std::sync::Mutex<Vec<SubmitOutcome<(), String>>>> =
            std::sync::Arc::default();
        let sink = collected.clone();
        let on_outcome = Callback::new(move |o: SubmitOutcome<(), String>| {
            sink.lock().unwrap().push(o);
        });
        // Exactly what <Form>'s spawned task does:
        let outcome = block_on(finish_attempt(&controller, |_snap| async {
            Ok::<(), String>(())
        }));
        on_outcome.run(outcome);
        let got = collected.lock().unwrap();
        assert_eq!(got.len(), 1, "success delivered exactly once");
        assert!(matches!(got[0], SubmitOutcome::Success(())));
    }

    #[test]
    fn guard_rejects_second_synchronous_attempt() {
        // Two submit events in the same tick: the first flips `is_submitting`
        // synchronously inside begin_attempt (before any future is polled),
        // so the second must be rejected.
        let controller = controller_with_valid_email();
        assert!(begin_attempt(&controller), "first attempt proceeds");
        assert!(
            !begin_attempt(&controller),
            "second same-tick attempt must be rejected"
        );
        assert!(controller.is_submitting().get());
        // Driving the composed future still resets the flag exactly once.
        let outcome: SubmitOutcome<(), std::convert::Infallible> =
            block_on(finish_attempt(&controller, |_snap| async { Ok(()) }));
        assert!(matches!(outcome, SubmitOutcome::Success(())));
        assert!(
            !controller.is_submitting().get(),
            "drop guard closes the bracket after the attempt settles"
        );
    }

    #[test]
    fn guard_rejects_attempt_while_in_flight() {
        use reactive_graph::traits::Set;
        let controller = controller_with_valid_email();
        // Simulate mid-flight state set from elsewhere; begin_attempt must
        // observe it and reject the attempt.
        controller.is_submitting().set(true);
        assert!(
            !begin_attempt(&controller),
            "in-flight guard must reject attempts while submitting"
        );
    }
}
