//! Persistent activity log — a small SQLite database that records every
//! query / exec / open / cancel event across the CLI, desktop, and MCP
//! surfaces so the user can grep back over the whole collaboration later.
//!
//! Default location: `~/.sqlv/activity.db`. Each record is append-only
//! (no UPDATE / DELETE surface). The log auto-creates and self-migrates
//! on open — cheap, fire-and-forget from the caller's side.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::{params, Connection};
use serde::Serialize;

use crate::error::{Error, Result};

pub struct ActivityLog {
    conn: Connection,
}

#[derive(Debug, Clone)]
pub struct ActivityEntry {
    pub ts_ms: i64,
    pub source: String,
    pub kind: String,
    pub sql: Option<String>,
    pub db_path: Option<String>,
    pub elapsed_ms: Option<i64>,
    pub rows: Option<i64>,
    pub error_code: Option<String>,
    pub error_message: Option<String>,
}

impl ActivityEntry {
    /// Build an entry with `ts_ms` set to now.
    pub fn now(source: impl Into<String>, kind: impl Into<String>) -> Self {
        Self {
            ts_ms: now_ms(),
            source: source.into(),
            kind: kind.into(),
            sql: None,
            db_path: None,
            elapsed_ms: None,
            rows: None,
            error_code: None,
            error_message: None,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ActivityRecord {
    pub id: i64,
    pub ts_ms: i64,
    pub source: String,
    pub kind: String,
    pub sql: Option<String>,
    pub db_path: Option<String>,
    pub elapsed_ms: Option<i64>,
    pub rows: Option<i64>,
    pub error_code: Option<String>,
    pub error_message: Option<String>,
}

/// Filters for [`ActivityLog::query`]. All fields default to "unfiltered".
#[derive(Debug, Clone, Default)]
pub struct ActivityQuery {
    /// Case-insensitive substring match on `sql` and `db_path`.
    pub grep: Option<String>,
    /// Only rows at or after this unix-ms timestamp.
    pub since_ms: Option<i64>,
    /// Restrict to one DB path.
    pub db_path: Option<String>,
    /// Restrict to a single source tag (`"ui"` / `"agent"` / `"cli"`).
    pub source: Option<String>,
    /// Max rows to return. 0 = unbounded (dangerous on huge logs).
    pub limit: u32,
}

impl ActivityLog {
    /// Open the default log at `~/.sqlv/activity.db`. Creates the file
    /// and parent dir if absent, runs the migration to latest schema.
    pub fn open_default() -> Result<Self> {
        Self::open_at(&default_log_path()?)
    }

    pub fn open_at(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(path)?;
        conn.busy_timeout(std::time::Duration::from_millis(2_000))?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        Self::migrate(&conn)?;
        Ok(Self { conn })
    }

    fn migrate(conn: &Connection) -> Result<()> {
        // Single migration for v1. If the schema evolves, gate further
        // ones on `PRAGMA user_version`.
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS events (
                 id             INTEGER PRIMARY KEY,
                 ts_ms          INTEGER NOT NULL,
                 source         TEXT NOT NULL,
                 kind           TEXT NOT NULL,
                 sql            TEXT,
                 db_path        TEXT,
                 elapsed_ms     INTEGER,
                 rows           INTEGER,
                 error_code     TEXT,
                 error_message  TEXT
             );
             CREATE INDEX IF NOT EXISTS idx_events_ts ON events(ts_ms DESC);
             CREATE INDEX IF NOT EXISTS idx_events_source ON events(source);
             CREATE INDEX IF NOT EXISTS idx_events_db ON events(db_path);",
        )?;
        Ok(())
    }

    /// Insert a new event, return the assigned id.
    pub fn append(&self, e: &ActivityEntry) -> Result<i64> {
        self.conn.execute(
            "INSERT INTO events
                (ts_ms, source, kind, sql, db_path, elapsed_ms, rows, error_code, error_message)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                e.ts_ms,
                e.source,
                e.kind,
                e.sql,
                e.db_path,
                e.elapsed_ms,
                e.rows,
                e.error_code,
                e.error_message,
            ],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    /// Query recent events newest-first, filtered by `q`.
    pub fn query(&self, q: &ActivityQuery) -> Result<Vec<ActivityRecord>> {
        let mut sql = String::from(
            "SELECT id, ts_ms, source, kind, sql, db_path, \
                    elapsed_ms, rows, error_code, error_message \
             FROM events WHERE 1=1",
        );
        let mut binds: Vec<String> = Vec::new();

        if let Some(g) = &q.grep {
            sql.push_str(" AND (sql LIKE ?1 OR db_path LIKE ?1)");
            let pat = format!("%{}%", g.to_lowercase());
            binds.push(pat);
        }
        if let Some(s) = q.since_ms {
            sql.push_str(&format!(" AND ts_ms >= ?{}", binds.len() + 1));
            binds.push(s.to_string());
        }
        if let Some(p) = &q.db_path {
            sql.push_str(&format!(" AND db_path = ?{}", binds.len() + 1));
            binds.push(p.clone());
        }
        if let Some(src) = &q.source {
            sql.push_str(&format!(" AND source = ?{}", binds.len() + 1));
            binds.push(src.clone());
        }
        sql.push_str(" ORDER BY ts_ms DESC");
        if q.limit > 0 {
            sql.push_str(&format!(" LIMIT {}", q.limit));
        }

        let mut stmt = self.conn.prepare(&sql)?;
        let params: Vec<&dyn rusqlite::ToSql> =
            binds.iter().map(|s| s as &dyn rusqlite::ToSql).collect();
        let rows = stmt.query_map(params.as_slice(), |r| {
            Ok(ActivityRecord {
                id: r.get(0)?,
                ts_ms: r.get(1)?,
                source: r.get(2)?,
                kind: r.get(3)?,
                sql: r.get(4)?,
                db_path: r.get(5)?,
                elapsed_ms: r.get(6)?,
                rows: r.get(7)?,
                error_code: r.get(8)?,
                error_message: r.get(9)?,
            })
        })?;
        Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
    }

    /// Delete all events older than `cutoff_ms`. Returns rows affected.
    pub fn prune_before(&self, cutoff_ms: i64) -> Result<u64> {
        let n = self
            .conn
            .execute("DELETE FROM events WHERE ts_ms < ?1", params![cutoff_ms])?;
        Ok(n as u64)
    }
}

pub fn default_log_path() -> Result<PathBuf> {
    let home = dirs::home_dir().ok_or_else(|| {
        Error::Invalid("could not determine home directory for activity log".into())
    })?;
    Ok(home.join(".sqlv").join("activity.db"))
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}
