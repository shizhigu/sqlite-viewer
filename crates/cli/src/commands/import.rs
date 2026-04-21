use sqlv_core::{CsvImportOpts, Db, OpenOpts};

use crate::cli::ImportArgs;
use crate::exit::Failure;
use crate::output;

pub fn run(args: ImportArgs, force_json: bool) -> Result<(), Failure> {
    if !args.write {
        return Err(Failure::usage(
            "import writes to the database — pass --write to opt in.",
        ));
    }

    let delim_bytes = args.delimiter.as_bytes();
    if delim_bytes.len() != 1 {
        return Err(Failure::usage(
            "--delimiter must be exactly one byte (e.g. `,`, `\\t`).",
        ));
    }
    let opts = CsvImportOpts {
        has_header: !args.no_header,
        delimiter: delim_bytes[0],
    };

    let db = Db::open(
        &args.db.db,
        OpenOpts {
            read_only: false,
            timeout_ms: Some(30_000),
        },
    )?;
    let res = db.import_csv(&args.file, &args.table, opts)?;
    output::emit(&res, force_json);
    Ok(())
}
