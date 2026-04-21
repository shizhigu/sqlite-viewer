use std::fs;
use std::time::Instant;

use serde::Serialize;
use sqlv_core::{Db, OpenOpts, Value};

use crate::cli::CheckpointArgs;
use crate::exit::Failure;
use crate::output;

#[derive(Serialize)]
struct CheckpointResult {
    source: String,
    destination: String,
    bytes: u64,
    elapsed_ms: u64,
}

pub fn run(args: CheckpointArgs, force_json: bool) -> Result<(), Failure> {
    // VACUUM INTO requires the destination not to exist. Be forgiving: if
    // it does, remove it first (documented behavior). We treat the
    // destination as the checkpoint's single source of truth.
    if args.to.exists() {
        fs::remove_file(&args.to).map_err(|e| {
            Failure::new(
                "io",
                format!("cannot overwrite {}: {e}", args.to.display()),
                crate::exit::EXIT_OTHER,
            )
        })?;
    }

    // Open read-only — VACUUM INTO needs only a read connection on the
    // source. This avoids taking the write lock on the original DB.
    let db = Db::open(
        &args.db.db,
        OpenOpts {
            read_only: true,
            timeout_ms: Some(60_000),
        },
    )?;

    let dest_str = args
        .to
        .to_str()
        .ok_or_else(|| Failure::usage("destination path must be valid UTF-8".to_string()))?;
    // SQLite single-quoted string literal, escape any inner quotes.
    let escaped = dest_str.replace('\'', "''");
    let sql = format!("VACUUM INTO '{escaped}'");

    let start = Instant::now();
    // `query` (not `exec`): VACUUM INTO is read-only from sqlv-core's
    // perspective and works on a read-only connection.
    let r = db
        .query(&sql, &[], sqlv_core::Page::default())
        .map_err(Failure::from)?;
    // VACUUM INTO returns no rows but also no structured output.
    // `r.rows` is empty on success — fall through without inspecting.
    let _ = r;

    let bytes = fs::metadata(&args.to).map(|m| m.len()).unwrap_or(0);
    let result = CheckpointResult {
        source: args.db.db.display().to_string(),
        destination: args.to.display().to_string(),
        bytes,
        elapsed_ms: start.elapsed().as_millis() as u64,
    };
    output::emit(&result, force_json);
    // Silence "unused Value" lint for future extension.
    let _ = std::marker::PhantomData::<Value>;
    Ok(())
}
