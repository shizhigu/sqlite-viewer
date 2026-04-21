use rusqlite::OptionalExtension;
use serde::{Deserialize, Serialize};

use crate::connection::{quote_ident, Db};
use crate::error::{Error, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TableKind {
    Table,
    View,
}

#[derive(Debug, Clone, Serialize)]
pub struct TableInfo {
    pub name: String,
    pub kind: TableKind,
    pub row_count: Option<u64>,
    pub sql: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ViewInfo {
    pub name: String,
    pub sql: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Column {
    pub cid: i32,
    pub name: String,
    pub decl_type: Option<String>,
    pub not_null: bool,
    pub default_value: Option<String>,
    /// 0 if not part of the primary key, otherwise the 1-based position.
    pub pk: i32,
}

#[derive(Debug, Clone, Serialize)]
pub struct ForeignKey {
    pub id: i32,
    pub seq: i32,
    pub table: String,
    pub from: String,
    pub to: String,
    pub on_update: String,
    pub on_delete: String,
    #[serde(rename = "match")]
    pub match_: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct IndexInfo {
    pub name: String,
    pub table: String,
    pub unique: bool,
    /// "c" for CREATE INDEX, "u" for UNIQUE constraint, "pk" for primary key.
    pub origin: String,
    pub partial: bool,
    pub columns: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TableSchema {
    pub name: String,
    pub kind: TableKind,
    pub columns: Vec<Column>,
    pub foreign_keys: Vec<ForeignKey>,
    pub indexes: Vec<IndexInfo>,
    pub sql: Option<String>,
}

impl Db {
    pub fn tables(&self) -> Result<Vec<TableInfo>> {
        let mut stmt = self.conn().prepare(
            "SELECT name, sql FROM sqlite_master \
             WHERE type='table' AND name NOT LIKE 'sqlite_%' \
             ORDER BY name",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, Option<String>>(1)?))
        })?;
        let mut out = Vec::new();
        for row in rows {
            let (name, sql) = row?;
            let row_count = self.table_row_count(&name).ok();
            out.push(TableInfo {
                name,
                kind: TableKind::Table,
                row_count,
                sql,
            });
        }
        Ok(out)
    }

    pub fn views(&self) -> Result<Vec<ViewInfo>> {
        let mut stmt = self
            .conn()
            .prepare("SELECT name, sql FROM sqlite_master WHERE type='view' ORDER BY name")?;
        let rows = stmt.query_map([], |r| {
            Ok(ViewInfo {
                name: r.get(0)?,
                sql: r.get(1)?,
            })
        })?;
        Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
    }

    /// List indexes. If `table` is `Some(name)`, restrict to that table.
    pub fn indexes(&self, table: Option<&str>) -> Result<Vec<IndexInfo>> {
        let target_tables: Vec<String> = if let Some(t) = table {
            self.assert_table_exists(t)?;
            vec![t.to_string()]
        } else {
            self.tables()?.into_iter().map(|t| t.name).collect()
        };

        let mut out = Vec::new();
        for t in target_tables {
            out.extend(self.indexes_for(&t)?);
        }
        Ok(out)
    }

    pub fn schema(&self, table: &str) -> Result<TableSchema> {
        let (kind, sql) = self.lookup_object(table)?;
        let columns = self.table_info(table)?;
        let foreign_keys = if matches!(kind, TableKind::Table) {
            self.foreign_keys_for(table)?
        } else {
            Vec::new()
        };
        let indexes = if matches!(kind, TableKind::Table) {
            self.indexes_for(table)?
        } else {
            Vec::new()
        };
        Ok(TableSchema {
            name: table.to_string(),
            kind,
            columns,
            foreign_keys,
            indexes,
            sql,
        })
    }

    // ----- helpers -----

    fn assert_table_exists(&self, name: &str) -> Result<()> {
        let exists: Option<String> = self
            .conn()
            .query_row(
                "SELECT name FROM sqlite_master WHERE type IN ('table','view') AND name=?1",
                [name],
                |r| r.get(0),
            )
            .optional()?;
        if exists.is_none() {
            return Err(Error::NotFound(format!("table or view '{name}'")));
        }
        Ok(())
    }

    fn lookup_object(&self, name: &str) -> Result<(TableKind, Option<String>)> {
        let row: Option<(String, Option<String>)> = self
            .conn()
            .query_row(
                "SELECT type, sql FROM sqlite_master WHERE name=?1 AND type IN ('table','view')",
                [name],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;
        match row {
            Some((t, sql)) if t == "table" => Ok((TableKind::Table, sql)),
            Some((t, sql)) if t == "view" => Ok((TableKind::View, sql)),
            _ => Err(Error::NotFound(format!("table or view '{name}'"))),
        }
    }

    fn table_row_count(&self, table: &str) -> Result<u64> {
        let sql = format!("SELECT COUNT(*) FROM {}", quote_ident(table));
        let count: i64 = self.conn().query_row(&sql, [], |r| r.get(0))?;
        Ok(count.max(0) as u64)
    }

    fn table_info(&self, table: &str) -> Result<Vec<Column>> {
        let sql = format!("PRAGMA table_info({})", quote_ident(table));
        let mut stmt = self.conn().prepare(&sql)?;
        let rows = stmt.query_map([], |r| {
            Ok(Column {
                cid: r.get(0)?,
                name: r.get(1)?,
                decl_type: r.get::<_, Option<String>>(2)?.filter(|s| !s.is_empty()),
                not_null: r.get::<_, i32>(3)? != 0,
                default_value: r.get::<_, Option<String>>(4)?,
                pk: r.get(5)?,
            })
        })?;
        Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
    }

    fn foreign_keys_for(&self, table: &str) -> Result<Vec<ForeignKey>> {
        let sql = format!("PRAGMA foreign_key_list({})", quote_ident(table));
        let mut stmt = self.conn().prepare(&sql)?;
        let rows = stmt.query_map([], |r| {
            Ok(ForeignKey {
                id: r.get(0)?,
                seq: r.get(1)?,
                table: r.get(2)?,
                from: r.get(3)?,
                to: r.get(4)?,
                on_update: r.get(5)?,
                on_delete: r.get(6)?,
                match_: r.get(7)?,
            })
        })?;
        Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
    }

    fn indexes_for(&self, table: &str) -> Result<Vec<IndexInfo>> {
        let list_sql = format!("PRAGMA index_list({})", quote_ident(table));
        let mut stmt = self.conn().prepare(&list_sql)?;
        let index_rows = stmt
            .query_map([], |r| {
                Ok((
                    r.get::<_, i32>(0)?,      // seq
                    r.get::<_, String>(1)?,   // name
                    r.get::<_, i32>(2)? != 0, // unique
                    r.get::<_, String>(3)?,   // origin
                    r.get::<_, i32>(4)? != 0, // partial
                ))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        let mut out = Vec::with_capacity(index_rows.len());
        for (_seq, name, unique, origin, partial) in index_rows {
            let info_sql = format!("PRAGMA index_info({})", quote_ident(&name));
            let mut info_stmt = self.conn().prepare(&info_sql)?;
            let cols: Vec<String> = info_stmt
                .query_map([], |r| r.get::<_, Option<String>>(2))?
                .filter_map(|r| r.ok().flatten())
                .collect();
            out.push(IndexInfo {
                name,
                table: table.to_string(),
                unique,
                origin,
                partial,
                columns: cols,
            });
        }
        Ok(out)
    }
}
