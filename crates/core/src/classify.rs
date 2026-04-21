//! Heuristic SQL classification for push-time preview.
//!
//! The rule is simple and deliberately over-cautious: if any mutating
//! keyword appears as a standalone word anywhere in the (comment-stripped)
//! statement, it's classified `Mutating`. Otherwise `ReadOnly`. We prefer
//! false positives (preview a harmless statement) over false negatives
//! (silently run an UPDATE the user didn't expect).
//!
//! This is NOT a SQL parser. Real parsing would bring in `sqlparser-rs`
//! (~1 MB of dependencies) and still get SQLite-specific edge cases wrong.
//! The keyword list below is the SQLite DML/DDL surface.

use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SqlKind {
    /// SELECT / EXPLAIN / WITH-that-ends-in-SELECT / most PRAGMAs.
    ReadOnly,
    /// Any INSERT / UPDATE / DELETE / CREATE / DROP / ALTER / VACUUM / ...
    Mutating,
}

/// Keywords that force classification as mutating. Trailing space is
/// intentional on multi-sense tokens (`CREATE ` avoids matching
/// `CREATED_AT` as a column name); word-boundary check handles the rest.
const MUTATING_KEYWORDS: &[&str] = &[
    "INSERT", "UPDATE", "DELETE", "REPLACE", "MERGE", "UPSERT", "CREATE", "DROP", "ALTER",
    "TRUNCATE", "RENAME", "ATTACH", "DETACH", "VACUUM", "REINDEX",
];

pub fn classify(sql: &str) -> SqlKind {
    let stripped = strip_comments(sql);
    let upper = stripped.to_ascii_uppercase();
    for kw in MUTATING_KEYWORDS {
        if contains_word(&upper, kw) {
            return SqlKind::Mutating;
        }
    }
    SqlKind::ReadOnly
}

/// Remove `-- line comments` and `/* block comments */` from the input.
/// Doesn't try to be perfect about string literal boundaries — an UPDATE
/// keyword inside a string literal will still trigger Mutating, which is
/// the safe direction.
fn strip_comments(sql: &str) -> String {
    let mut out = String::with_capacity(sql.len());
    let mut chars = sql.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '-' if chars.peek() == Some(&'-') => {
                // Consume until end of line.
                while let Some(&nc) = chars.peek() {
                    if nc == '\n' {
                        break;
                    }
                    chars.next();
                }
            }
            '/' if chars.peek() == Some(&'*') => {
                chars.next(); // consume '*'
                let mut prev = '\0';
                for nc in chars.by_ref() {
                    if prev == '*' && nc == '/' {
                        break;
                    }
                    prev = nc;
                }
            }
            _ => out.push(c),
        }
    }
    out
}

/// Does `haystack` contain `needle` as a full alphanumeric word?
/// Both are expected to be pre-uppercased by the caller.
fn contains_word(haystack: &str, needle: &str) -> bool {
    let bytes = haystack.as_bytes();
    let n = needle.as_bytes();
    let mut i = 0;
    while i + n.len() <= bytes.len() {
        if &bytes[i..i + n.len()] == n {
            let before_ok = i == 0 || !is_ident_byte(bytes[i - 1]);
            let after_ok = i + n.len() == bytes.len() || !is_ident_byte(bytes[i + n.len()]);
            if before_ok && after_ok {
                return true;
            }
        }
        i += 1;
    }
    false
}

fn is_ident_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_select_is_read_only() {
        assert_eq!(classify("SELECT * FROM users"), SqlKind::ReadOnly);
        assert_eq!(classify("  select 1"), SqlKind::ReadOnly);
    }

    #[test]
    fn explain_is_read_only() {
        assert_eq!(classify("EXPLAIN QUERY PLAN SELECT 1"), SqlKind::ReadOnly,);
    }

    #[test]
    fn pragma_read_is_read_only() {
        // PRAGMA reads don't touch data.
        assert_eq!(classify("PRAGMA table_info(users)"), SqlKind::ReadOnly);
    }

    #[test]
    fn with_ending_in_select_is_read_only() {
        let sql = "WITH t AS (SELECT 1) SELECT * FROM t";
        assert_eq!(classify(sql), SqlKind::ReadOnly);
    }

    #[test]
    fn with_ending_in_update_is_mutating() {
        let sql = "WITH t AS (SELECT 1) UPDATE users SET name='x' WHERE id=1";
        assert_eq!(classify(sql), SqlKind::Mutating);
    }

    #[test]
    fn ddl_and_dml_keywords_are_mutating() {
        for sql in [
            "INSERT INTO t VALUES (1)",
            "UPDATE t SET x=1",
            "DELETE FROM t",
            "CREATE TABLE t(x)",
            "DROP TABLE t",
            "ALTER TABLE t ADD COLUMN y",
            "VACUUM",
            "REINDEX",
            "ATTACH DATABASE 'x.db' AS x",
            "DETACH DATABASE x",
            "REPLACE INTO t VALUES (1)",
        ] {
            assert_eq!(classify(sql), SqlKind::Mutating, "{sql}");
        }
    }

    #[test]
    fn case_insensitive() {
        assert_eq!(classify("update t set x=1"), SqlKind::Mutating);
        assert_eq!(classify("Select 1"), SqlKind::ReadOnly);
    }

    #[test]
    fn mutating_keyword_in_identifier_does_not_trigger() {
        // `created_at` contains CREATE only as a substring, not a word.
        assert_eq!(
            classify("SELECT created_at, updated_at FROM users"),
            SqlKind::ReadOnly,
        );
        // `UPDATES` (plural noun), `INSERTING` — not standalone keywords.
        assert_eq!(classify("SELECT 'updates' FROM logs"), SqlKind::ReadOnly,);
    }

    #[test]
    fn keyword_in_line_comment_ignored() {
        assert_eq!(classify("-- UPDATE t SET x=1\nSELECT 1"), SqlKind::ReadOnly,);
    }

    #[test]
    fn keyword_in_block_comment_ignored() {
        assert_eq!(
            classify("/* don't really UPDATE */ SELECT 1"),
            SqlKind::ReadOnly,
        );
    }

    #[test]
    fn keyword_in_string_literal_still_flagged_over_cautiously() {
        // We err toward false-positive preview, not silent execution.
        assert_eq!(
            classify("SELECT 'the user will UPDATE his profile' AS msg"),
            SqlKind::Mutating,
        );
    }
}
