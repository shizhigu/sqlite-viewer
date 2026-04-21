use sqlv_core::{diff_schemas, Db, OpenOpts};

use crate::cli::DiffArgs;
use crate::exit::Failure;
use crate::output;

pub fn run(args: DiffArgs, force_json: bool) -> Result<(), Failure> {
    let a = Db::open(&args.a, OpenOpts::default())?;
    let b = Db::open(&args.b, OpenOpts::default())?;
    let report = diff_schemas(&a, &b)?;
    output::emit(&report, force_json);
    Ok(())
}
