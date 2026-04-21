use sqlv_core::TableSchema;

use crate::cli::SchemaArgs;
use crate::exit::Failure;
use crate::output;

use super::open_readonly;

pub fn run(args: SchemaArgs, force_json: bool) -> Result<(), Failure> {
    let db = open_readonly(&args.db)?;
    match args.table.as_deref() {
        Some(name) => {
            let schema = db.schema(name)?;
            output::emit(&schema, force_json);
        }
        None => {
            // No table given — describe every user table. Views are intentionally
            // excluded to keep the default output focused; use `sqlv views` or
            // `sqlv schema --db X <view>` for view details.
            let mut all: Vec<TableSchema> = Vec::new();
            for t in db.tables()? {
                all.push(db.schema(&t.name)?);
            }
            output::emit(&all, force_json);
        }
    }
    Ok(())
}
