use serde::Serialize;
use std::collections::HashMap;

use chrono::{DateTime, Utc};
use rusqlite::named_params;

use crate::{
    Result,
    db::Db,
    normalize::normalize_error,
    types::{Decision, Source},
};

#[derive(Debug, PartialEq, Clone, Serialize)]
pub struct PermissionStat {
    pub tool: String,
    pub pattern: String,
    pub allow_once: i64,
    pub allow_always: i64,
    pub deny: i64,
}

#[derive(Debug, PartialEq, Clone, Serialize)]
pub struct FailureStat {
    pub tool: String,
    pub error_pattern: String,
    pub example: String,
    pub count: i64,
    pub source: Source,
}

#[derive(Debug, PartialEq, Clone)]
pub enum Scope {
    Global,
    Repo(String),
}

struct FailureRaw {
    tool: String,
    error: String,
    source: Source,
}

impl Db {
    const WHERE_CLAUSE: &str = "WHERE timestamp >= :since AND (:repo IS NULL OR repo = :repo)";

    pub fn get_permission_stats(
        &self,
        since: DateTime<Utc>,
        scope: Scope,
    ) -> Result<Vec<PermissionStat>> {
        let sql = format!(
            "SELECT tool,
                          pattern,
                          SUM(CASE WHEN decision = :allow_once THEN 1 ELSE 0 END) as allow_once,
                          SUM(CASE WHEN decision = :allow_always THEN 1 ELSE 0 END) as allow_always,
                          SUM(CASE WHEN decision = :deny THEN 1 ELSE 0 END) as deny
                   FROM permission_events
                   {}
                   GROUP BY tool, pattern
                   ORDER BY allow_once DESC, tool ASC, pattern ASC",
            Self::WHERE_CLAUSE
        );

        let mut stmt = self.conn().prepare(sql.as_str())?;
        let rows = stmt.query_map(
            named_params! {
                ":allow_once": Decision::AllowOnce.as_str(),
                ":allow_always": Decision::AllowAlways.as_str(),
                ":deny": Decision::Deny.as_str(),
                ":since": since,
                ":repo": scope.as_repo(),
            },
            |r| {
                Ok(PermissionStat {
                    tool: r.get("tool")?,
                    pattern: r.get("pattern")?,
                    allow_once: r.get("allow_once")?,
                    allow_always: r.get("allow_always")?,
                    deny: r.get("deny")?,
                })
            },
        )?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(Into::into)
    }

    pub fn get_failure_stats(
        &self,
        since: DateTime<Utc>,
        scope: Scope,
    ) -> Result<Vec<FailureStat>> {
        let sql = format!(
            "SELECT tool, error, source
                           FROM tool_failures
                           {}",
            Self::WHERE_CLAUSE
        );

        let mut stmt = self.conn().prepare(sql.as_str())?;
        let rows = stmt.query_map(
            named_params! {
                    ":since": since,
                    ":repo": scope.as_repo(),
            },
            |r| {
                Ok(FailureRaw {
                    tool: r.get("tool")?,
                    error: r.get("error")?,
                    source: r.get("source")?,
                })
            },
        )?;

        let raw = rows.collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(Self::aggregate_failures(raw))
    }

    fn aggregate_failures(rows: impl IntoIterator<Item = FailureRaw>) -> Vec<FailureStat> {
        let mut counts: HashMap<(String, String, Source), (i64, String)> = HashMap::new();
        for row in rows {
            let key = (row.tool, normalize_error(&row.error), row.source);
            counts
                .entry(key)
                .and_modify(|v| v.0 += 1)
                .or_insert((1, row.error));
        }

        let mut stats: Vec<FailureStat> = counts
            .into_iter()
            .map(
                |((tool, error_pattern, source), (count, example))| FailureStat {
                    tool,
                    error_pattern,
                    example,
                    count,
                    source,
                },
            )
            .collect();

        stats.sort_unstable_by(|a, b| {
            b.count
                .cmp(&a.count)
                .then_with(|| a.tool.cmp(&b.tool))
                .then_with(|| a.error_pattern.cmp(&b.error_pattern))
        });
        stats
    }
}

impl Scope {
    fn as_repo(&self) -> Option<&str> {
        match self {
            Scope::Repo(repo) => Some(repo.as_str()),
            Scope::Global => None,
        }
    }
}

#[cfg(test)]
mod test {
    use chrono::TimeDelta;

    use crate::{
        PermissionEvent,
        Source::{self},
        ToolFailure,
    };

    use super::*;

    pub fn create_permission(
        repo: Option<String>,
        tool: String,
        pattern: String,
        decision: Decision,
    ) -> PermissionEvent {
        PermissionEvent {
            id: None,
            timestamp: Utc::now(),
            agent: String::from("opencode"),
            session_id: Some(String::from("s_123456")),
            repo,
            model: None,
            tool,
            pattern,
            decision,
            context: None,
        }
    }

    fn insert_permissions(db: &Db, permissions: Vec<PermissionEvent>) -> Result<()> {
        for p in permissions {
            let _ = db.insert_permission(&p)?;
        }

        Ok(())
    }

    #[test]
    fn permissions_aggregate_global() -> Result<()> {
        let db = Db::open_in_memory()?;
        insert_permissions(
            &db,
            vec![
                create_permission(
                    Some(String::from("test")),
                    String::from("edit"),
                    String::from("*"),
                    Decision::AllowOnce,
                ),
                create_permission(
                    Some(String::from("test")),
                    String::from("edit"),
                    String::from("*"),
                    Decision::AllowOnce,
                ),
                create_permission(
                    Some(String::from("test")),
                    String::from("edit"),
                    String::from("*"),
                    Decision::AllowAlways,
                ),
                create_permission(
                    Some(String::from("test")),
                    String::from("edit"),
                    String::from("*"),
                    Decision::Deny,
                ),
                create_permission(
                    None,
                    String::from("edit"),
                    String::from("*"),
                    Decision::Deny,
                ),
            ],
        )?;

        let stats = db.get_permission_stats(Utc::now() - TimeDelta::days(1), Scope::Global)?;

        assert_eq!(stats.len(), 1);
        assert_eq!(stats.first().unwrap().allow_once, 2);
        assert_eq!(stats.first().unwrap().allow_always, 1);
        assert_eq!(stats.first().unwrap().deny, 2);

        Ok(())
    }

    #[test]
    fn permissions_stats_filter_timestamps() -> Result<()> {
        let db = Db::open_in_memory()?;
        let mut permissions = vec![
            create_permission(
                Some(String::from("test")),
                String::from("edit"),
                String::from("*"),
                Decision::AllowOnce,
            ),
            create_permission(
                Some(String::from("test")),
                String::from("edit"),
                String::from("*"),
                Decision::AllowOnce,
            ),
            create_permission(
                Some(String::from("test")),
                String::from("edit"),
                String::from("*"),
                Decision::AllowAlways,
            ),
            create_permission(
                Some(String::from("test")),
                String::from("edit"),
                String::from("*"),
                Decision::Deny,
            ),
            create_permission(
                None,
                String::from("edit"),
                String::from("*"),
                Decision::Deny,
            ),
        ];
        permissions[0].timestamp -= TimeDelta::days(2);

        insert_permissions(&db, permissions)?;

        let stats = db.get_permission_stats(Utc::now() - TimeDelta::days(1), Scope::Global)?;

        assert_eq!(stats.len(), 1);
        assert_eq!(stats.first().unwrap().allow_once, 1);
        assert_eq!(stats.first().unwrap().allow_always, 1);
        assert_eq!(stats.first().unwrap().deny, 2);

        Ok(())
    }

    #[test]
    fn permissions_aggregate_per_repo() -> Result<()> {
        let db = Db::open_in_memory()?;
        insert_permissions(
            &db,
            vec![
                create_permission(
                    Some(String::from("test")),
                    String::from("edit"),
                    String::from("*"),
                    Decision::AllowOnce,
                ),
                create_permission(
                    Some(String::from("test")),
                    String::from("edit"),
                    String::from("*"),
                    Decision::AllowOnce,
                ),
                create_permission(
                    Some(String::from("test")),
                    String::from("edit"),
                    String::from("*"),
                    Decision::AllowAlways,
                ),
                create_permission(
                    Some(String::from("test")),
                    String::from("edit"),
                    String::from("*"),
                    Decision::Deny,
                ),
                create_permission(
                    None,
                    String::from("edit"),
                    String::from("*"),
                    Decision::Deny,
                ),
            ],
        )?;

        let stats = db.get_permission_stats(
            Utc::now() - TimeDelta::days(1),
            Scope::Repo(String::from("test")),
        )?;

        assert_eq!(stats.len(), 1);
        assert_eq!(stats.first().unwrap().allow_once, 2);
        assert_eq!(stats.first().unwrap().allow_always, 1);
        assert_eq!(stats.first().unwrap().deny, 1);

        Ok(())
    }

    #[test]
    fn permissions_aggregate_per_repo_per_tool() -> Result<()> {
        let db = Db::open_in_memory()?;
        insert_permissions(
            &db,
            vec![
                create_permission(
                    Some(String::from("test")),
                    String::from("edit"),
                    String::from("*"),
                    Decision::AllowOnce,
                ),
                create_permission(
                    Some(String::from("test")),
                    String::from("edit"),
                    String::from("*"),
                    Decision::AllowOnce,
                ),
                create_permission(
                    Some(String::from("test")),
                    String::from("read"),
                    String::from("*"),
                    Decision::AllowAlways,
                ),
                create_permission(
                    Some(String::from("test")),
                    String::from("read"),
                    String::from("*"),
                    Decision::AllowOnce,
                ),
                create_permission(
                    Some(String::from("test")),
                    String::from("delete"),
                    String::from("*"),
                    Decision::Deny,
                ),
                create_permission(
                    Some(String::from("test")),
                    String::from("delete"),
                    String::from("*"),
                    Decision::AllowOnce,
                ),
                create_permission(
                    None,
                    String::from("delete"),
                    String::from("*"),
                    Decision::Deny,
                ),
            ],
        )?;

        let stats = db.get_permission_stats(
            Utc::now() - TimeDelta::days(1),
            Scope::Repo(String::from("test")),
        )?;

        assert_eq!(
            stats,
            vec![
                PermissionStat {
                    tool: "edit".into(),
                    pattern: "*".into(),
                    allow_once: 2,
                    allow_always: 0,
                    deny: 0
                },
                PermissionStat {
                    tool: "delete".into(),
                    pattern: "*".into(),
                    allow_once: 1,
                    allow_always: 0,
                    deny: 1
                },
                PermissionStat {
                    tool: "read".into(),
                    pattern: "*".into(),
                    allow_once: 1,
                    allow_always: 1,
                    deny: 0
                },
            ]
        );

        Ok(())
    }

    pub fn create_failure(
        repo: Option<String>,
        tool: String,
        error: String,
        source: Source,
    ) -> ToolFailure {
        ToolFailure {
            id: None,
            timestamp: Utc::now(),
            agent: String::from("opencode"),
            session_id: Some(String::from("s_123456")),
            repo,
            model: None,
            tool,
            context: None,
            input: None,
            error,
            source,
        }
    }

    fn insert_failures(db: &Db, failures: Vec<ToolFailure>) -> Result<()> {
        for f in failures {
            let _ = db.insert_failure(&f)?;
        }

        Ok(())
    }

    #[test]
    fn failures_aggregate_global() -> Result<()> {
        let db = Db::open_in_memory()?;
        insert_failures(
            &db,
            vec![
                create_failure(
                    Some(String::from("test")),
                    String::from("bash"),
                    String::from(
                        "exit 1: /User/c/repo/foo/node_modules/bin/cypress: invalid argument",
                    ),
                    Source::Hook,
                ),
                create_failure(
                    Some(String::from("test")),
                    String::from("bash"),
                    String::from(
                        "exit 1: /User/c/repo/bar/node_modules/bin/cypress: invalid argument",
                    ),
                    Source::Hook,
                ),
                create_failure(
                    Some(String::from("test")),
                    String::from("bash"),
                    String::from(
                        "exit 1: /User/c/repo/bar/node_modules/bin/jest: invalid argument",
                    ),
                    Source::Hook,
                ),
                create_failure(
                    None,
                    String::from("bash"),
                    String::from(
                        "exit 1: /User/c/repo/foo/node_modules/bin/cypress: invalid argument",
                    ),
                    Source::Hook,
                ),
            ],
        )?;

        let stats = db.get_failure_stats(Utc::now() - TimeDelta::days(1), Scope::Global)?;

        assert_eq!(
            stats,
            vec![
                FailureStat {
                    tool: "bash".into(),
                    error_pattern: "exit 1: <path>/cypress: invalid argument".into(),
                    example: "exit 1: /User/c/repo/foo/node_modules/bin/cypress: invalid argument"
                        .into(),
                    count: 3,
                    source: Source::Hook,
                },
                FailureStat {
                    tool: "bash".into(),
                    error_pattern: "exit 1: <path>/jest: invalid argument".into(),
                    example: "exit 1: /User/c/repo/bar/node_modules/bin/jest: invalid argument"
                        .into(),
                    count: 1,
                    source: Source::Hook,
                }
            ]
        );

        Ok(())
    }

    #[test]
    fn failures_aggregate_per_repo() -> Result<()> {
        let db = Db::open_in_memory()?;
        insert_failures(
            &db,
            vec![
                create_failure(
                    Some(String::from("test")),
                    String::from("bash"),
                    String::from(
                        "exit 1: /User/c/repo/foo/node_modules/bin/cypress: invalid argument",
                    ),
                    Source::Hook,
                ),
                create_failure(
                    Some(String::from("test")),
                    String::from("bash"),
                    String::from(
                        "exit 1: /User/c/repo/bar/node_modules/bin/cypress: invalid argument",
                    ),
                    Source::Hook,
                ),
                create_failure(
                    Some(String::from("test")),
                    String::from("bash"),
                    String::from(
                        "exit 1: /User/c/repo/bar/node_modules/bin/jest: invalid argument",
                    ),
                    Source::Hook,
                ),
                create_failure(
                    None,
                    String::from("bash"),
                    String::from(
                        "exit 1: /User/c/repo/foo/node_modules/bin/cypress: invalid argument",
                    ),
                    Source::Hook,
                ),
            ],
        )?;

        let stats =
            db.get_failure_stats(Utc::now() - TimeDelta::days(1), Scope::Repo("test".into()))?;

        assert_eq!(
            stats,
            vec![
                FailureStat {
                    tool: "bash".into(),
                    error_pattern: "exit 1: <path>/cypress: invalid argument".into(),
                    example: "exit 1: /User/c/repo/foo/node_modules/bin/cypress: invalid argument"
                        .into(),
                    count: 2,
                    source: Source::Hook,
                },
                FailureStat {
                    tool: "bash".into(),
                    error_pattern: "exit 1: <path>/jest: invalid argument".into(),
                    example: "exit 1: /User/c/repo/bar/node_modules/bin/jest: invalid argument"
                        .into(),
                    count: 1,
                    source: Source::Hook,
                }
            ]
        );

        Ok(())
    }

    #[test]
    fn failures_aggregate_per_repo_per_tool() -> Result<()> {
        let db = Db::open_in_memory()?;
        insert_failures(
            &db,
            vec![
                create_failure(
                    Some(String::from("test")),
                    String::from("bash"),
                    String::from(
                        "exit 1: /User/c/repo/foo/node_modules/bin/cypress: invalid argument",
                    ),
                    Source::Hook,
                ),
                create_failure(
                    Some(String::from("test")),
                    String::from("kubernetes_create_or_update"),
                    String::from(
                        "Error from server (AlreadyExists): deployments.apps \"nginx-deployment\" already exists",
                    ),
                    Source::Hook,
                ),
                create_failure(
                    Some(String::from("test")),
                    String::from("bash"),
                    String::from(
                        "exit 1: /User/c/repo/bar/node_modules/bin/jest: invalid argument",
                    ),
                    Source::Hook,
                ),
                create_failure(
                    None,
                    String::from("bash"),
                    String::from(
                        "exit 1: /User/c/repo/foo/node_modules/bin/cypress: invalid argument",
                    ),
                    Source::Hook,
                ),
            ],
        )?;

        let stats =
            db.get_failure_stats(Utc::now() - TimeDelta::days(1), Scope::Repo("test".into()))?;

        assert_eq!(
            stats,
            vec![FailureStat {
                tool: "bash".into(),
                error_pattern: "exit 1: <path>/cypress: invalid argument".into(),
                example: "exit 1: /User/c/repo/foo/node_modules/bin/cypress: invalid argument".into(),
                count: 1,
                source: Source::Hook,
            },
            FailureStat {
                tool: "bash".into(),
                error_pattern: "exit 1: <path>/jest: invalid argument".into(),
                example: "exit 1: /User/c/repo/bar/node_modules/bin/jest: invalid argument".into(),
                count: 1,
                source: Source::Hook,
            },
            FailureStat {
                tool: "kubernetes_create_or_update".into(),
                error_pattern: "Error from server (AlreadyExists): deployments.apps \"nginx-deployment\" already exists".into(),
                example: "Error from server (AlreadyExists): deployments.apps \"nginx-deployment\" already exists".into(),
                count: 1,
                source: Source::Hook,
            }
            ]
        );

        Ok(())
    }
}
