use serde::Serialize;

use crate::connection::Db;
use crate::error::{Error, Result};

#[derive(Debug, Clone, Serialize)]
pub struct PragmaValue {
    pub name: String,
    /// Pragmas can return multiple rows (e.g. `table_info`) or multiple
    /// columns; we flatten to a vector of stringified values for a uniform
    /// shape. Callers that need structured pragma output should use the
    /// dedicated schema/stats helpers instead.
    pub values: Vec<Vec<String>>,
}

impl Db {
    /// Read a pragma. If `new_value` is `Some`, set it first (requires a
    /// read-write connection) and then return the result of re-reading it.
    pub fn pragma(&self, name: &str, new_value: Option<&str>) -> Result<PragmaValue> {
        if !is_safe_pragma_name(name) {
            return Err(Error::Invalid(format!(
                "pragma name '{name}' contains invalid characters"
            )));
        }

        if let Some(val) = new_value {
            if self.is_read_only() {
                return Err(Error::ReadOnly);
            }
            // PRAGMAs don't support parameter binding, so values must be
            // validated by the caller. We accept numbers and bare keywords;
            // anything else needs quoting via `'...'` by the user.
            if !is_safe_pragma_value(val) {
                return Err(Error::Invalid(format!(
                    "pragma value '{val}' must be numeric, keyword, or single-quoted"
                )));
            }
            let sql = format!("PRAGMA {name} = {val}");
            self.conn().execute_batch(&sql)?;
        }

        let sql = format!("PRAGMA {name}");
        let mut stmt = self.conn().prepare(&sql)?;
        let col_count = stmt.column_count();
        let mut rows = stmt.query([])?;
        let mut values: Vec<Vec<String>> = Vec::new();
        while let Some(row) = rows.next()? {
            let mut r = Vec::with_capacity(col_count);
            for i in 0..col_count {
                let raw = row.get_ref(i)?;
                r.push(match raw {
                    rusqlite::types::ValueRef::Null => "".into(),
                    rusqlite::types::ValueRef::Integer(n) => n.to_string(),
                    rusqlite::types::ValueRef::Real(f) => f.to_string(),
                    rusqlite::types::ValueRef::Text(b) => String::from_utf8_lossy(b).into_owned(),
                    rusqlite::types::ValueRef::Blob(_) => "<blob>".into(),
                });
            }
            values.push(r);
        }
        Ok(PragmaValue {
            name: name.to_string(),
            values,
        })
    }
}

fn is_safe_pragma_name(s: &str) -> bool {
    !s.is_empty() && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}

fn is_safe_pragma_value(s: &str) -> bool {
    if s.is_empty() {
        return false;
    }
    // Numeric (possibly signed).
    if s.chars()
        .all(|c| c.is_ascii_digit() || c == '-' || c == '+' || c == '.')
    {
        return true;
    }
    // Single-quoted literal, no embedded quotes.
    if s.starts_with('\'') && s.ends_with('\'') && !s[1..s.len() - 1].contains('\'') {
        return true;
    }
    // Bare keyword (alnum + underscore).
    if s.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
        return true;
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn safe_pragma_names() {
        assert!(is_safe_pragma_name("user_version"));
        assert!(is_safe_pragma_name("journal_mode"));
        assert!(!is_safe_pragma_name(""));
        assert!(!is_safe_pragma_name("user;drop"));
        assert!(!is_safe_pragma_name("name with space"));
    }

    #[test]
    fn safe_pragma_values() {
        assert!(is_safe_pragma_value("42"));
        assert!(is_safe_pragma_value("-1"));
        assert!(is_safe_pragma_value("3.14"));
        assert!(is_safe_pragma_value("WAL"));
        assert!(is_safe_pragma_value("'some string'"));
        assert!(!is_safe_pragma_value(""));
        assert!(!is_safe_pragma_value("1; DROP TABLE t"));
        assert!(!is_safe_pragma_value("'bad ' quote'"));
    }
}
