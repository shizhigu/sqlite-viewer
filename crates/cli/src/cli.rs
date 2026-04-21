use clap::{Args, Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(
    name = "sqlv",
    version,
    about = "Agent-friendly SQLite client (JSON-first, read-only default)"
)]
pub struct Cli {
    /// Force JSON output (the default when stdout is not a TTY).
    #[arg(long, global = true)]
    pub json: bool,

    /// Suppress non-error output (applies only to pretty mode).
    #[arg(long, global = true)]
    pub quiet: bool,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Validate and print database metadata (page size, encoding, version, ...).
    Open(DbArgs),
    /// List user tables with row counts.
    Tables(DbArgs),
    /// List views.
    Views(DbArgs),
    /// List indexes, optionally restricted to a single table.
    Indexes(IndexesArgs),
    /// Describe the schema of a table/view. Omit the arg to describe all.
    Schema(SchemaArgs),
    /// Execute a read-only SQL query and print results.
    Query(QueryArgs),
    /// Execute a mutating SQL statement. Requires `--write`.
    Exec(ExecArgs),
    /// Database-level statistics (size, page counts, per-table row counts).
    Stats(DbArgs),
    /// Read or set a PRAGMA value.
    Pragma(PragmaArgs),
    /// Dump schema and/or data as SQL to stdout.
    Dump(DumpArgs),
    /// Send a SQL query to the running desktop app (live-mirrors in its UI).
    Push(PushArgs),
    /// Ask the running desktop app to open a database file.
    #[command(name = "push-open")]
    PushOpen(PushOpenArgs),
    /// Bulk-load a CSV / JSON / JSONL file into a table (transactional).
    Import(ImportArgs),
    /// Database housekeeping: VACUUM / REINDEX / ANALYZE / integrity-check /
    /// wal-checkpoint. Most require `--write`; integrity-check doesn't.
    Maintenance(MaintenanceArgs),
    /// Snapshot the DB to a new file via `VACUUM INTO`. Useful before
    /// agent-driven mutations.
    Checkpoint(CheckpointArgs),
    /// Search the cross-surface activity log at `~/.sqlv/activity.db`.
    History(HistoryArgs),
    /// Schema-level diff between two SQLite files (tables, columns, indexes).
    Diff(DiffArgs),
}

#[derive(Args, Debug, Clone)]
pub struct DbArgs {
    /// Path to the SQLite database file.
    #[arg(long)]
    pub db: PathBuf,
}

#[derive(Args, Debug)]
pub struct IndexesArgs {
    #[command(flatten)]
    pub db: DbArgs,
    /// Restrict the listing to indexes on this table.
    #[arg(long)]
    pub table: Option<String>,
}

#[derive(Args, Debug)]
pub struct SchemaArgs {
    #[command(flatten)]
    pub db: DbArgs,
    /// The table or view to describe. If omitted, describes every user table.
    pub table: Option<String>,
}

#[derive(Args, Debug)]
pub struct QueryArgs {
    #[command(flatten)]
    pub db: DbArgs,
    /// SELECT statement to run. Use positional `?1`, `?2`, ... for parameters.
    pub sql: String,
    /// Positional parameter (JSON-valued). Repeat for `?2`, `?3`, ...
    #[arg(short = 'p', long = "param", value_name = "JSON")]
    pub params: Vec<String>,
    /// Maximum rows to return.
    #[arg(long, default_value_t = 1000)]
    pub limit: u32,
    /// Rows to skip before returning.
    #[arg(long, default_value_t = 0)]
    pub offset: u32,
    /// Emit NDJSON (one JSON row per line) instead of a buffered result set.
    /// Useful for piping huge result sets without blowing memory.
    #[arg(long)]
    pub stream: bool,
}

#[derive(Args, Debug)]
pub struct ExecArgs {
    #[command(flatten)]
    pub db: DbArgs,
    /// The SQL to execute (INSERT/UPDATE/DELETE/DDL).
    pub sql: String,
    /// Positional parameter (JSON-valued). Repeat for `?2`, `?3`, ...
    #[arg(short = 'p', long = "param", value_name = "JSON")]
    pub params: Vec<String>,
    /// Opt-in flag required to open the database read-write. Without this,
    /// `exec` refuses to run — enforced to keep agents from mutating data
    /// without explicit user intent.
    #[arg(long)]
    pub write: bool,
}

#[derive(Args, Debug)]
pub struct PragmaArgs {
    #[command(flatten)]
    pub db: DbArgs,
    /// PRAGMA name (e.g. `user_version`, `journal_mode`).
    pub name: String,
    /// Optional new value. Requires `--write`. Must be numeric, a bare
    /// keyword, or a single-quoted literal.
    pub value: Option<String>,
    /// Open the DB read-write. Required only when `value` is provided.
    #[arg(long)]
    pub write: bool,
}

#[derive(Args, Debug)]
pub struct DumpArgs {
    #[command(flatten)]
    pub db: DbArgs,
    /// Emit schema statements only (no INSERTs).
    #[arg(long, conflicts_with = "data_only")]
    pub schema_only: bool,
    /// Emit data (INSERT) statements only.
    #[arg(long, conflicts_with = "schema_only")]
    pub data_only: bool,
    /// Restrict the dump to specific tables. Repeat for multiple.
    #[arg(long = "table", value_name = "TABLE")]
    pub tables: Vec<String>,
}

#[derive(Args, Debug)]
pub struct PushArgs {
    /// The SQL to send. Will run against whatever DB is currently open in
    /// the desktop app (the user opens it first via the GUI or `push-open`).
    pub sql: String,
    /// Positional parameter (JSON-valued). Repeat for `?2`, `?3`, ...
    #[arg(short = 'p', long = "param", value_name = "JSON")]
    pub params: Vec<String>,
    /// Maximum rows to return.
    #[arg(long, default_value_t = 1000)]
    pub limit: u32,
    /// Rows to skip before returning.
    #[arg(long, default_value_t = 0)]
    pub offset: u32,
    /// Override the desktop port (default: scan 50500..=50509).
    #[arg(long)]
    pub port: Option<u16>,
    /// Bypass the dry-run preview and execute immediately. Default: SELECT
    /// / EXPLAIN run automatically; anything that looks like a write is
    /// staged in the desktop editor for the human to approve. Pass `--run`
    /// from trusted agent loops that have already consented.
    #[arg(long)]
    pub run: bool,
}

#[derive(Args, Debug)]
pub struct ImportArgs {
    #[command(flatten)]
    pub db: DbArgs,
    /// Target table (must already exist).
    #[arg(long)]
    pub table: String,
    /// Path to the input file.
    pub file: std::path::PathBuf,
    /// File format. `auto` detects from extension (`.csv` / `.json` /
    /// `.jsonl` / `.ndjson`). CSV flags (`--no-header`, `--delimiter`,
    /// `--null-token`) only apply when the effective format is CSV.
    #[arg(long, value_enum, default_value = "auto")]
    pub format: ImportFormat,
    /// First row is NOT a header. CSV only.
    #[arg(long)]
    pub no_header: bool,
    /// Field delimiter (one byte). Default is `,`. CSV only.
    #[arg(long, default_value = ",")]
    pub delimiter: String,
    /// CSV field that should become NULL (e.g. `""` or `"NULL"`). CSV only.
    #[arg(long, value_name = "STRING")]
    pub null_token: Option<String>,
    /// Required to open the database read-write.
    #[arg(long)]
    pub write: bool,
}

#[derive(clap::ValueEnum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImportFormat {
    Auto,
    Csv,
    Json,
    Jsonl,
    Ndjson,
}

#[derive(Args, Debug)]
pub struct MaintenanceArgs {
    #[command(flatten)]
    pub db: DbArgs,
    #[command(subcommand)]
    pub task: MaintenanceTask,
    /// Open the DB read-write. Required for vacuum/reindex/analyze/wal.
    #[arg(long)]
    pub write: bool,
}

#[derive(clap::Subcommand, Debug)]
pub enum MaintenanceTask {
    /// Rewrite the file, reclaiming free pages.
    Vacuum,
    /// Rebuild indexes (optionally target a single table/index).
    Reindex {
        #[arg(long)]
        table: Option<String>,
    },
    /// Refresh query-planner statistics.
    Analyze {
        #[arg(long)]
        table: Option<String>,
    },
    /// PRAGMA integrity_check (works on read-only connections too).
    #[command(name = "integrity-check")]
    IntegrityCheck,
    /// PRAGMA wal_checkpoint(MODE).
    #[command(name = "wal-checkpoint")]
    WalCheckpoint {
        #[arg(long, default_value = "TRUNCATE")]
        mode: String,
    },
}

#[derive(Args, Debug)]
pub struct HistoryArgs {
    /// Case-insensitive substring match on the SQL text or DB path.
    #[arg(long)]
    pub grep: Option<String>,
    /// Only records for queries against this DB path.
    #[arg(long)]
    pub db: Option<String>,
    /// Source filter: `ui`, `agent`, `cli`.
    #[arg(long)]
    pub source: Option<String>,
    /// Records from the last N minutes.
    #[arg(long, value_name = "N")]
    pub since_minutes: Option<i64>,
    /// Maximum records to return (0 = all).
    #[arg(long, default_value_t = 100)]
    pub limit: u32,
}

#[derive(Args, Debug)]
pub struct DiffArgs {
    /// First (baseline) DB path.
    #[arg(long)]
    pub a: std::path::PathBuf,
    /// Second DB path to compare against `--a`.
    #[arg(long)]
    pub b: std::path::PathBuf,
}

#[derive(Args, Debug)]
pub struct CheckpointArgs {
    #[command(flatten)]
    pub db: DbArgs,
    /// Destination path for the snapshot file. Overwritten if it exists.
    #[arg(long)]
    pub to: std::path::PathBuf,
}

#[derive(Args, Debug)]
pub struct PushOpenArgs {
    /// Path to the SQLite database file.
    pub path: String,
    /// Open read-write. Default is read-only (safer for agent workflows).
    #[arg(long)]
    pub write: bool,
    /// Override the desktop port (default: scan 50500..=50509).
    #[arg(long)]
    pub port: Option<u16>,
}
