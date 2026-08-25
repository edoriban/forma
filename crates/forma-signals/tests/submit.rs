//! FSS-1..FSS-4: submit gate, `is_submitting` lifecycle, runtime-free
//! composition, snapshot isolation.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc;
use std::thread;

use forma_core::prelude::*;
use forma_signals::{FormController, FormSnapshot, SubmitError, ValidateOn};
use futures::executor::block_on;
use reactive_graph::traits::{Get, Set};

fn valid_form() -> FormController {
    let mut c = FormController::new(ValidateOn::Blur);
    c.register(FieldPath::key("name"), Box::new(string().min(1)))
        .unwrap();
    c.field(&FieldPath::key("name"))
        .unwrap()
        .value()
        .set(Value::from("ada"));
    c
}

#[test]
fn fss1_invalid_submit_resolves_validation_without_invoking_handler() {
    let mut c = FormController::new(ValidateOn::Blur);
    c.register(FieldPath::key("email"), Box::new(string().min(8)))
        .unwrap();
    c.field(&FieldPath::key("email"))
        .unwrap()
        .value()
        .set(Value::from("ab"));

    let invoked = AtomicUsize::new(0);
    let outcome: Result<(), SubmitError<std::convert::Infallible>> = block_on(c.on_submit(|_s| {
        invoked.fetch_add(1, Ordering::SeqCst);
        async { Ok(()) }
    }));
    assert!(matches!(outcome, Err(SubmitError::Validation(_))));
    assert_eq!(
        invoked.load(Ordering::SeqCst),
        0,
        "handler must never be constructed on validation failure"
    );
}

#[test]
fn fss1_valid_submit_invokes_handler_exactly_once_with_current_values() {
    let c = valid_form();
    let calls = AtomicUsize::new(0);
    let (tx, rx) = mpsc::channel();
    let result: Result<String, SubmitError<std::convert::Infallible>> =
        block_on(c.on_submit(|snapshot| {
            calls.fetch_add(1, Ordering::SeqCst);
            tx.send(snapshot).expect("receiver alive");
            async { Ok("done".to_owned()) }
        }));
    assert_eq!(result.unwrap(), "done");
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    let snapshot = rx.recv().expect("handler ran");
    assert_eq!(
        snapshot.get(&FieldPath::key("name")),
        Some(&Value::from("ada"))
    );
}

#[test]
fn fss2_is_submitting_true_while_pending_false_after_ok() {
    let c = valid_form();
    let is_submitting = c.is_submitting();
    let (entered, entered_rx) = mpsc::channel::<()>();
    let (release_tx, release_rx) = mpsc::channel::<()>();
    let observed_inside = std::sync::Arc::new(AtomicUsize::new(0));
    let observed_clone = observed_inside.clone();

    let handle = thread::spawn(move || {
        block_on(c.on_submit(move |_| {
            let inside = observed_clone.clone();
            async move {
                inside.fetch_add(1, Ordering::SeqCst);
                entered.send(()).expect("main thread alive");
                release_rx.recv().expect("release signal");
                Ok::<(), std::convert::Infallible>(())
            }
        }))
    });
    entered_rx.recv().expect("handler entered");
    assert!(
        is_submitting.get(),
        "flag must read true while the handler future is pending"
    );
    release_tx.send(()).expect("handler waiting");
    handle.join().expect("submit thread").unwrap();
    assert!(
        !is_submitting.get(),
        "flag must read false after completion"
    );
    let _ = observed_inside.load(Ordering::SeqCst);
}

#[test]
fn fss2_is_submitting_false_after_handler_err() {
    let c = valid_form();
    let is_submitting = c.is_submitting();
    let outcome: Result<(), SubmitError<&'static str>> =
        block_on(c.on_submit(|_| async { Err("boom") }));
    assert!(matches!(outcome, Err(SubmitError::Handler("boom"))));
    assert!(!is_submitting.get());
}

#[test]
fn fss3_future_driven_by_futures_executor_block_on_no_spawner_config() {
    let c = valid_form();
    let fut = c.on_submit(|_| async { Ok(()) });
    let outcome: Result<(), SubmitError<std::convert::Infallible>> = block_on(fut);
    assert!(
        outcome.is_ok(),
        "minimal executor must drive the composed future"
    );
}

#[test]
fn fss4_snapshot_retains_pre_edit_values_during_in_flight_handler() {
    let c = valid_form();
    let path = FieldPath::key("name");
    let (in_handler_tx, in_handler_rx) = mpsc::channel::<FormSnapshot>();
    let (release_tx, release_rx) = mpsc::channel::<()>();

    let submitter = c.clone();
    let handle = thread::spawn(move || {
        block_on(submitter.on_submit(move |snapshot| async move {
            in_handler_tx.send(snapshot).expect("main thread alive");
            release_rx.recv().expect("release signal");
            Ok::<(), std::convert::Infallible>(())
        }))
    });

    let snapshot = in_handler_rx.recv().expect("handler received snapshot");
    c.field(&path)
        .unwrap()
        .value()
        .set(Value::from("edited-in-flight"));
    assert_eq!(
        snapshot.get(&path),
        Some(&Value::from("ada")),
        "edits during flight must not mutate the handler's snapshot"
    );
    release_tx.send(()).expect("handler waiting");
    handle.join().expect("submit thread").unwrap();
}

#[test]
fn fss2_dropped_future_resets_is_submitting() {
    use std::future::Future;
    use std::task::{Context, Poll};

    let c = valid_form();
    let mut fut = Box::pin(
        c.on_submit(|_| std::future::pending::<Result<(), std::convert::Infallible>>()),
    );
    let waker = futures::task::noop_waker();
    let mut cx = Context::from_waker(&waker);
    assert!(matches!(fut.as_mut().poll(&mut cx), Poll::Pending));
    assert!(
        c.is_submitting().get(),
        "flag must be true after the first poll"
    );
    drop(fut);
    assert!(
        !c.is_submitting().get(),
        "dropping (cancelling) the composed future must reset the flag"
    );
}
