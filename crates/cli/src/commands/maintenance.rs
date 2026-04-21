use sqlv_core::{Db, OpenOpts};

use crate::cli::{MaintenanceArgs, MaintenanceTask};
use crate::exit::Failure;
use crate::output;

pub fn run(args: MaintenanceArgs, force_json: bool) -> Result<(), Failure> {
    // integrity_check can run read-only; others need --write.
    let needs_write = !matches!(args.task, MaintenanceTask::IntegrityCheck);
    if needs_write && !args.write {
        return Err(Failure::usage(
            "this maintenance task writes to the database — pass --write to opt in.",
        ));
    }

    let opts = if args.write {
        OpenOpts {
            read_only: false,
            timeout_ms: Some(60_000),
        }
    } else {
        OpenOpts {
            read_only: true,
            timeout_ms: Some(60_000),
        }
    };
    let db = Db::open(&args.db.db, opts)?;

    let result = match args.task {
        MaintenanceTask::Vacuum => db.vacuum(),
        MaintenanceTask::Reindex { table } => db.reindex(table.as_deref()),
        MaintenanceTask::Analyze { table } => db.analyze(table.as_deref()),
        MaintenanceTask::IntegrityCheck => db.integrity_check(),
        MaintenanceTask::WalCheckpoint { mode } => db.wal_checkpoint(&mode),
    }?;

    output::emit(&result, force_json);
    Ok(())
}
