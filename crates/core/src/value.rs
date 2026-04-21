use rusqlite::types::{ToSqlOutput, ValueRef};
use serde::ser::{Serialize, SerializeMap, Serializer};

/// A SQLite cell value, preserving the four storage classes plus NULL.
///
/// JSON serialization is lossy-safe: when a value doesn't fit in JSON's
/// native types, we tag it so the consumer (CLI / frontend / MCP host) can
/// recover the exact SQLite value instead of silently losing precision.
///
/// - `Null` → `null`
/// - `Integer` → JSON number when it fits in JS's safe-integer range
///   (±2⁵³-1); otherwise `{"$int64": "9007199254740993"}`. JavaScript's
///   `number` is a float64 — larger integers lose precision silently.
/// - `Real` → JSON number when finite; otherwise
///   `{"$real": "NaN" | "Infinity" | "-Infinity"}`. `serde_json` rejects
///   non-finite floats; we turn them into a stable tagged form instead.
/// - `Text` → JSON string
/// - `Blob` ≤ 16 KiB → `{"$blob_base64": "..."}`
/// - `Blob` > 16 KiB → `{"$blob_base64_truncated": "<first 16 KiB>",
///    "$blob_size": <total_bytes>}`. Full-blob access is a separate
///   endpoint (agents/consumers that need the whole blob can re-query).
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Null,
    Integer(i64),
    Real(f64),
    Text(String),
    Blob(Vec<u8>),
}

/// Largest integer that JS / JSON can represent exactly (2⁵³ - 1).
pub const JSON_SAFE_INTEGER_MAX: i64 = 9_007_199_254_740_991;
/// Max blob size we'll fully embed inline in a JSON response.
pub const BLOB_PREVIEW_BYTES: usize = 16 * 1024;

impl Value {
    pub fn from_json(v: &serde_json::Value) -> Self {
        match v {
            serde_json::Value::Null => Value::Null,
            serde_json::Value::Bool(b) => Value::Integer(if *b { 1 } else { 0 }),
            serde_json::Value::Number(n) => {
                if let Some(i) = n.as_i64() {
                    Value::Integer(i)
                } else if let Some(f) = n.as_f64() {
                    Value::Real(f)
                } else {
                    Value::Null
                }
            }
            serde_json::Value::String(s) => Value::Text(s.clone()),
            serde_json::Value::Array(_) | serde_json::Value::Object(_) => {
                Value::Text(v.to_string())
            }
        }
    }
}

impl Serialize for Value {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match self {
            Value::Null => s.serialize_unit(),
            Value::Integer(i) => {
                if (-JSON_SAFE_INTEGER_MAX..=JSON_SAFE_INTEGER_MAX).contains(i) {
                    s.serialize_i64(*i)
                } else {
                    // Tag as a decimal string so JS / Python consumers that
                    // parse the response can losslessly reconstruct the i64.
                    let mut m = s.serialize_map(Some(1))?;
                    m.serialize_entry("$int64", &i.to_string())?;
                    m.end()
                }
            }
            Value::Real(f) => {
                if f.is_finite() {
                    s.serialize_f64(*f)
                } else {
                    let label = if f.is_nan() {
                        "NaN"
                    } else if f.is_sign_negative() {
                        "-Infinity"
                    } else {
                        "Infinity"
                    };
                    let mut m = s.serialize_map(Some(1))?;
                    m.serialize_entry("$real", label)?;
                    m.end()
                }
            }
            Value::Text(t) => s.serialize_str(t),
            Value::Blob(b) => {
                if b.len() <= BLOB_PREVIEW_BYTES {
                    let mut m = s.serialize_map(Some(1))?;
                    m.serialize_entry("$blob_base64", &b64_encode(b))?;
                    m.end()
                } else {
                    let mut m = s.serialize_map(Some(2))?;
                    m.serialize_entry(
                        "$blob_base64_truncated",
                        &b64_encode(&b[..BLOB_PREVIEW_BYTES]),
                    )?;
                    m.serialize_entry("$blob_size", &b.len())?;
                    m.end()
                }
            }
        }
    }
}

impl rusqlite::ToSql for Value {
    fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
        Ok(match self {
            Value::Null => ToSqlOutput::Borrowed(ValueRef::Null),
            Value::Integer(i) => ToSqlOutput::Owned(rusqlite::types::Value::Integer(*i)),
            Value::Real(f) => ToSqlOutput::Owned(rusqlite::types::Value::Real(*f)),
            Value::Text(s) => ToSqlOutput::Borrowed(ValueRef::Text(s.as_bytes())),
            Value::Blob(b) => ToSqlOutput::Borrowed(ValueRef::Blob(b)),
        })
    }
}

impl From<ValueRef<'_>> for Value {
    fn from(v: ValueRef<'_>) -> Self {
        match v {
            ValueRef::Null => Value::Null,
            ValueRef::Integer(i) => Value::Integer(i),
            ValueRef::Real(f) => Value::Real(f),
            ValueRef::Text(bytes) => Value::Text(String::from_utf8_lossy(bytes).into_owned()),
            ValueRef::Blob(bytes) => Value::Blob(bytes.to_vec()),
        }
    }
}

// Minimal RFC 4648 base64 encoder. Kept inline to avoid a dep for a handful
// of bytes — we only hit this path for blob cells.
fn b64_encode(input: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(input.len().div_ceil(3) * 4);
    let mut chunks = input.chunks_exact(3);
    for c in chunks.by_ref() {
        let n = ((c[0] as u32) << 16) | ((c[1] as u32) << 8) | (c[2] as u32);
        out.push(ALPHABET[((n >> 18) & 0x3F) as usize] as char);
        out.push(ALPHABET[((n >> 12) & 0x3F) as usize] as char);
        out.push(ALPHABET[((n >> 6) & 0x3F) as usize] as char);
        out.push(ALPHABET[(n & 0x3F) as usize] as char);
    }
    let rem = chunks.remainder();
    match rem.len() {
        1 => {
            let n = (rem[0] as u32) << 16;
            out.push(ALPHABET[((n >> 18) & 0x3F) as usize] as char);
            out.push(ALPHABET[((n >> 12) & 0x3F) as usize] as char);
            out.push('=');
            out.push('=');
        }
        2 => {
            let n = ((rem[0] as u32) << 16) | ((rem[1] as u32) << 8);
            out.push(ALPHABET[((n >> 18) & 0x3F) as usize] as char);
            out.push(ALPHABET[((n >> 12) & 0x3F) as usize] as char);
            out.push(ALPHABET[((n >> 6) & 0x3F) as usize] as char);
            out.push('=');
        }
        _ => {}
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_shapes() {
        assert_eq!(serde_json::to_string(&Value::Null).unwrap(), "null");
        assert_eq!(serde_json::to_string(&Value::Integer(42)).unwrap(), "42");
        assert_eq!(
            serde_json::to_string(&Value::Text("hi".into())).unwrap(),
            "\"hi\""
        );
        assert_eq!(
            serde_json::to_string(&Value::Blob(vec![0xde, 0xad, 0xbe, 0xef])).unwrap(),
            "{\"$blob_base64\":\"3q2+7w==\"}"
        );
    }
}
