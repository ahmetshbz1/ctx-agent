use anyhow::{Context, Result};
use clap::Parser;
use std::path::PathBuf;

mod cli;
mod commands;

use cli::Cli;

fn get_project_root(cli: &Cli) -> Result<PathBuf> {
    match &cli.project {
        Some(p) => Ok(p.clone()),
        None => std::env::current_dir().context("Failed to get current directory"),
    }
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let root = get_project_root(&cli)?;
    commands::run(cli.command, &root, cli.json)
}
