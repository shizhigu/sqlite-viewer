use crate::cli::DbArgs;
use crate::exit::Failure;
use crate::output;

use super::open_readonly;

pub fn run(args: DbArgs, force_json: bool) -> Result<(), Failure> {
    let db = open_readonly(&args)?;
    output::emit(&db.stats()?, force_json);
    Ok(())
}
