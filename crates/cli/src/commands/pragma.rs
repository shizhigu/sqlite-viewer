use sqlv_core::{Db, OpenOpts};

use crate::cli::PragmaArgs;
use crate::exit::Failure;
use crate::output;

pub fn run(args: PragmaArgs, force_json: bool) -> Result<(), Failure> {
    let setting = args.value.is_some();
    if setting && !args.write {
        return Err(Failure::usage(
            "setting a PRAGMA requires --write to open the database read-write.",
        ));
    }

    let opts = if setting {
        OpenOpts { read_only: false, timeout_ms: Some(5_000) }
    } else {
        OpenOpts::default()
    };
    let db = Db::open(&args.db.db, opts)?;

    let result = db.pragma(&args.name, args.value.as_deref())?;
    output::emit(&result, force_json);
    Ok(())
}
