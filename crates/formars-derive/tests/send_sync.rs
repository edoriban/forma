//! `Send + Sync` pinning for derived companions (spec #1635: DP-2), extending
//! the formars-core SS-lineage probe style.
//!
//! Phase B slice: companion schema containing a nested derived child is
//! `Send + Sync`; its `Box<dyn DynSchema>` too. Extended in Phase C with
//! refine-closure and boxed-rule override probes.

use std::sync::Arc;

use formars_core::schema::DynSchema;
use formars_derive::FormSchema;

fn assert_send_sync<T: Send + Sync>() {}

fn assert_fn_send_sync_static<F: Fn() + Send + Sync + 'static>(_: F) {}

#[derive(FormSchema)]
struct Leaf {
    qty: u32,
}

#[derive(FormSchema)]
struct Branch {
    leaf: Leaf,
    note: String,
}

/// Full DP-2 pin (Phase C): refine closure capturing &'static str, a
/// boxed-rule-bearing `schema =` override, and a nested derived child — the
/// companion and its erased view are all `Send + Sync`.
#[derive(FormSchema)]
struct Pinned {
    #[form(schema = ::formars_core::types::string().min(2).refine(|s: &str| s != PINNED_SENTINEL))]
    guarded: String,
    #[form(schema = ::formars_core::types::string().trim().refine(|s: &str| !s.is_empty()))]
    boxed_rule: String,
    child: Leaf,
}

const PINNED_SENTINEL: &str = "forbidden";

#[test]
fn dp2_full_pin_refine_override_and_nested_child_are_send_sync() {
    assert_send_sync::<PinnedSchema>();

    let erased: Box<dyn DynSchema> = Box::new(PinnedSchema::new());
    assert_send_sync::<Box<dyn DynSchema>>();

    let arc: Arc<Box<dyn DynSchema>> = Arc::new(erased);
    assert_fn_send_sync_static(move || drop(arc.clone()));
}

#[test]
fn dp2_companion_with_nested_derived_child_is_send_sync() {
    assert_send_sync::<BranchSchema>();

    let erased: Box<dyn DynSchema> = Box::new(BranchSchema::new());
    assert_send_sync::<Box<dyn DynSchema>>();

    let arc: Arc<Box<dyn DynSchema>> = Arc::new(erased);
    assert_fn_send_sync_static(move || drop(arc.clone()));
}
