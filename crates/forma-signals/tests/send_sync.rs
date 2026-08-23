//! FS-DEP-3: Arc-family types are Send + Sync, usable from a fresh
//! owner-free thread.

use forma_core::prelude::*;
use forma_signals::{FieldHandle, FormController, ValidateOn};
use reactive_graph::traits::{Get, Set};

const _: fn() = || {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<FormController>();
    assert_send_sync::<FieldHandle>();
    assert_send_sync::<reactive_graph::signal::ArcRwSignal<bool>>();
    assert_send_sync::<reactive_graph::computed::ArcMemo<Vec<FormaIssue>>>();
};

#[test]
fn fsdep3_controller_and_handles_are_send_sync() {
    fn assert_send<T: Send>(_v: &T) {}
    fn assert_sync<T: Sync>(_v: &T) {}
    let mut c = FormController::new(ValidateOn::Blur);
    let handle = c.register(FieldPath::key("a"), Box::new(string())).unwrap();
    assert_send(&c);
    assert_sync(&c);
    assert_send(&handle);
    assert_sync(&handle);
}

#[test]
fn fsdep3_owner_free_construction_on_fresh_thread() {
    let result = std::thread::spawn(|| {
        let mut c = FormController::new(ValidateOn::Change);
        let handle = c
            .register(FieldPath::key("email"), Box::new(string().email()))
            .expect("fresh registration on bare thread");
        handle.value().set(Value::from("user@example.com"));
        assert!(handle.dirty().get(), "edit away from snapshot reads dirty");
        c.field(&FieldPath::key("email")).unwrap().value().get()
    })
    .join()
    .expect("no panic without an ownership tree");
    assert_eq!(result, Value::from("user@example.com"));
}
