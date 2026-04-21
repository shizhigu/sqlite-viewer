use sqlv_core::Value;

#[test]
fn null_serializes_to_json_null() {
    assert_eq!(serde_json::to_string(&Value::Null).unwrap(), "null");
}

#[test]
fn integer_serializes_as_number() {
    assert_eq!(serde_json::to_string(&Value::Integer(42)).unwrap(), "42");
    assert_eq!(serde_json::to_string(&Value::Integer(-1)).unwrap(), "-1");
    assert_eq!(
        serde_json::to_string(&Value::Integer(i64::MAX)).unwrap(),
        i64::MAX.to_string()
    );
}

#[test]
fn real_serializes_as_number() {
    let v = Value::Real(3.5);
    assert_eq!(serde_json::to_string(&v).unwrap(), "3.5");
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
        let encoded = s.trim_start_matches("{\"$blob_base64\":\"").trim_end_matches("\"}");
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
    assert_eq!(Value::from_json(&serde_json::json!("hi")), Value::Text("hi".into()));
}

#[test]
fn from_json_maps_bool_to_integer() {
    assert_eq!(Value::from_json(&serde_json::json!(true)), Value::Integer(1));
    assert_eq!(Value::from_json(&serde_json::json!(false)), Value::Integer(0));
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
