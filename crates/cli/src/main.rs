mod cli;
mod commands;
mod exit;
mod output;
mod params;

use clap::Parser;

use crate::cli::Cli;

fn main() {
    let args = Cli::parse();
    match commands::dispatch(args) {
        Ok(()) => std::process::exit(exit::EXIT_OK),
        Err(fail) => {
            output::emit_error(fail.code(), &fail.message());
            std::process::exit(fail.exit_code());
        }
    }
}
