use crate::cli::{DbArgs, IndexesArgs};
use crate::exit::Failure;
use crate::output;

use super::open_readonly;

pub fn tables(args: DbArgs, force_json: bool) -> Result<(), Failure> {
    let db = open_readonly(&args)?;
    output::emit(&db.tables()?, force_json);
    Ok(())
}

pub fn views(args: DbArgs, force_json: bool) -> Result<(), Failure> {
    let db = open_readonly(&args)?;
    output::emit(&db.views()?, force_json);
    Ok(())
}

pub fn indexes(args: IndexesArgs, force_json: bool) -> Result<(), Failure> {
    let db = open_readonly(&args.db)?;
    let list = db.indexes(args.table.as_deref())?;
    output::emit(&list, force_json);
    Ok(())
}
