use serde::Serialize;

use crate::connection::Db;
use crate::error::Result;

#[derive(Debug, Clone, Serialize)]
pub struct TableStat {
    pub name: String,
    pub row_count: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct DbStats {
    pub path: String,
    pub size_bytes: u64,
    pub page_size: i64,
    pub page_count: i64,
    pub freelist_count: i64,
    pub tables: Vec<TableStat>,
}

impl Db {
    pub fn stats(&self) -> Result<DbStats> {
        let meta = self.meta()?;
        let freelist_count: i64 =
            self.conn().query_row("PRAGMA freelist_count", [], |r| r.get(0))?;

        let mut tables = Vec::new();
        for t in self.tables()? {
            tables.push(TableStat {
                row_count: t.row_count.unwrap_or(0),
                name: t.name,
            });
        }

        Ok(DbStats {
            path: meta.path,
            size_bytes: meta.size_bytes,
            page_size: meta.page_size,
            page_count: meta.page_count,
            freelist_count,
            tables,
        })
    }
}
