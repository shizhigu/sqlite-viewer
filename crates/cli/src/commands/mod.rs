mod dump;
mod exec;
mod import;
mod open;
mod pragma;
mod push;
mod query;
mod schema;
mod simple;
mod stats;

use sqlv_core::{Db, OpenOpts};

use crate::cli::{Cli, Command, DbArgs};
use crate::exit::Failure;

pub fn dispatch(cli: Cli) -> Result<(), Failure> {
    let force_json = cli.json;
    match cli.command {
        Command::Open(a) => open::run(a, force_json),
        Command::Tables(a) => simple::tables(a, force_json),
        Command::Views(a) => simple::views(a, force_json),
        Command::Indexes(a) => simple::indexes(a, force_json),
        Command::Schema(a) => schema::run(a, force_json),
        Command::Query(a) => query::run(a, force_json),
        Command::Exec(a) => exec::run(a, force_json),
        Command::Stats(a) => stats::run(a, force_json),
        Command::Pragma(a) => pragma::run(a, force_json),
        Command::Dump(a) => dump::run(a),
        Command::Push(a) => push::query(a, force_json),
        Command::PushOpen(a) => push::open(a, force_json),
        Command::Import(a) => import::run(a, force_json),
    }
}

/// Open helper shared by read-only commands.
pub(crate) fn open_readonly(args: &DbArgs) -> Result<Db, Failure> {
    Db::open(&args.db, OpenOpts::default()).map_err(Into::into)
}
