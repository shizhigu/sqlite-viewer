//! Schema diff — compare two SQLite databases structurally.
//!
//! Two axes of change per table: *columns* (added / removed / type-changed)
//! and *indexes* (added / removed). We keep it deliberately conservative:
//!   - Column *order* is not compared (SQLite ALTER TABLE ADD COLUMN only
//!     appends, but users regularly rebuild tables so an order diff would
//!     be noisy false-positives).
//!   - `decl_type` is compared case-insensitively — SQLite stores what
//!     you typed, but the affinity is case-insensitive.
//!
//! Designed for `sqlv diff --a X.db --b Y.db` and future GUI use.

use std::collections::BTreeMap;

use serde::Serialize;

use crate::connection::Db;
use crate::error::Result;
use crate::schema::{Column, IndexInfo};

#[derive(Debug, Clone, Serialize)]
pub struct DiffReport {
    pub only_in_a: Vec<String>,
    pub only_in_b: Vec<String>,
    pub changed: Vec<TableDiff>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TableDiff {
    pub name: String,
    pub columns_added: Vec<Column>,
    pub columns_removed: Vec<Column>,
    pub columns_changed: Vec<ColumnChange>,
    pub indexes_added: Vec<IndexInfo>,
    pub indexes_removed: Vec<IndexInfo>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ColumnChange {
    pub name: String,
    pub before: Column,
    pub after: Column,
    pub reasons: Vec<String>,
}

/// Diff `a` against `b`. Tables are matched by name; unmatched tables
/// go into `only_in_a` / `only_in_b`. For matched tables, `changed`
/// records column and index deltas; unchanged tables are omitted.
pub fn diff_schemas(a: &Db, b: &Db) -> Result<DiffReport> {
    let a_tables: BTreeMap<String, ()> = a.tables()?.into_iter().map(|t| (t.name, ())).collect();
    let b_tables: BTreeMap<String, ()> = b.tables()?.into_iter().map(|t| (t.name, ())).collect();

    let only_in_a: Vec<String> = a_tables
        .keys()
        .filter(|n| !b_tables.contains_key(*n))
        .cloned()
        .collect();
    let only_in_b: Vec<String> = b_tables
        .keys()
        .filter(|n| !a_tables.contains_key(*n))
        .cloned()
        .collect();

    let mut changed = Vec::new();
    for name in a_tables.keys() {
        if !b_tables.contains_key(name) {
            continue;
        }
        let sa = a.schema(name)?;
        let sb = b.schema(name)?;
        let td = diff_table(name, &sa.columns, &sb.columns, &sa.indexes, &sb.indexes);
        if !td.is_empty() {
            changed.push(td);
        }
    }

    Ok(DiffReport {
        only_in_a,
        only_in_b,
        changed,
    })
}

fn diff_table(
    name: &str,
    a_cols: &[Column],
    b_cols: &[Column],
    a_idx: &[IndexInfo],
    b_idx: &[IndexInfo],
) -> TableDiff {
    let a_by_name: BTreeMap<&str, &Column> = a_cols.iter().map(|c| (c.name.as_str(), c)).collect();
    let b_by_name: BTreeMap<&str, &Column> = b_cols.iter().map(|c| (c.name.as_str(), c)).collect();

    let mut columns_added = Vec::new();
    let mut columns_removed = Vec::new();
    let mut columns_changed = Vec::new();

    for (n, col) in &b_by_name {
        if !a_by_name.contains_key(n) {
            columns_added.push((*col).clone());
        }
    }
    for (n, col) in &a_by_name {
        if !b_by_name.contains_key(n) {
            columns_removed.push((*col).clone());
            continue;
        }
        let after = b_by_name[n];
        let reasons = column_differences(col, after);
        if !reasons.is_empty() {
            columns_changed.push(ColumnChange {
                name: (*n).into(),
                before: (*col).clone(),
                after: after.clone(),
                reasons,
            });
        }
    }

    let a_idx_by_name: BTreeMap<&str, &IndexInfo> =
        a_idx.iter().map(|i| (i.name.as_str(), i)).collect();
    let b_idx_by_name: BTreeMap<&str, &IndexInfo> =
        b_idx.iter().map(|i| (i.name.as_str(), i)).collect();

    let indexes_added: Vec<IndexInfo> = b_idx_by_name
        .iter()
        .filter(|(n, _)| !a_idx_by_name.contains_key(*n))
        .map(|(_, v)| (*v).clone())
        .collect();
    let indexes_removed: Vec<IndexInfo> = a_idx_by_name
        .iter()
        .filter(|(n, _)| !b_idx_by_name.contains_key(*n))
        .map(|(_, v)| (*v).clone())
        .collect();

    TableDiff {
        name: name.into(),
        columns_added,
        columns_removed,
        columns_changed,
        indexes_added,
        indexes_removed,
    }
}

fn column_differences(before: &Column, after: &Column) -> Vec<String> {
    let mut out = Vec::new();
    if norm(&before.decl_type) != norm(&after.decl_type) {
        out.push(format!(
            "type: {:?} → {:?}",
            before.decl_type.as_deref().unwrap_or(""),
            after.decl_type.as_deref().unwrap_or(""),
        ));
    }
    if before.not_null != after.not_null {
        out.push(format!(
            "not_null: {} → {}",
            before.not_null, after.not_null
        ));
    }
    if before.default_value != after.default_value {
        out.push("default changed".into());
    }
    if before.pk != after.pk {
        out.push(format!("pk: {} → {}", before.pk, after.pk));
    }
    if before.hidden != after.hidden {
        out.push(format!("hidden: {} → {}", before.hidden, after.hidden));
    }
    out
}

fn norm(s: &Option<String>) -> Option<String> {
    s.as_deref().map(|v| v.to_ascii_uppercase())
}

impl TableDiff {
    fn is_empty(&self) -> bool {
        self.columns_added.is_empty()
            && self.columns_removed.is_empty()
            && self.columns_changed.is_empty()
            && self.indexes_added.is_empty()
            && self.indexes_removed.is_empty()
    }
}
