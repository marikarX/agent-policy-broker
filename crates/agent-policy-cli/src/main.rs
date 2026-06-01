use std::process::ExitCode;

use clap::Parser;

mod cli;
mod commands;
mod git;
mod indexing;
mod paths;
mod render;

fn main() -> ExitCode {
    let cli = cli::Cli::parse();
    match cli::run(cli) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error:#}");
            ExitCode::from(1)
        }
    }
}
