use sqlv_core::{Value, BLOB_PREVIEW_BYTES, JSON_SAFE_INTEGER_MAX};

#[test]
fn null_serializes_to_json_null() {
    assert_eq!(serde_json::to_string(&Value::Null).unwrap(), "null");
}

#[test]
fn integer_within_js_safe_range_serializes_as_number() {
    assert_eq!(serde_json::to_string(&Value::Integer(42)).unwrap(), "42");
    assert_eq!(serde_json::to_string(&Value::Integer(-1)).unwrap(), "-1");
    assert_eq!(
        serde_json::to_string(&Value::Integer(JSON_SAFE_INTEGER_MAX)).unwrap(),
        JSON_SAFE_INTEGER_MAX.to_string(),
    );
    assert_eq!(
        serde_json::to_string(&Value::Integer(-JSON_SAFE_INTEGER_MAX)).unwrap(),
        (-JSON_SAFE_INTEGER_MAX).to_string(),
    );
}

#[test]
fn integer_above_js_safe_range_serializes_as_tagged_string() {
    // 2^53 + 1 — the canonical number that JS loses precision on.
    let v = Value::Integer(JSON_SAFE_INTEGER_MAX + 2);
    let s = serde_json::to_string(&v).unwrap();
    assert_eq!(
        s,
        format!("{{\"$int64\":\"{}\"}}", JSON_SAFE_INTEGER_MAX + 2),
    );
}

#[test]
fn integer_i64_extremes_serialize_as_tagged_string() {
    let max = serde_json::to_string(&Value::Integer(i64::MAX)).unwrap();
    let min = serde_json::to_string(&Value::Integer(i64::MIN)).unwrap();
    assert!(max.contains("$int64"), "got {max}");
    assert!(min.contains("$int64"), "got {min}");
    // Parse back and verify no digits were lost.
    let parsed: serde_json::Value = serde_json::from_str(&max).unwrap();
    assert_eq!(parsed["$int64"], i64::MAX.to_string());
}

#[test]
fn real_finite_serializes_as_number() {
    let v = Value::Real(3.5);
    assert_eq!(serde_json::to_string(&v).unwrap(), "3.5");
}

#[test]
fn real_non_finite_serializes_as_tagged_label() {
    assert_eq!(
        serde_json::to_string(&Value::Real(f64::NAN)).unwrap(),
        "{\"$real\":\"NaN\"}",
    );
    assert_eq!(
        serde_json::to_string(&Value::Real(f64::INFINITY)).unwrap(),
        "{\"$real\":\"Infinity\"}",
    );
    assert_eq!(
        serde_json::to_string(&Value::Real(f64::NEG_INFINITY)).unwrap(),
        "{\"$real\":\"-Infinity\"}",
    );
}

#[test]
fn text_serializes_as_string_with_escapes() {
    let v = Value::Text("line\n\"quote\"".into());
    let json = serde_json::to_string(&v).unwrap();
    assert_eq!(json, "\"line\\n\\\"quote\\\"\"");
}

#[test]
fn blob_serializes_as_tagged_object() {
    let v = Value::Blob(vec![0xde, 0xad, 0xbe, 0xef]);
    assert_eq!(
        serde_json::to_string(&v).unwrap(),
        "{\"$blob_base64\":\"3q2+7w==\"}"
    );
}

#[test]
fn blob_above_threshold_serializes_as_truncated_with_size() {
    // Threshold + 1 byte triggers the truncated shape.
    let big = vec![0xabu8; BLOB_PREVIEW_BYTES + 1];
    let s = serde_json::to_string(&Value::Blob(big.clone())).unwrap();
    let v: serde_json::Value = serde_json::from_str(&s).unwrap();
    assert!(v.get("$blob_base64_truncated").is_some());
    assert_eq!(v["$blob_size"], big.len());
    assert!(
        v.get("$blob_base64").is_none(),
        "shouldn't also emit full form"
    );
    // Ensure the preview is exactly the threshold, not the whole thing.
    let preview = v["$blob_base64_truncated"].as_str().unwrap();
    let decoded_len = preview.trim_end_matches('=').len() * 3 / 4;
    assert_eq!(decoded_len, BLOB_PREVIEW_BYTES);
}

#[test]
fn blob_at_exactly_threshold_still_uses_full_form() {
    // Boundary: exactly THRESHOLD bytes should still be emitted whole.
    let exact = vec![0x42u8; BLOB_PREVIEW_BYTES];
    let s = serde_json::to_string(&Value::Blob(exact)).unwrap();
    let v: serde_json::Value = serde_json::from_str(&s).unwrap();
    assert!(v.get("$blob_base64").is_some());
    assert!(v.get("$blob_base64_truncated").is_none());
}

#[test]
fn blob_base64_roundtrip_lengths() {
    // Exercise the 0/1/2/3-byte tail branches of the b64 encoder.
    let cases: &[(&[u8], usize)] = &[
        (&[], 0),
        (&[1], 4),
        (&[1, 2], 4),
        (&[1, 2, 3], 4),
        (&[1, 2, 3, 4], 8),
        (&[1, 2, 3, 4, 5], 8),
    ];
    for (bytes, expected_len) in cases {
        let s = serde_json::to_string(&Value::Blob(bytes.to_vec())).unwrap();
        // shape: {"$blob_base64":"...."}
        let encoded = s
            .trim_start_matches("{\"$blob_base64\":\"")
            .trim_end_matches("\"}");
        assert_eq!(
            encoded.len(),
            *expected_len,
            "wrong b64 length for {} bytes",
            bytes.len()
        );
    }
}

#[test]
fn from_json_maps_primitives() {
    assert_eq!(Value::from_json(&serde_json::json!(null)), Value::Null);
    assert_eq!(Value::from_json(&serde_json::json!(5)), Value::Integer(5));
    assert_eq!(Value::from_json(&serde_json::json!(-3)), Value::Integer(-3));
    assert_eq!(Value::from_json(&serde_json::json!(2.5)), Value::Real(2.5));
    assert_eq!(
        Value::from_json(&serde_json::json!("hi")),
        Value::Text("hi".into())
    );
}

#[test]
fn from_json_maps_bool_to_integer() {
    assert_eq!(
        Value::from_json(&serde_json::json!(true)),
        Value::Integer(1)
    );
    assert_eq!(
        Value::from_json(&serde_json::json!(false)),
        Value::Integer(0)
    );
}

#[test]
fn from_json_collapses_arrays_and_objects_to_text() {
    match Value::from_json(&serde_json::json!([1, 2, 3])) {
        Value::Text(s) => assert_eq!(s, "[1,2,3]"),
        other => panic!("unexpected {other:?}"),
    }
    match Value::from_json(&serde_json::json!({"a": 1})) {
        Value::Text(s) => assert_eq!(s, "{\"a\":1}"),
        other => panic!("unexpected {other:?}"),
    }
}

#[test]
fn value_equality_respects_storage_class() {
    // 1 (Integer) must not equal 1.0 (Real) — SQLite treats these differently
    // in storage class and that's the invariant we encode.
    assert_ne!(Value::Integer(1), Value::Real(1.0));
    assert_ne!(Value::Text("1".into()), Value::Integer(1));
    assert_ne!(Value::Null, Value::Text(String::new()));
}
