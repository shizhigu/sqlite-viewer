use std::io::{self, Write};

use sqlv_core::Page;

use crate::cli::QueryArgs;
use crate::exit::Failure;
use crate::output;
use crate::params::parse_params;

use super::open_readonly;

pub fn run(args: QueryArgs, force_json: bool) -> Result<(), Failure> {
    let db = open_readonly(&args.db)?;
    let params = parse_params(&args.params)?;

    if args.stream {
        return run_streaming(&db, &args, &params);
    }

    let page = Page { limit: args.limit, offset: args.offset };
    let res = db.query(&args.sql, &params, page)?;
    output::emit(&res, force_json);
    Ok(())
}

/// Streaming mode: emit one NDJSON record per line. First line is a
/// `{"type":"header", "columns": [...], "column_types": [...]}` record,
/// then a sequence of `{"type":"row", "values": [...]}` records, then a
/// closing `{"type":"summary","rows":N,"elapsed_ms":M,"truncated":bool}`.
/// `--offset` is honored; `--limit` caps the row count (truncated=true when
/// more existed). Stdout is line-buffered for pipe-friendliness.
fn run_streaming(
    db: &sqlv_core::Db,
    args: &crate::cli::QueryArgs,
    params: &[sqlv_core::Value],
) -> Result<(), Failure> {
    let started = std::time::Instant::now();
    let stdout = io::stdout();
    let mut out = stdout.lock();

    let mut res = db
        .query(
            &args.sql,
            params,
            Page { limit: args.limit, offset: args.offset },
        )?;

    // Header first.
    let header = serde_json::json!({
        "type": "header",
        "columns": res.columns,
        "column_types": res.column_types,
    });
    let _ = serde_json::to_writer(&mut out, &header);
    let _ = out.write_all(b"\n");

    // Rows next. (Core buffers up to `limit` currently — a true-streaming
    // iterator is the obvious next refactor inside sqlv-core. The CLI
    // contract documented here doesn't change.)
    let rows_emitted = res.rows.len() as u64;
    for row in res.rows.drain(..) {
        let record = serde_json::json!({
            "type": "row",
            "values": row,
        });
        let _ = serde_json::to_writer(&mut out, &record);
        let _ = out.write_all(b"\n");
    }

    let summary = serde_json::json!({
        "type": "summary",
        "rows": rows_emitted,
        "elapsed_ms": started.elapsed().as_millis() as u64,
        "truncated": res.truncated,
    });
    let _ = serde_json::to_writer(&mut out, &summary);
    let _ = out.write_all(b"\n");
    Ok(())
}
