//! SQLite housekeeping operations surfaced to the desktop + CLI.
//!
//! All five require a read-write connection. `integrity_check` and
//! `wal_checkpoint` return structured output; the others return a single
//! "done" line with timing. Errors propagate via `sqlv_core::Error`.

use std::time::Instant;

use serde::Serialize;

use crate::connection::{quote_ident, Db};
use crate::error::{Error, Result};

#[derive(Debug, Clone, Serialize)]
pub struct MaintenanceResult {
    pub task: String,
    pub output: Vec<String>,
    pub elapsed_ms: u64,
}

impl Db {
    /// `VACUUM` — rewrites the DB file, reclaiming unused pages and
    /// restoring row ordering. Expensive on large files.
    pub fn vacuum(&self) -> Result<MaintenanceResult> {
        self.run_maintenance_batch("vacuum", "VACUUM;")
    }

    /// `REINDEX [name]` — rebuilds either every index (`None`) or indexes
    /// backing a specific table/index (`Some(name)`).
    pub fn reindex(&self, target: Option<&str>) -> Result<MaintenanceResult> {
        let sql = match target {
            Some(t) => format!("REINDEX {};", quote_ident(t)),
            None => "REINDEX;".into(),
        };
        self.run_maintenance_batch("reindex", &sql)
    }

    /// `ANALYZE [name]` — updates the query planner's statistics.
    pub fn analyze(&self, target: Option<&str>) -> Result<MaintenanceResult> {
        let sql = match target {
            Some(t) => format!("ANALYZE {};", quote_ident(t)),
            None => "ANALYZE;".into(),
        };
        self.run_maintenance_batch("analyze", &sql)
    }

    /// `PRAGMA integrity_check` — returns `["ok"]` on a clean DB, or a
    /// list of human-readable error strings.
    pub fn integrity_check(&self) -> Result<MaintenanceResult> {
        if self.is_read_only() {
            // Integrity check doesn't strictly require RW, but other
            // maintenance does and we want consistent semantics.
        }
        let start = Instant::now();
        let mut stmt = self.conn().prepare("PRAGMA integrity_check")?;
        let rows = stmt.query_map([], |r| r.get::<_, String>(0))?;
        let output: Vec<String> = rows.collect::<std::result::Result<_, _>>()?;
        Ok(MaintenanceResult {
            task: "integrity_check".into(),
            output,
            elapsed_ms: start.elapsed().as_millis() as u64,
        })
    }

    /// `PRAGMA wal_checkpoint(MODE)` — flushes the `-wal` file back into
    /// the main DB. Mode is `PASSIVE | FULL | RESTART | TRUNCATE`.
    /// Returns `["busy=<N>", "log=<N>", "checkpointed=<N>"]`.
    pub fn wal_checkpoint(&self, mode: &str) -> Result<MaintenanceResult> {
        if self.is_read_only() {
            return Err(Error::ReadOnly);
        }
        let m = mode.to_ascii_uppercase();
        if !matches!(m.as_str(), "PASSIVE" | "FULL" | "RESTART" | "TRUNCATE") {
            return Err(Error::Invalid(format!(
                "wal_checkpoint mode must be PASSIVE / FULL / RESTART / TRUNCATE, got {mode:?}"
            )));
        }
        let start = Instant::now();
        let sql = format!("PRAGMA wal_checkpoint({m})");
        let mut stmt = self.conn().prepare(&sql)?;
        let rows = stmt.query_map([], |r| {
            Ok((
                r.get::<_, i64>(0)?, // busy (1 if blocked)
                r.get::<_, i64>(1)?, // total frames in the log
                r.get::<_, i64>(2)?, // frames checkpointed
            ))
        })?;
        let mut output = Vec::new();
        for row in rows {
            let (busy, log, ckpt) = row?;
            output.push(format!("busy={busy}"));
            output.push(format!("log={log}"));
            output.push(format!("checkpointed={ckpt}"));
        }
        Ok(MaintenanceResult {
            task: format!("wal_checkpoint({m})"),
            output,
            elapsed_ms: start.elapsed().as_millis() as u64,
        })
    }

    fn run_maintenance_batch(&self, task: &str, sql: &str) -> Result<MaintenanceResult> {
        if self.is_read_only() {
            return Err(Error::ReadOnly);
        }
        let start = Instant::now();
        self.conn().execute_batch(sql)?;
        Ok(MaintenanceResult {
            task: task.into(),
            output: vec!["done".into()],
            elapsed_ms: start.elapsed().as_millis() as u64,
        })
    }
}
