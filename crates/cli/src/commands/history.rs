use sqlv_core::{ActivityLog, ActivityQuery};

use crate::cli::HistoryArgs;
use crate::exit::Failure;
use crate::output;

pub fn run(args: HistoryArgs, force_json: bool) -> Result<(), Failure> {
    let log = ActivityLog::open_default()?;
    let since_ms = args.since_minutes.map(|m| {
        use std::time::{SystemTime, UNIX_EPOCH};
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0);
        now - m * 60_000
    });
    let q = ActivityQuery {
        grep: args.grep,
        since_ms,
        db_path: args.db,
        source: args.source,
        limit: args.limit,
    };
    let records = log.query(&q)?;
    output::emit(&records, force_json);
    Ok(())
}
