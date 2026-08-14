use agent_friction_core::{Db, PermissionEvent, Source, ToolFailure};

use crate::cli::{FailureArgs, LogCommand, PermissionArgs};
use anyhow::{Context, Result};
use chrono::Utc;

pub fn run(db: &Db, logs: LogCommand) -> Result<()> {
    match logs {
        LogCommand::Permission(args) => run_permissions(db, args),
        LogCommand::Failure(args) => run_failure(db, args),
    }
}

fn run_permissions(db: &Db, permission: PermissionArgs) -> Result<()> {
    db.insert_permission(&PermissionEvent {
        id: None,
        timestamp: Utc::now(),
        agent: permission.common.agent,
        session_id: permission.common.session_id,
        repo: permission.common.repo,
        model: permission.common.model,
        tool: permission.common.tool,
        pattern: permission.pattern,
        decision: permission.decision,
        context: permission.common.context,
    })
    .context("inserting permission into db")?;

    Ok(())
}

fn run_failure(db: &Db, failure: FailureArgs) -> Result<()> {
    db.insert_failure(&ToolFailure {
        id: None,
        timestamp: Utc::now(),
        agent: failure.common.agent,
        session_id: failure.common.session_id,
        repo: failure.common.repo,
        model: failure.common.model,
        tool: failure.common.tool,
        context: failure.common.context,
        error: failure.error,
        input: failure.input,
        source: Source::Hook,
    })
    .context("inserting tool into db")?;

    Ok(())
}
