//! sqlv-core: shared SQLite operations for the desktop app and CLI.
//!
//! All types returned by this crate are `serde::Serialize` so the same shapes
//! flow through Tauri `invoke` responses and the CLI's `--json` output.

mod classify;
mod connection;
mod dump;
mod error;
mod import;
mod maintenance;
mod meta;
mod pragma;
mod query;
mod schema;
mod stats;
mod value;

pub use classify::{classify as classify_sql, SqlKind};
pub use connection::{CancelHandle, Db, OpenOpts};
pub use dump::DumpFilter;
pub use error::{Error, Result};
pub use import::{guess_json_format, CsvImportOpts, ImportResult, JsonFormat};
pub use maintenance::MaintenanceResult;
pub use meta::DbMeta;
pub use pragma::PragmaValue;
pub use query::{ExecResult, Page, QueryResult};
pub use schema::{
    Column, ForeignKey, IndexInfo, SchemaInfo, TableInfo, TableKind, TableSchema, TriggerInfo,
    ViewInfo,
};
pub use stats::{DbStats, TableStat};
pub use value::{Value, BLOB_PREVIEW_BYTES, JSON_SAFE_INTEGER_MAX};
