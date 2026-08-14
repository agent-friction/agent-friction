use agent_friction_cli::{cli::Cli, run};
use agent_friction_core::{Db, Scope};
use anyhow::Result;
use chrono::{TimeDelta, Utc};
use clap::Parser;

fn parse(args: &[&str]) -> Result<Cli> {
    Ok(Cli::try_parse_from(args)?)
}

#[test]
fn log_failure_writes_a_record() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let db_path = dir.path().join("friction.db");
    let db_arg = db_path.to_str().expect("tempdir path is valid utf-8");

    let cli = parse(&[
        "agent-friction",
        "--db", db_arg,
        "log", "failure",
        "--agent", "opencode",
        "--tool", "bash",
        "--error", "exit 1: make: no rule for make target 'test'",
        "--input", r#"{"command":"make test"}"#,
    ])?;

    run(cli)?;

    let db = Db::open(&db_path)?;
    let stats = db.get_failure_stats(Utc::now() - TimeDelta::days(1), Scope::Global)?;

    assert_eq!(stats.len(), 1);
    assert_eq!(stats[0].tool, "bash");

    Ok(())
}

#[test]
fn invalid_decision_is_rejected() {
    let result = Cli::try_parse_from([
        "agent-friction", "log", "permission",
        "--agent", "opencode", "--tool", "bash",
        "--pattern", "git push *", "--decision", "bogus",
    ]);

    assert!(result.is_err());
}
