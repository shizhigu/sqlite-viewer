use sqlv_core::{Db, OpenOpts};

use crate::cli::ExecArgs;
use crate::exit::Failure;
use crate::output;
use crate::params::parse_params;

pub fn run(args: ExecArgs, force_json: bool) -> Result<(), Failure> {
    if !args.write {
        return Err(Failure::usage(
            "refusing to run `exec` without --write. Pass --write to opt in to mutations.",
        ));
    }
    let db = Db::open(
        &args.db.db,
        OpenOpts { read_only: false, timeout_ms: Some(5_000) },
    )?;
    let params = parse_params(&args.params)?;
    let res = db.exec(&args.sql, &params)?;
    output::emit(&res, force_json);
    Ok(())
}
