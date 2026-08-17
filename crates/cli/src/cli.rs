use std::path::PathBuf;

use chrono::{DateTime, TimeDelta, Utc};
use clap::{Args, Parser, Subcommand};

use agent_friction_core::{Decision, Limits};
use interim::parse_date_string;

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
    /// Prints stats about permission events and tool failures
    Stats(StatsArgs),
    /// Suggests permission rules from the recorded events
    Analyze(AnalyzeArgs),
}

#[derive(Args)]
pub struct AnalyzeArgs {
    #[command(flatten)]
    pub common: CommonStatsArgs,
    /// Emit the raw suggestions as JSON
    #[arg(long)]
    pub json: bool,
}

#[derive(Subcommand)]
pub enum LogCommand {
    /// Logs permission events for future analysis
    Permission(PermissionArgs),
    /// Logs tool failures for future analysis
    Failure(FailureArgs),
}

#[derive(Args)]
pub struct StatsArgs {
    #[command(subcommand)]
    pub sub_action: Option<StatsSubcommands>,
    #[command(flatten)]
    pub common: CommonStatsArgs,
}

#[derive(Args)]
pub struct PermissionArgs {
    #[command(flatten)]
    pub common: CommonLogsArgs,
    #[arg(long, required = true)]
    pub pattern: Vec<String>,
    #[arg(long)]
    pub decision: Decision,
}

#[derive(Args)]
pub struct FailureArgs {
    #[command(flatten)]
    pub common: CommonLogsArgs,
    #[arg(long)]
    pub error: String,
    #[arg(long, value_parser = parse_json)]
    pub input: Option<serde_json::Value>,
}

#[derive(Args)]
pub struct CommonLogsArgs {
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

#[derive(Subcommand)]
pub enum StatsSubcommands {
    /// Check permissions stats
    Permissions,
    /// Check tool failure stats
    Failures,
}

#[derive(Args)]
pub struct CommonStatsArgs {
    #[arg(long, value_parser = parse_flexible_datetime)]
    since: Option<DateTime<Utc>>,
    #[arg(long)]
    pub repo: Option<String>,
    /// Show only the busiest N rows
    #[arg(long)]
    pub limit: Option<i64>,
    /// Hide anything seen fewer than N times
    #[arg(long, default_value_t = 0)]
    pub min_count: i64,
}

impl CommonStatsArgs {
    pub fn since(&self) -> DateTime<Utc> {
        match self.since {
            Some(dt) => dt,
            None => Utc::now() - TimeDelta::weeks(2),
        }
    }

    pub fn limits(&self) -> Limits {
        Limits {
            min_count: self.min_count,
            limit: self.limit,
        }
    }
}

fn parse_json(s: &str) -> Result<serde_json::Value, serde_json::Error> {
    serde_json::from_str(s)
}

fn parse_flexible_datetime(s: &str) -> Result<DateTime<Utc>, String> {
    if let Ok(dt) = s.parse::<DateTime<Utc>>() {
        return Ok(dt);
    }

    let now = Utc::now();
    parse_date_string(s, now, interim::Dialect::Us)
        .map(|dt| dt.with_timezone(&Utc))
        .map_err(|_| format!("Cloud not parse date expression: '{s}'"))
}
