//! DV-4 serde round-trip (requires the `serde` feature).

#![cfg(feature = "serde")]

use formars_core::value::Value;

#[test]
fn dv4_bidirectional_roundtrip_with_ordered_entries() {
    let mut o = formars_core::value::Object::new();
    o.insert("z", Value::I64(1));
    o.insert("a", Value::I64(2));
    o.insert("m", Value::String("s".into()));
    let original = Value::Object(o);

    let json = serde_json::to_value(&original).expect("to serde_json");
    let back: Value = serde_json::from_value(json).expect("from serde_json");
    assert_eq!(
        back, original,
        "round-trip preserves everything including entry order"
    );

    if let Value::Object(entries_back) = &back {
        let keys: Vec<&str> = entries_back.iter().map(|(k, _)| k.as_ref()).collect();
        assert_eq!(
            keys,
            vec!["z", "a", "m"],
            "serde_json preserve_order keeps insertion order"
        );
    } else {
        panic!("expected object");
    }
}

#[test]
fn dv4_u64_above_i64_max_maps_to_f64_documented_lossy() {
    let json = serde_json::json!(u64::MAX);
    let v: Value = serde_json::from_value(json).expect("conversion succeeds");
    match v {
        Value::F64(f) => {
            // `u64::MAX` widens to exactly 2^64 under round-to-nearest;
            // computed without a cast and compared bit-for-bit so the
            // documented lossy value is pinned precisely.
            let expected = f64::powi(2.0, 64);
            assert_eq!(
                f.to_bits(),
                expected.to_bits(),
                "documented lossy mapping for ints beyond i64"
            );
        }
        other => panic!("expected F64 (documented lossy), got {other:?}"),
    }
}
