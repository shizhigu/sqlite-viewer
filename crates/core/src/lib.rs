//! sqlv-core: shared SQLite operations for the desktop app and CLI.
//!
//! All types returned by this crate are `serde::Serialize` so the same shapes
//! flow through Tauri `invoke` responses and the CLI's `--json` output.

mod connection;
mod dump;
mod error;
mod import;
mod meta;
mod pragma;
mod query;
mod schema;
mod stats;
mod value;

pub use connection::{Db, OpenOpts};
pub use dump::DumpFilter;
pub use error::{Error, Result};
pub use import::{CsvImportOpts, ImportResult};
pub use meta::DbMeta;
pub use pragma::PragmaValue;
pub use query::{ExecResult, Page, QueryResult};
pub use schema::{Column, ForeignKey, IndexInfo, TableInfo, TableKind, TableSchema, ViewInfo};
pub use stats::{DbStats, TableStat};
pub use value::Value;
