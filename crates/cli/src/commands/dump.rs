use sqlv_core::DumpFilter;
use std::io::{self, Write};

use crate::cli::DumpArgs;
use crate::exit::Failure;

use super::open_readonly;

pub fn run(args: DumpArgs) -> Result<(), Failure> {
    let db = open_readonly(&args.db)?;
    let (schema, data) = match (args.schema_only, args.data_only) {
        (true, false) => (true, false),
        (false, true) => (false, true),
        (false, false) => (true, true),
        (true, true) => unreachable!("clap conflicts_with enforces mutual exclusion"),
    };
    let filter = DumpFilter {
        schema,
        data,
        only_tables: if args.tables.is_empty() {
            None
        } else {
            Some(args.tables.as_slice())
        },
    };
    let sql = db.dump(filter)?;
    // Dump is raw SQL text; bypass the JSON emitter.
    let mut out = io::stdout().lock();
    out.write_all(sql.as_bytes())
        .map_err(|e| Failure::new("io", e.to_string(), crate::exit::EXIT_OTHER))?;
    if !sql.ends_with('\n') {
        let _ = out.write_all(b"\n");
    }
    Ok(())
}
