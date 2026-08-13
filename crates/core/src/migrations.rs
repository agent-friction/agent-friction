use rusqlite::Connection;

use crate::error::{
    Error::{DatabaseVersion, MigrationVersion},
    Result,
};

pub(crate) const MIGRATIONS: &[&str] = &[include_str!("../migrations/0001_initial.sql")];

pub fn run(conn: &mut Connection) -> Result<()> {
    debug_assert!(MIGRATIONS.len() < i64::MAX as usize);

    let raw: i64 = conn.pragma_query_value(None, "user_version", |r| r.get(0))?;
    let curr_version = usize::try_from(raw).map_err(|_| MigrationVersion(raw))?;

    if curr_version > MIGRATIONS.len() {
        return Err(DatabaseVersion(raw));
    }

    for (i, m) in MIGRATIONS.iter().enumerate().skip(curr_version) {
        let version = i as i64 + 1;
        let tx = conn.transaction()?;
        tx.execute_batch(m)?;
        tx.pragma_update(None, "user_version", version)?;
        tx.commit()?;
    }

    Ok(())
}

#[cfg(test)]
mod test {
    use super::*;
    use rusqlite::Connection;
    #[test]
    fn test_migrations_are_idempotent() -> Result<()> {
        let mut conn = Connection::open_in_memory()?;
        run(&mut conn)?;
        run(&mut conn)?;

        assert_eq!(conn.pragma_query_value(None, "user_version", |r| r.get(0)), Ok(MIGRATIONS.len() as i64));

        Ok(())
    }
}

