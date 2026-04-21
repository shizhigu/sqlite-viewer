use crate::cli::DbArgs;
use crate::exit::Failure;
use crate::output;

use super::open_readonly;

pub fn run(args: DbArgs, force_json: bool) -> Result<(), Failure> {
    let db = open_readonly(&args)?;
    let meta = db.meta()?;
    output::emit(&meta, force_json);
    Ok(())
}
