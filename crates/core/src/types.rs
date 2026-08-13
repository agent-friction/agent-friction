use serde::{Deserialize, Serialize};
use rusqlite::types::{FromSql, FromSqlError, FromSqlResult, ToSql, ToSqlOutput, ValueRef};

use crate::error::Error::Parse;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Decision {
    AllowOnce,
    AllowAlways,
    Deny,
}

impl std::str::FromStr for Decision {
    type Err = crate::error::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            Decision::ALLOW_ONCE => Ok(Decision::AllowOnce),
            Decision::ALLOW_ALWAYS => Ok(Decision::AllowAlways),
            Decision::DENY => Ok(Decision::Deny),
            _ => Err(Parse {
                kind: "decision",
                value: s.to_string(),
            }),
        }
    }
}

impl std::fmt::Display for Decision {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl ToSql for Decision {
    fn to_sql(&self) -> rusqlite::Result<rusqlite::types::ToSqlOutput<'_>> {
        Ok(ToSqlOutput::from(self.as_str()))
    }
}

impl FromSql for Decision {
    fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
        let text = value.as_str()?;
        text.parse::<Decision>().map_err(FromSqlError::other)
    }
}

impl Decision {
    const ALLOW_ONCE: &str = "allow_once";
    const ALLOW_ALWAYS: &str = "allow_always";
    const DENY: &str = "deny";

    pub fn as_str(&self) -> &'static str {
        match self {
            Decision::AllowOnce => Self::ALLOW_ONCE,
            Decision::AllowAlways => Self::ALLOW_ALWAYS,
            Decision::Deny => Self::DENY,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Source {
    Hook,
    Model,
}

impl std::str::FromStr for Source {
    type Err = crate::error::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            Source::HOOK => Ok(Source::Hook),
            Source::MODEL => Ok(Source::Model),
            _ => Err(Parse {
                kind: "source",
                value: s.to_string(),
            }),
        }
    }
}

impl std::fmt::Display for Source {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl ToSql for Source {
    fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
        Ok(ToSqlOutput::from(self.as_str()))
    }
}

impl FromSql for Source {
    fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
        let text = value.as_str()?;
        text.parse::<Source>().map_err(FromSqlError::other)
    }
}

impl Source {
    const HOOK: &str = "hook";
    const MODEL: &str = "model";

    pub fn as_str(&self) -> &'static str {
        match self {
            Source::Hook => Self::HOOK,
            Source::Model => Self::MODEL,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PermissionEvent {
    pub id: Option<i64>,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub agent: String,
    pub session_id: Option<String>,
    pub repo: Option<String>,
    pub model: Option<String>,
    pub tool: String,
    pub pattern: String,
    pub decision: Decision,
    pub context: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ToolFailure {
    pub id: Option<i64>,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub agent: String,
    pub session_id: Option<String>,
    pub repo: Option<String>,
    pub model: Option<String>,
    pub tool: String,
    pub input: Option<serde_json::Value>,
    pub error: String,
    pub source: Source,
    pub context: Option<serde_json::Value>,
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_decision_from_str_display_round_trip() -> Result<(), crate::error::Error> {
        for d in [Decision::AllowOnce, Decision::AllowAlways, Decision::Deny] {
            assert_eq!(d.to_string().parse::<Decision>()?, d);
            assert_eq!(serde_json::to_value(d)?, serde_json::json!(d.to_string()));
        }
        Ok(())
    }

    #[test]
    fn test_source_from_str_display_round_trip() -> Result<(), crate::error::Error> {
        for s in [Source::Hook, Source::Model] {
            assert_eq!(s.to_string().parse::<Source>()?, s);
            assert_eq!(serde_json::to_value(s)?, serde_json::json!(s.to_string()));
        }
        Ok(())
    }
}
