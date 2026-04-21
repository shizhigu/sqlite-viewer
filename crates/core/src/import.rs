use std::fs::File;
use std::path::Path;
use std::time::Instant;

use serde::Serialize;

use crate::connection::{quote_ident, Db};
use crate::error::{Error, Result};
use crate::value::Value;

#[derive(Debug, Clone, Serialize)]
pub struct ImportResult {
    pub rows_inserted: u64,
    pub elapsed_ms: u64,
    /// Column names that were actually written to (from the CSV header or
    /// the target table in order).
    pub columns: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct CsvImportOpts {
    pub has_header: bool,
    pub delimiter: u8,
    /// When set, any field whose raw string equals this exact value is
    /// inserted as `NULL` instead of as `TEXT`. CSV has no native NULL
    /// representation, so this is a policy choice the caller must make
    /// explicit. Common settings:
    ///   - `Some("")`      — unquoted empty fields become NULL.
    ///   - `Some("NULL")`  — the literal string `NULL` becomes NULL.
    ///   - `None`          — every field is TEXT (status quo).
    pub null_token: Option<String>,
}

impl Default for CsvImportOpts {
    fn default() -> Self {
        Self {
            has_header: true,
            delimiter: b',',
            null_token: None,
        }
    }
}

impl Db {
    /// Import rows from a CSV file into `table`. Writes are wrapped in a
    /// single transaction and rolled back on any error — the DB is either
    /// fully imported or unchanged.
    ///
    /// - With `has_header: true` (default), the first CSV row is the column
    ///   name list.
    /// - With `has_header: false`, columns are taken from the target table
    ///   in declared order.
    /// - All imported values are strings (SQLite's dynamic typing lets the
    ///   column affinity coerce). Use a typed JSON file or raw SQL for full
    ///   type control.
    pub fn import_csv(
        &self,
        path: &Path,
        table: &str,
        opts: CsvImportOpts,
    ) -> Result<ImportResult> {
        if self.is_read_only() {
            return Err(Error::ReadOnly);
        }

        let schema = self.schema(table)?;
        let file = File::open(path)?;
        let mut rdr = csv::ReaderBuilder::new()
            .has_headers(opts.has_header)
            .delimiter(opts.delimiter)
            .from_reader(file);

        let columns: Vec<String> = if opts.has_header {
            rdr.headers()
                .map_err(|e| Error::Invalid(format!("bad CSV header: {e}")))?
                .iter()
                .map(String::from)
                .collect()
        } else {
            schema.columns.iter().map(|c| c.name.clone()).collect()
        };

        if columns.is_empty() {
            return Err(Error::Invalid("no columns to import".into()));
        }

        let qcols = columns
            .iter()
            .map(|c| quote_ident(c))
            .collect::<Vec<_>>()
            .join(", ");
        let placeholders = (1..=columns.len())
            .map(|i| format!("?{i}"))
            .collect::<Vec<_>>()
            .join(", ");
        let sql = format!(
            "INSERT INTO {} ({qcols}) VALUES ({placeholders})",
            quote_ident(table)
        );

        let start = Instant::now();
        let conn = self.conn();
        conn.execute_batch("BEGIN IMMEDIATE")?;

        let mut stmt = conn.prepare(&sql).map_err(|e| {
            let _ = conn.execute_batch("ROLLBACK");
            Error::from(e)
        })?;
        let mut count: u64 = 0;
        for record in rdr.records() {
            let rec = match record {
                Ok(r) => r,
                Err(e) => {
                    drop(stmt);
                    let _ = conn.execute_batch("ROLLBACK");
                    return Err(Error::Invalid(format!("CSV parse error: {e}")));
                }
            };
            if rec.len() != columns.len() {
                drop(stmt);
                let _ = conn.execute_batch("ROLLBACK");
                return Err(Error::Invalid(format!(
                    "row has {} fields but expected {} (headers: {})",
                    rec.len(),
                    columns.len(),
                    columns.join(", ")
                )));
            }
            let params: Vec<Value> = rec
                .iter()
                .map(|s| match &opts.null_token {
                    Some(tok) if s == tok.as_str() => Value::Null,
                    _ => Value::Text(s.to_string()),
                })
                .collect();
            match stmt.execute(rusqlite::params_from_iter(params.iter())) {
                Ok(_) => count += 1,
                Err(e) => {
                    drop(stmt);
                    let _ = conn.execute_batch("ROLLBACK");
                    return Err(Error::from(e));
                }
            }
        }
        drop(stmt);
        conn.execute_batch("COMMIT")?;

        Ok(ImportResult {
            rows_inserted: count,
            elapsed_ms: start.elapsed().as_millis() as u64,
            columns,
        })
    }
}
