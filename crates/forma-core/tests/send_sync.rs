//! Compile-time probes pinning `Send + Sync` guarantees on erased schema
//! views (spec `sdd/forma-core-send-sync`, SS-1..SS-5).
//!
//! Presence in the binary IS the assertion: a lost supertrait fails the
//! build with E0277 here, before any downstream crate breaks.

use std::sync::Arc;

use forma_core::coerce::CoercedSchema;
use forma_core::prelude::*;
use forma_core::rule::Rule;
use forma_core::schema::DynSchema;
use forma_core::types::{BoolSchema, NumberSchema, StringSchema};

fn assert_send_sync<T: Send + Sync>() {}

fn assert_fn_send_sync_static<F: Fn() + Send + Sync + 'static>(_: F) {}

#[test]
fn ss3_builtin_schemas_are_send_sync() {
    assert_send_sync::<StringSchema>();
    assert_send_sync::<NumberSchema<i64>>();
    assert_send_sync::<NumberSchema<f64>>();
    assert_send_sync::<BoolSchema>();
    assert_send_sync::<CoercedSchema<u32>>();
}

#[test]
fn ss12_erased_views_are_send_sync() {
    assert_send_sync::<Box<dyn Rule<str>>>();
    assert_send_sync::<Box<dyn DynSchema>>();

    let s: Box<dyn DynSchema> = Box::new(string().min(3));
    let _r: &dyn DynSchema = &*s;
    assert_send_sync::<&dyn DynSchema>();
}

#[test]
fn ss5_memo_capturable_schema() {
    let s: Arc<Box<dyn DynSchema>> = Arc::new(Box::new(string().min(3)));
    assert_fn_send_sync_static(move || drop(s.clone()));
}

// ----------------------------------------------- DV-8: object family probes

#[test]
fn dv8_object_schema_is_send_sync() {
    assert_send_sync::<forma_core::types::ObjectSchema>();

    let schema = object()
        .field("name", string().min(1).refine(|s| !s.is_empty()))
        .field("age", forma_core::coerce::coerced::<u32>())
        .field("sub", object().field("ok", bool().equals(true)));
    assert_send_sync::<forma_core::types::ObjectSchema>();
    assert_send_sync::<Box<dyn DynSchema>>();

    let erased: Box<dyn DynSchema> = Box::new(schema);
    let arc: Arc<Box<dyn DynSchema>> = Arc::new(erased);
    assert_fn_send_sync_static(move || drop(arc.clone()));
}
