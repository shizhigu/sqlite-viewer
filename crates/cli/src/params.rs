use sqlv_core::Value;

use crate::exit::Failure;

/// Parse a raw `--param` argument. Each value is a JSON literal so that the
/// caller can unambiguously express integers, strings, null, arrays, etc.
///
///   `-p 42`        → Value::Integer(42)
///   `-p '"Alice"'` → Value::Text("Alice")
///   `-p null`      → Value::Null
///   `-p true`      → Value::Integer(1)
///
/// Bare identifiers (not valid JSON) are rejected — use `'"..."'` for strings
/// so there is no ambiguity at the shell boundary.
pub fn parse_params(raw: &[String]) -> Result<Vec<Value>, Failure> {
    raw.iter()
        .enumerate()
        .map(|(i, s)| parse_one(s).map_err(|e| {
            Failure::usage(format!("--param #{}: {}: {}", i + 1, e, s))
        }))
        .collect()
}

fn parse_one(raw: &str) -> Result<Value, String> {
    let v: serde_json::Value = serde_json::from_str(raw)
        .map_err(|e| format!("invalid JSON ({e})"))?;
    Ok(Value::from_json(&v))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_null_integer_string_array() {
        let out = parse_params(&[
            "null".into(),
            "42".into(),
            "\"hi\"".into(),
            "[1,2,3]".into(),
            "true".into(),
        ])
        .unwrap();
        assert_eq!(out[0], Value::Null);
        assert_eq!(out[1], Value::Integer(42));
        assert_eq!(out[2], Value::Text("hi".into()));
        match &out[3] {
            Value::Text(t) => assert_eq!(t, "[1,2,3]"),
            other => panic!("unexpected {other:?}"),
        }
        assert_eq!(out[4], Value::Integer(1));
    }

    #[test]
    fn rejects_bare_identifier() {
        let err = parse_params(&["hello".into()]).unwrap_err();
        assert_eq!(err.code(), "usage");
    }
}
