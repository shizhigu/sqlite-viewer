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
    /// Bulk-load a CSV file into a table (transactional — all-or-nothing).
    Import(ImportArgs),
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
    /// Target table (must already exist with matching columns).
    #[arg(long)]
    pub table: String,
    /// Path to the input file (CSV).
    pub file: std::path::PathBuf,
    /// First row is NOT a header. Columns are then taken from the target
    /// table in declared order.
    #[arg(long)]
    pub no_header: bool,
    /// Field delimiter (one byte). Default is `,`.
    #[arg(long, default_value = ",")]
    pub delimiter: String,
    /// When set, any CSV field whose raw string equals this exact value is
    /// inserted as NULL. Use `--null-token ""` to treat unquoted empty
    /// fields as NULL, or `--null-token NULL` for the literal string.
    #[arg(long, value_name = "STRING")]
    pub null_token: Option<String>,
    /// Required to open the database read-write.
    #[arg(long)]
    pub write: bool,
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
