//! Command-line interface for the `mdk` toolkit.

mod cli;
mod commands;
mod config;
mod files;
mod paint;

use std::process::ExitCode;

use anyhow::Result;
use clap::Parser;

use crate::cli::{Cli, Command};
use crate::config::Config;

fn main() -> ExitCode {
    // Exit quietly when a downstream pipe closes, instead of panicking.
    sigpipe::reset();
    let cli = Cli::parse();
    match run(&cli) {
        Ok(code) => code,
        Err(error) => {
            eprintln!("error: {error:#}");
            ExitCode::from(2)
        }
    }
}

fn run(cli: &Cli) -> Result<ExitCode> {
    let (config, path) = Config::load(cli.config.as_deref())?;
    if cli.verbose > 0
        && let Some(path) = &path
    {
        eprintln!("mdk: using configuration from {}", path.display());
    }
    let dialect = config.dialect(cli.dialect);
    match &cli.command {
        Command::Lint(args) => commands::lint::run(cli, args, &config, dialect),
        Command::Fmt(args) => commands::fmt::run(cli, args, &config, dialect),
        Command::Render(args) => commands::render::run(cli, args, &config, dialect),
        Command::Rules(args) => commands::rules::run(args),
    }
}
