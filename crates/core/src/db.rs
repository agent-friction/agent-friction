use std::convert::AsRef;
use std::fs;
use std::path::{Path, PathBuf};

use rusqlite::Connection;

use crate::error::{Error, Result};
use crate::migrations;

pub struct Db {
    pub(crate) conn: Connection,
}

impl Db {
    const XDG_PREFIX: &str = "agent-friction";
    const DB_FILE: &str = "friction.db";

    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        if let Some(parent) = path.as_ref().parent()
            && !parent.as_os_str().is_empty()
            && parent != Path::new(".")
        {
            fs::create_dir_all(parent)?;
        }

        let mut conn = Connection::open(path)?;
        Self::setup(&mut conn)?;
        Ok(Db { conn })
    }

    pub fn open_in_memory() -> Result<Self> {
        let mut conn = Connection::open_in_memory()?;
        Self::setup(&mut conn)?;
        Ok(Db { conn })
    }

    pub fn default_path() -> Result<PathBuf> {
        xdg::BaseDirectories::with_prefix(Self::XDG_PREFIX)
            .get_data_home()
            .map(|d| d.join(Self::DB_FILE))
            .ok_or(Error::NoDataDir)
    }

    pub(crate) fn conn(&self) -> &Connection {
        &self.conn
    }

    fn setup(conn: &mut Connection) -> Result<()> {
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "busy_timeout", 5000)?;
        conn.pragma_update(None, "synchronous", "NORMAL")?;
        conn.pragma_update(None, "foreign_keys", "ON")?;

        migrations::run(conn)?;

        Ok(())
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn open_in_memory_runs_migrations() -> Result<()> {
        let db = Db::open_in_memory()?;
        assert_eq!(
            db.conn()
                .pragma_query_value(None, "user_version", |f| f.get(0)),
            Ok(migrations::MIGRATIONS.len() as i64)
        );

        Ok(())
    }
}
