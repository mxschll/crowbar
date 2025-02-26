use anyhow::Result;
use rusqlite::Connection;

pub const CURRENT_VERSION: i32 = 1;

pub struct Schema;

impl Schema {
    pub fn initialize(conn: &Connection) -> Result<()> {
        // Create version table if it doesn't exist
        conn.execute(
            "CREATE TABLE IF NOT EXISTS schema_version (version INTEGER NOT NULL)",
            [],
        )?;

        // Get current version
        let version: Option<i32> = conn
            .query_row("SELECT version FROM schema_version", [], |row| row.get(0))
            .ok();

        match version {
            None => {
                // First time initialization
                Self::create_tables(conn)?;
                conn.execute(
                    "INSERT INTO schema_version (version) VALUES (?1)",
                    [CURRENT_VERSION],
                )?;
            }
            Some(v) if v < CURRENT_VERSION => {
                conn.execute("UPDATE schema_version SET version = ?1", [CURRENT_VERSION])?;
            }
            _ => (), // Schema is up to date
        }

        Ok(())
    }

    fn create_tables(conn: &Connection) -> Result<()> {
        conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS actions (
                id INTEGER PRIMARY KEY,
                name TEXT NOT NULL,
                searchname TEXT NOT NULL,
                action_type TEXT NOT NULL,
                UNIQUE(name, action_type)
            );

            CREATE TABLE IF NOT EXISTS program_items (
                id INTEGER PRIMARY KEY,
                name TEXT NOT NULL,
                path TEXT NOT NULL,
                UNIQUE(path, name)
            );

            CREATE TABLE IF NOT EXISTS desktop_items (
                id INTEGER PRIMARY KEY,
                name TEXT NOT NULL,
                exec TEXT NOT NULL,
                accepts_args BOOLEAN NOT NULL DEFAULT 0,
                UNIQUE(exec, name)
            );

            CREATE TABLE IF NOT EXISTS action_executions (
                action_id TEXT NOT NULL,
                execution_timestamp TEXT NOT NULL,
                FOREIGN KEY(action_id) REFERENCES actions(id)
            );
           ",
        )?;

        Ok(())
    }
}
