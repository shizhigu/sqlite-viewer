use serde::Serialize;

use crate::connection::Db;
use crate::error::Result;

#[derive(Debug, Clone, Serialize)]
pub struct DbMeta {
    pub path: String,
    pub size_bytes: u64,
    pub page_size: i64,
    pub page_count: i64,
    pub encoding: String,
    pub user_version: i64,
    pub application_id: i64,
    pub journal_mode: String,
    pub sqlite_library_version: String,
    pub read_only: bool,
}

impl Db {
    pub fn meta(&self) -> Result<DbMeta> {
        let size_bytes = std::fs::metadata(self.path()).map(|m| m.len()).unwrap_or(0);
        let page_size: i64 = self
            .conn()
            .query_row("PRAGMA page_size", [], |r| r.get(0))?;
        let page_count: i64 = self
            .conn()
            .query_row("PRAGMA page_count", [], |r| r.get(0))?;
        let encoding: String = self.conn().query_row("PRAGMA encoding", [], |r| r.get(0))?;
        let user_version: i64 = self
            .conn()
            .query_row("PRAGMA user_version", [], |r| r.get(0))?;
        let application_id: i64 = self
            .conn()
            .query_row("PRAGMA application_id", [], |r| r.get(0))?;
        let journal_mode: String = self
            .conn()
            .query_row("PRAGMA journal_mode", [], |r| r.get(0))?;
        let sqlite_library_version: String =
            self.conn()
                .query_row("SELECT sqlite_version()", [], |r| r.get(0))?;

        Ok(DbMeta {
            path: self.path().display().to_string(),
            size_bytes,
            page_size,
            page_count,
            encoding,
            user_version,
            application_id,
            journal_mode,
            sqlite_library_version,
            read_only: self.is_read_only(),
        })
    }
}
