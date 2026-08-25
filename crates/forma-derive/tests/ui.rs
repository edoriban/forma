//! trybuild UI suite (spec #1635: DQ-1/DQ-3). Every case pins exact rendered
//! stderr — message AND primary span.
//!
//! Workflow: after intentionally changing diagnostics, regenerate snapshots
//! with `TRYBUILD=overwrite cargo test -p forma-derive --test ui`, then
//! REVIEW every `.stderr` diff by hand before committing (DQ-1 quality gate).

#[test]
fn ui() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/ui/*.rs");
}
