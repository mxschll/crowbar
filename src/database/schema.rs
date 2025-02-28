use anyhow::Result;
use rusqlite::Connection;

pub const CURRENT_VERSION: i32 = 1;

pub const TABLE_SCHEMA_VERSION: &str = "
CREATE TABLE IF NOT EXISTS schema_version (
    version INTEGER NOT NULL
)";

pub const TABLE_ACTIONS: &str = "
CREATE TABLE IF NOT EXISTS actions (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL,
    searchname TEXT NOT NULL,
    action_type TEXT NOT NULL,
    UNIQUE(name, action_type)
)";

pub const TABLE_PROGRAM_ITEMS: &str = "
CREATE TABLE IF NOT EXISTS program_items (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL,
    path TEXT NOT NULL,
    UNIQUE(path, name)
)";

pub const TABLE_DESKTOP_ITEMS: &str = "
CREATE TABLE IF NOT EXISTS desktop_items (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL,
    exec TEXT NOT NULL,
    accepts_args BOOLEAN NOT NULL DEFAULT 0,
    UNIQUE(exec, name)
)";

pub const TABLE_ACTION_EXECUTIONS: &str = "
CREATE TABLE IF NOT EXISTS action_executions (
    action_id TEXT NOT NULL,
    execution_timestamp TEXT NOT NULL,
    FOREIGN KEY(action_id) REFERENCES actions(id)
)";

pub const TABLE_HANDLERS: &str = "
CREATE TABLE IF NOT EXISTS handlers (
    id TEXT PRIMARY KEY,
    enabled BOOLEAN NOT NULL DEFAULT 1
)";

// Schema version migration steps
struct MigrationStep {
    target_version: i32,
    migration_fn: fn(&Connection) -> Result<()>,
}

pub struct Schema;

impl Schema {
    pub fn initialize(conn: &Connection) -> Result<()> {
        // create tables
        Self::create_tables(conn)?;

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
                // Migrate database schema
                Self::migrate_schema(conn, v)?;
                conn.execute("UPDATE schema_version SET version = ?1", [CURRENT_VERSION])?;
            }
            _ => (), // Schema is up to date
        }

        Ok(())
    }

    fn create_tables(conn: &Connection) -> Result<()> {
        // Execute each table creation statement
        conn.execute(TABLE_SCHEMA_VERSION, [])?;
        conn.execute(TABLE_ACTIONS, [])?;
        conn.execute(TABLE_PROGRAM_ITEMS, [])?;
        conn.execute(TABLE_DESKTOP_ITEMS, [])?;
        conn.execute(TABLE_ACTION_EXECUTIONS, [])?;
        conn.execute(TABLE_HANDLERS, [])?;

        Ok(())
    }

    fn migrate_schema(conn: &Connection, current_version: i32) -> Result<()> {
        // Define the migration steps
        let migration_steps = [
            // Add migration steps for future versions
            MigrationStep {
                target_version: 1,
                migration_fn: Self::migrate_to_v1,
            },
        ];

        // Execute migrations in order, skipping those already applied
        for step in migration_steps.iter() {
            if current_version < step.target_version {
                (step.migration_fn)(conn)?;
                println!("Migrated schema to version {}", step.target_version);
            }
        }

        Ok(())
    }

    fn migrate_to_v1(conn: &Connection) -> Result<()> {
        Self::create_tables(conn)?;
        Ok(())
    }
}
