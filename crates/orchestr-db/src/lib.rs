//! SQLite-backed local metadata storage.
//!
//! This crate intentionally owns schema setup and migrations so UI code never
//! reaches into SQLite directly.

use std::path::Path;

use rusqlite::{params, Connection, OptionalExtension, Result};

const INITIAL_SCHEMA_VERSION: i64 = 1;
const INITIAL_MIGRATION: &str = include_str!("../migrations/0001_settings.sql");

pub struct SettingsRepository {
    connection: Connection,
}

impl SettingsRepository {
    pub fn open(database_path: &Path) -> Result<Self> {
        let connection = Connection::open(database_path)?;
        connection.pragma_update(None, "foreign_keys", "ON")?;
        migrate(&connection)?;
        Ok(Self { connection })
    }

    pub fn get(&self, key: &str) -> Result<Option<String>> {
        self.connection
            .query_row("SELECT value FROM settings WHERE key = ?1", [key], |row| {
                row.get(0)
            })
            .optional()
    }

    pub fn set(&self, key: &str, value: &str) -> Result<()> {
        self.connection.execute(
            "INSERT INTO settings (key, value, updated_at) VALUES (?1, ?2, CURRENT_TIMESTAMP)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = CURRENT_TIMESTAMP",
            params![key, value],
        )?;
        Ok(())
    }
}

fn migrate(connection: &Connection) -> Result<()> {
    connection.execute_batch(INITIAL_MIGRATION)?;
    let current_version: Option<i64> = connection
        .query_row(
            "SELECT version FROM schema_migrations ORDER BY version DESC LIMIT 1",
            [],
            |row| row.get(0),
        )
        .optional()?;

    if current_version.unwrap_or_default() < INITIAL_SCHEMA_VERSION {
        connection.execute(
            "INSERT INTO schema_migrations (version) VALUES (?1)",
            [INITIAL_SCHEMA_VERSION],
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::SettingsRepository;
    use std::{
        fs,
        time::{SystemTime, UNIX_EPOCH},
    };

    fn temporary_database_path() -> std::path::PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after Unix epoch")
            .as_nanos();
        std::env::temp_dir().join(format!("orchestr-db-{nonce}.sqlite"))
    }

    #[test]
    fn settings_persist_when_the_database_is_reopened() {
        let database_path = temporary_database_path();
        let repository = SettingsRepository::open(&database_path).expect("database opens");
        repository
            .set("ui.sidebar.collapsed", "true")
            .expect("setting saves");
        drop(repository);

        let reopened = SettingsRepository::open(&database_path).expect("database reopens");
        assert_eq!(
            reopened.get("ui.sidebar.collapsed").expect("setting loads"),
            Some("true".into())
        );
        drop(reopened);

        fs::remove_file(database_path).expect("temporary database removes");
    }
}
