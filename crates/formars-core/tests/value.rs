//! DV-1 ordered entries and number fidelity.

use formars_core::prelude::*;

#[test]
fn dv1_ordered_entries_preserved() {
    let mut o = formars_core::value::Object::new();
    o.insert("b", Value::I64(1));
    o.insert("a", Value::I64(2));
    o.insert("c", Value::I64(3));
    let keys: Vec<&str> = o.iter().map(|(k, _)| k.as_ref()).collect();
    assert_eq!(
        keys,
        vec!["b", "a", "c"],
        "iteration follows insertion order, not sorted/hashed"
    );
}

#[test]
fn dv1_number_fidelity_i64_max_and_f64() {
    let v = Value::I64(i64::MAX);
    assert_eq!(v.as_i64(), Some(i64::MAX), "exact i64 precision retained");
    let f = Value::F64(0.1);
    assert_eq!(f.as_f64(), Some(0.1), "f64 semantics retained");
    assert_eq!(f.as_i64(), None, "no silent F64-to-I64 cast");
}
