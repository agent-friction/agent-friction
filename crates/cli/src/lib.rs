pub mod cli;
pub mod commands;
pub mod display;

use anyhow::{Context, Result};
use agent_friction_core::Db;
use cli::{Cli, Command};

pub fn run(cli: Cli) -> Result<()> {
    let path = match cli.db {
        Some(p) => p,
        None => Db::default_path().context("resolving default database path")?,
    };

    let db = Db::open(&path)
        .with_context(|| format!("opening database at {}", path.display()))?;

    match cli.command {
        Command::Log(log) => commands::logs::run(&db, log),
        Command::Stats(stats) => commands::stats::run(&db, stats),
        Command::Analyze(args) => commands::analyze::run(&db, args),
    }
}
