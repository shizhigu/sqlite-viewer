use std::path::{Path, PathBuf};
use std::time::Duration;

use rusqlite::{Connection, OpenFlags};
use serde::{Deserialize, Serialize};

use crate::error::Result;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenOpts {
    /// Open the file with SQLITE_OPEN_READ_ONLY. Default `true` — writes must
    /// be explicitly opted in.
    pub read_only: bool,
    /// Busy timeout in milliseconds. `None` disables (uses sqlite default of 0).
    pub timeout_ms: Option<u64>,
}

impl Default for OpenOpts {
    fn default() -> Self {
        Self {
            read_only: true,
            timeout_ms: Some(5_000),
        }
    }
}

pub struct Db {
    conn: Connection,
    path: PathBuf,
    read_only: bool,
}

impl Db {
    /// Hand back a cancellation handle tied to this connection. Calling
    /// [`CancelHandle::cancel`] from another thread makes the currently
    /// running query return `Error::Sqlite(SqliteFailure(… INTERRUPT …))`.
    ///
    /// The handle can be cloned and stored in app state; interrupting a
    /// non-running connection is a no-op so cross-thread races are safe.
    pub fn cancel_handle(&self) -> CancelHandle {
        CancelHandle {
            inner: std::sync::Arc::new(self.conn.get_interrupt_handle()),
        }
    }
}

/// Thread-safe cancellation handle for a running query on a [`Db`].
#[derive(Clone)]
pub struct CancelHandle {
    inner: std::sync::Arc<rusqlite::InterruptHandle>,
}

impl CancelHandle {
    /// Signal the underlying SQLite engine to abort the in-flight statement.
    /// Returns once the interrupt has been delivered — the running call on
    /// the other thread will unwind with `Error::Sqlite` shortly after.
    pub fn cancel(&self) {
        self.inner.interrupt();
    }
}

impl std::fmt::Debug for Db {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Db")
            .field("path", &self.path)
            .field("read_only", &self.read_only)
            .finish()
    }
}

impl Db {
    pub fn open(path: &Path, opts: OpenOpts) -> Result<Self> {
        let mut flags = if opts.read_only {
            OpenFlags::SQLITE_OPEN_READ_ONLY
        } else {
            OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_CREATE
        };
        flags |= OpenFlags::SQLITE_OPEN_NO_MUTEX | OpenFlags::SQLITE_OPEN_URI;

        let conn = Connection::open_with_flags(path, flags)?;
        if let Some(ms) = opts.timeout_ms {
            conn.busy_timeout(Duration::from_millis(ms))?;
        }
        // Enforce FK constraints consistently across desktop + CLI.
        conn.pragma_update(None, "foreign_keys", "ON")?;
        // For read-write connections, switch to WAL mode. This lets readers
        // and writers proceed concurrently (eliminates most SQLITE_BUSY
        // errors) and is cheap on small DBs. Read-only connections can't
        // change journal mode, so we only do this when we own the writer.
        if !opts.read_only {
            // `execute_batch` instead of `pragma_update` — `journal_mode`
            // returns the resulting mode as a row, which `pragma_update`
            // refuses to ignore.
            let _ = conn.execute_batch("PRAGMA journal_mode=WAL;");
        }

        Ok(Self {
            conn,
            path: path.to_path_buf(),
            read_only: opts.read_only,
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn is_read_only(&self) -> bool {
        self.read_only
    }

    pub(crate) fn conn(&self) -> &Connection {
        &self.conn
    }
}

/// Quote a SQLite identifier (table/column name) with double-quotes, escaping
/// any embedded double-quotes. Use this anywhere you need to splice a name
/// into SQL; the user-facing name is already validated by SQLite at introspection
/// time, but we still quote defensively.
pub(crate) fn quote_ident(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for ch in s.chars() {
        if ch == '"' {
            out.push('"');
            out.push('"');
        } else {
            out.push(ch);
        }
    }
    out.push('"');
    out
}
