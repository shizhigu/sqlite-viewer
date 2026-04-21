use rusqlite::types::{ToSqlOutput, ValueRef};
use serde::ser::{Serialize, SerializeMap, Serializer};

/// A SQLite cell value, preserving the four storage classes plus NULL.
///
/// JSON serialization:
/// - `Null` → `null`
/// - `Integer` → JSON number
/// - `Real` → JSON number
/// - `Text` → JSON string
/// - `Blob` → `{"$blob_base64": "..."}` — object tag so downstream code can
///   distinguish strings from blobs without guessing.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Null,
    Integer(i64),
    Real(f64),
    Text(String),
    Blob(Vec<u8>),
}

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
            Value::Integer(i) => s.serialize_i64(*i),
            Value::Real(f) => s.serialize_f64(*f),
            Value::Text(t) => s.serialize_str(t),
            Value::Blob(b) => {
                let mut m = s.serialize_map(Some(1))?;
                m.serialize_entry("$blob_base64", &b64_encode(b))?;
                m.end()
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
