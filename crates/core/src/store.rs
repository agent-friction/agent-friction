use chrono::{DateTime, Utc};

use crate::db::Db;
use crate::error::Result;
use crate::types::{PermissionEvent, ToolFailure};

impl Db {
    pub fn insert_permission(&self, event: &PermissionEvent) -> Result<i64> {
        let sql = "INSERT INTO permission_events
                   (timestamp, agent, session_id, repo, model, tool, pattern, decision, context)
                   VALUES (:timestamp, :agent, :session_id, :repo, :model, :tool, :pattern, :decision, :context)
                   RETURNING id";
        let id = self.conn().query_one(
            sql,
            rusqlite::named_params! {
                ":timestamp": event.timestamp,
                ":agent": event.agent,
                ":session_id": event.session_id,
                ":repo": event.repo,
                ":model": event.model,
                ":tool": event.tool,
                ":pattern": event.pattern,
                ":decision": event.decision,
                ":context": event.context,
            },
            |r| r.get(0),
        )?;

        Ok(id)
    }

    pub fn insert_failure(&self, failure: &ToolFailure) -> Result<i64> {
        let sql = "INSERT INTO tool_failures
                   (timestamp, agent, session_id, repo, model, tool, input, error, source, context)
                   VALUES (:timestamp, :agent, :session_id, :repo, :model, :tool, :input, :error, :source, :context)
                   RETURNING id";
        let id = self.conn().query_one(
            sql,
            rusqlite::named_params! {
                ":timestamp": failure.timestamp,
                ":agent": failure.agent,
                ":session_id": failure.session_id,
                ":repo": failure.repo,
                ":model": failure.model,
                ":tool": failure.tool,
                ":input": failure.input,
                ":error": failure.error,
                ":source": failure.source,
                ":context": failure.context,
            },
            |r| r.get(0),
        )?;

        Ok(id)
    }

    pub fn prune_before(&mut self, cutoff: DateTime<Utc>) -> Result<usize> {
        let tx = self.conn.transaction()?;
        let prune_tool_failures = "DELETE FROM tool_failures WHERE timestamp < ?1";
        let tool_failures_pruned = tx.execute(prune_tool_failures, rusqlite::params![cutoff])?;

        let prune_permission_events = "DELETE FROM permission_events WHERE timestamp < ?1";
        let permission_events_pruned =
            tx.execute(prune_permission_events, rusqlite::params![cutoff])?;

        tx.commit()?;

        Ok(tool_failures_pruned + permission_events_pruned)
    }

    #[cfg(test)]
    pub(crate) fn get_permission(&self, id: i64) -> Result<PermissionEvent> {
        let sql = "SELECT * FROM permission_events WHERE id = ?1";
        let permission_event = self.conn().query_one(sql, rusqlite::params![id], |r| {
            Ok(PermissionEvent {
                id: r.get("id")?,
                timestamp: r.get("timestamp")?,
                agent: r.get("agent")?,
                session_id: r.get("session_id")?,
                repo: r.get("repo")?,
                model: r.get("model")?,
                tool: r.get("tool")?,
                pattern: r.get("pattern")?,
                decision: r.get("decision")?,
                context: r.get("context")?,
            })
        })?;
        Ok(permission_event)
    }

    #[cfg(test)]
    pub(crate) fn get_tool_failure(&self, id: i64) -> Result<ToolFailure> {
        let sql = "SELECT * FROM tool_failures WHERE id = ?1";
        let tool_failure = self.conn().query_one(sql, rusqlite::params![id], |r| {
            Ok(ToolFailure {
                id: r.get("id")?,
                timestamp: r.get("timestamp")?,
                agent: r.get("agent")?,
                context: r.get("context")?,
                error: r.get("error")?,
                input: r.get("input")?,
                model: r.get("model")?,
                repo: r.get("repo")?,
                session_id: r.get("session_id")?,
                source: r.get("source")?,
                tool: r.get("tool")?,
            })
        })?;

        Ok(tool_failure)
    }
}

#[cfg(test)]
mod test {
    use chrono::TimeDelta;

    use super::*;
    use crate::types::Decision::*;
    use crate::types::Source::*;

    fn sample_permission_event() -> PermissionEvent {
        PermissionEvent {
            id: None,
            timestamp: chrono::Utc::now(),
            agent: String::from("opencode"),
            session_id: None,
            repo: Some("agent-friction".to_string()),
            model: None,
            tool: String::from("bash"),
            context: Some(serde_json::json!({"patterns": ["git push *"], "n": 3})),
            pattern: String::from("grep *"),
            decision: AllowOnce,
        }
    }

    fn sample_tool_failure() -> ToolFailure {
        ToolFailure {
            id: None,
            timestamp: chrono::Utc::now(),
            agent: String::from("claude-code"),
            context: None,
            error: String::from("error"),
            input: None,
            model: None,
            repo: Some(String::from("agent-friction")),
            session_id: Some(String::from("sess_1234")),
            source: Hook,
            tool: String::from("edit"),
        }
    }

    #[test]
    fn test_permission_roundtrip() -> Result<()> {
        let db = Db::open_in_memory()?;
        let mut permission_event = sample_permission_event();
        permission_event.id = Some(db.insert_permission(&permission_event)?);
        let got = db.get_permission(permission_event.id.unwrap())?;
        assert_eq!(permission_event, got);

        Ok(())
    }

    #[test]
    fn test_tool_failure_roundtrip() -> Result<()> {
        let db = Db::open_in_memory()?;
        let mut tool_failure = sample_tool_failure();
        tool_failure.id = Some(db.insert_failure(&tool_failure)?);
        let got = db.get_tool_failure(tool_failure.id.unwrap())?;
        assert_eq!(tool_failure, got);

        Ok(())
    }

    #[test]
    fn test_prune_deletes_old_entries() -> Result<()> {
        let mut db = Db::open_in_memory()?;

        let mut tool_failure_old = sample_tool_failure();
        tool_failure_old.timestamp -= TimeDelta::days(2);
        tool_failure_old.id = Some(db.insert_failure(&tool_failure_old)?);

        let mut tool_failure_new = sample_tool_failure();
        tool_failure_new.id = Some(db.insert_failure(&tool_failure_new)?);

        let mut permission_event_old = sample_permission_event();
        permission_event_old.timestamp -= TimeDelta::days(2);
        permission_event_old.id = Some(db.insert_permission(&permission_event_old)?);

        let mut permission_event_new = sample_permission_event();
        permission_event_new.id = Some(db.insert_permission(&permission_event_new)?);

        let deleted_count = db.prune_before(Utc::now() - TimeDelta::days(1))?;

        assert_eq!(deleted_count, 2);
        assert_eq!(db.get_tool_failure(tool_failure_new.id.unwrap())?, tool_failure_new);
        assert_eq!(db.get_permission(permission_event_new.id.unwrap())?, permission_event_new);

        Ok(())
    }
}
