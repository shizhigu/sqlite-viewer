use serde::{Deserialize, Serialize};

use crate::connection::Db;
use crate::error::{Error, Result};
use crate::value::Value;

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Page {
    pub limit: u32,
    pub offset: u32,
}

impl Default for Page {
    fn default() -> Self {
        // Sensible ceiling for agent-driven queries; callers can override.
        Self { limit: 1000, offset: 0 }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct QueryResult {
    pub columns: Vec<String>,
    pub column_types: Vec<Option<String>>,
    pub rows: Vec<Vec<Value>>,
    /// True if the underlying cursor had more rows than `page.limit` allowed.
    pub truncated: bool,
    pub elapsed_ms: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct ExecResult {
    pub rows_affected: u64,
    pub last_insert_rowid: i64,
    pub elapsed_ms: u64,
}

impl Db {
    pub fn query(&self, sql: &str, params: &[Value], page: Page) -> Result<QueryResult> {
        let start = std::time::Instant::now();

        let mut stmt = self.conn().prepare(sql)?;

        let column_names: Vec<String> =
            stmt.column_names().into_iter().map(String::from).collect();

        let column_types: Vec<Option<String>> = (0..stmt.column_count())
            .map(|i| {
                stmt.columns()
                    .get(i)
                    .and_then(|c| c.decl_type())
                    .map(|s| s.to_string())
            })
            .collect();

        let col_count = column_names.len();
        let mut rows = stmt.query(rusqlite::params_from_iter(params.iter()))?;

        let mut out: Vec<Vec<Value>> = Vec::new();
        let mut seen: u64 = 0;
        let mut truncated = false;

        while let Some(row) = rows.next()? {
            if (seen as u32) < page.offset {
                seen += 1;
                continue;
            }
            if out.len() as u32 >= page.limit {
                truncated = true;
                break;
            }
            let mut r = Vec::with_capacity(col_count);
            for i in 0..col_count {
                r.push(Value::from(row.get_ref(i)?));
            }
            out.push(r);
            seen += 1;
        }

        Ok(QueryResult {
            columns: column_names,
            column_types,
            rows: out,
            truncated,
            elapsed_ms: start.elapsed().as_millis() as u64,
        })
    }

    pub fn exec(&self, sql: &str, params: &[Value]) -> Result<ExecResult> {
        if self.is_read_only() {
            return Err(Error::ReadOnly);
        }
        let start = std::time::Instant::now();
        let affected = self
            .conn()
            .execute(sql, rusqlite::params_from_iter(params.iter()))?;
        Ok(ExecResult {
            rows_affected: affected as u64,
            last_insert_rowid: self.conn().last_insert_rowid(),
            elapsed_ms: start.elapsed().as_millis() as u64,
        })
    }

    /// Execute a batch of statements inside a single SQLite transaction.
    ///
    /// All or nothing: the first failing statement aborts the batch and the
    /// whole transaction rolls back, even for statements that already
    /// succeeded. The returned `ExecResult.rows_affected` is the sum of
    /// per-statement `changes()` across the batch; `last_insert_rowid` is
    /// the value after the final INSERT-shaped statement (0 if none).
    ///
    /// Used by the desktop's "Delete selected rows" button and by any agent
    /// workflow where half-applied writes would leave the DB inconsistent.
    pub fn exec_many(&self, statements: &[(&str, &[Value])]) -> Result<ExecResult> {
        if self.is_read_only() {
            return Err(Error::ReadOnly);
        }
        let start = std::time::Instant::now();
        let conn = self.conn();

        conn.execute_batch("BEGIN IMMEDIATE")?;

        let mut total_affected: u64 = 0;
        let mut last_rowid: i64 = 0;

        for (sql, params) in statements {
            let result = conn.execute(sql, rusqlite::params_from_iter(params.iter()));
            match result {
                Ok(n) => {
                    total_affected += n as u64;
                    let rid = conn.last_insert_rowid();
                    if rid != 0 {
                        last_rowid = rid;
                    }
                }
                Err(e) => {
                    // Rollback on any failure — don't leave the DB
                    // half-mutated. Swallow the rollback error (there's
                    // nothing useful to do with it) and surface the
                    // original failure.
                    let _ = conn.execute_batch("ROLLBACK");
                    return Err(Error::from(e));
                }
            }
        }

        conn.execute_batch("COMMIT")?;

        Ok(ExecResult {
            rows_affected: total_affected,
            last_insert_rowid: last_rowid,
            elapsed_ms: start.elapsed().as_millis() as u64,
        })
    }
}
