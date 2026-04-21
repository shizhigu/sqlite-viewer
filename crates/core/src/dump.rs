use std::fmt::Write as _;

use crate::connection::{quote_ident, Db};
use crate::error::Result;

#[derive(Debug, Clone, Copy)]
pub struct DumpFilter<'a> {
    pub schema: bool,
    pub data: bool,
    pub only_tables: Option<&'a [String]>,
}

impl Default for DumpFilter<'_> {
    fn default() -> Self {
        Self { schema: true, data: true, only_tables: None }
    }
}

impl Db {
    /// Dump the database to a single SQL string (pragma + schema + data).
    ///
    /// For v1 we return the whole dump as one owned `String` rather than a
    /// streaming iterator. The desktop + CLI consumers don't need streaming
    /// yet, and materialization keeps the logic simple. If we hit large-DB
    /// use cases later, swap for `impl Iterator<Item=Result<String>>`.
    pub fn dump(&self, filter: DumpFilter<'_>) -> Result<String> {
        let mut out = String::new();
        writeln!(out, "PRAGMA foreign_keys = OFF;").ok();
        writeln!(out, "BEGIN TRANSACTION;").ok();

        if filter.schema {
            self.dump_schema(&mut out, filter.only_tables)?;
        }
        if filter.data {
            self.dump_data(&mut out, filter.only_tables)?;
        }

        writeln!(out, "COMMIT;").ok();
        Ok(out)
    }

    fn dump_schema(
        &self,
        out: &mut String,
        only: Option<&[String]>,
    ) -> Result<()> {
        // Emit tables first, then indexes/triggers/views — matches `.dump` in
        // the sqlite3 shell. Use `WHERE type IN (...)` ordered so that parent
        // objects come before their indexes.
        let mut stmt = self.conn().prepare(
            "SELECT type, name, sql, tbl_name FROM sqlite_master \
             WHERE sql IS NOT NULL AND name NOT LIKE 'sqlite_%' \
             ORDER BY CASE type \
                 WHEN 'table' THEN 1 \
                 WHEN 'index' THEN 2 \
                 WHEN 'trigger' THEN 3 \
                 WHEN 'view' THEN 4 \
                 ELSE 5 END, name",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?, // type
                r.get::<_, String>(1)?, // name
                r.get::<_, String>(2)?, // sql
                r.get::<_, String>(3)?, // tbl_name
            ))
        })?;
        for row in rows {
            let (_typ, _name, sql, tbl_name) = row?;
            if let Some(filter) = only {
                if !filter.iter().any(|f| f == &tbl_name) {
                    continue;
                }
            }
            writeln!(out, "{sql};").ok();
        }
        Ok(())
    }

    fn dump_data(&self, out: &mut String, only: Option<&[String]>) -> Result<()> {
        let tables = self.tables()?;
        for t in tables {
            if let Some(filter) = only {
                if !filter.iter().any(|f| f == &t.name) {
                    continue;
                }
            }
            self.dump_table_data(&t.name, out)?;
        }
        Ok(())
    }

    fn dump_table_data(&self, table: &str, out: &mut String) -> Result<()> {
        let qt = quote_ident(table);
        let select_sql = format!("SELECT * FROM {qt}");
        let mut stmt = self.conn().prepare(&select_sql)?;
        let col_names: Vec<String> =
            stmt.column_names().into_iter().map(String::from).collect();
        if col_names.is_empty() {
            return Ok(());
        }
        let col_list = col_names
            .iter()
            .map(|c| quote_ident(c))
            .collect::<Vec<_>>()
            .join(", ");

        let mut rows = stmt.query([])?;
        while let Some(row) = rows.next()? {
            let mut value_exprs: Vec<String> = Vec::with_capacity(col_names.len());
            for i in 0..col_names.len() {
                value_exprs.push(render_literal(row.get_ref(i)?));
            }
            writeln!(
                out,
                "INSERT INTO {qt} ({col_list}) VALUES ({});",
                value_exprs.join(", ")
            )
            .ok();
        }
        Ok(())
    }
}

fn render_literal(v: rusqlite::types::ValueRef<'_>) -> String {
    use rusqlite::types::ValueRef;
    match v {
        ValueRef::Null => "NULL".into(),
        ValueRef::Integer(i) => i.to_string(),
        ValueRef::Real(f) => {
            // SQLite's REAL may lose trailing zeros; format to something that
            // round-trips. Using Rust's default {} is fine for our needs.
            let s = format!("{f}");
            // Ensure the literal looks like a number, not an integer.
            if !s.contains('.') && !s.contains('e') && !s.contains('E') {
                format!("{s}.0")
            } else {
                s
            }
        }
        ValueRef::Text(bytes) => {
            let s = std::str::from_utf8(bytes).unwrap_or("");
            let mut escaped = String::with_capacity(s.len() + 2);
            escaped.push('\'');
            for ch in s.chars() {
                if ch == '\'' {
                    escaped.push('\'');
                    escaped.push('\'');
                } else {
                    escaped.push(ch);
                }
            }
            escaped.push('\'');
            escaped
        }
        ValueRef::Blob(bytes) => {
            // SQLite blob literal syntax: X'hex'
            let mut s = String::with_capacity(bytes.len() * 2 + 3);
            s.push_str("X'");
            for b in bytes {
                use std::fmt::Write as _;
                let _ = write!(s, "{b:02X}");
            }
            s.push('\'');
            s
        }
    }
}
