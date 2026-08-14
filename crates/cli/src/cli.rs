use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};

use agent_friction_core::Decision;

#[derive(Parser)]
#[command(name = "agent-friction", version)]
pub struct Cli {
    /// Override the database location (defaults to XDG data dir)
    #[arg(long, global = true)]
    pub db: Option<PathBuf>,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    /// Logs permission events and tool failures for future analysis
    #[command(subcommand)]
    Log(LogCommand),
}

#[derive(Subcommand)]
pub enum LogCommand {
    /// Logs permission events for future analysis
    Permission(PermissionArgs),
    /// Logs tool failures for future analysis
    Failure(FailureArgs),
}

#[derive(Args)]
pub struct PermissionArgs {
    #[command(flatten)]
    pub common: CommonArgs,
    #[arg(long)]
    pub pattern: String,
    #[arg(long)]
    pub decision: Decision,
}

#[derive(Args)]
pub struct FailureArgs {
    #[command(flatten)]
    pub common: CommonArgs,
    #[arg(long)]
    pub error: String,
    #[arg(long, value_parser = parse_json)]
    pub input: Option<serde_json::Value>,
}

#[derive(Args)]
pub struct CommonArgs {
    #[arg(long)]
    pub agent: String,
    #[arg(long)]
    pub tool: String,
    #[arg(long)]
    pub session_id: Option<String>,
    #[arg(long)]
    pub repo: Option<String>,
    #[arg(long)]
    pub model: Option<String>,
    #[arg(long, value_parser = parse_json)]
    pub context: Option<serde_json::Value>,
}

fn parse_json(s: &str) -> Result<serde_json::Value, serde_json::Error> {
    serde_json::from_str(s)
}
