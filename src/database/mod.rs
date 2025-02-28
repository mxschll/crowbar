mod models;
mod schema;

use anyhow::{Context, Result};
use rusqlite::Connection;
use std::{env, fs, path::PathBuf};

pub use models::{ActionHandlerModel, DesktopItem, ProgramItem};

#[derive(Debug)]
pub struct Database {
    conn: Connection,
}

impl Database {
    pub fn new() -> Result<Self> {
        let conn = Self::initialize_database()?;
        Ok(Database { conn })
    }

    pub fn connection(&self) -> &Connection {
        &self.conn
    }

    pub fn insert_binary(&self, name: &str, path: &str) -> Result<i64> {
        ProgramItem::insert(&self.conn, name, path)
    }

    pub fn insert_application(&self, name: &str, exec: &str) -> Result<i64> {
        DesktopItem::insert(&self.conn, name, exec, true)
    }

    pub fn set_handler_enabled(&self, handler_id: &str, enabled: bool) -> Result<()> {
        ActionHandlerModel::set_enabled(&self.conn, handler_id, enabled)?;
        Ok(())
    }

    pub fn log_execution(&self, action_id: &str) -> Result<()> {
        let timestamp = chrono::Local::now().to_rfc3339();
        self.conn.execute(
            "INSERT INTO action_executions (action_id, execution_timestamp) VALUES (?1, ?2)",
            (action_id, timestamp),
        )?;
        Ok(())
    }

    pub fn get_execution_count(&self, action_id: &str) -> Result<i32> {
        let count: i32 = self.conn.query_row(
            "SELECT COUNT(*) FROM action_executions WHERE action_id = ?1",
            [action_id],
            |row| row.get(0),
        )?;
        Ok(count)
    }

    pub fn get_action_relevance(&self, action_id: &str) -> Result<(usize, i32)> {
        let (rank_score, count): (f64, i32) = self.conn.query_row(
            "
            WITH action_stats AS (
                SELECT 
                    -- Base frequency score (number of executions with time decay)
                    COALESCE(
                        SUM(
                            1.0 / (1.0 + (
                                (julianday('now') - julianday(execution_timestamp)) * 24.0 * 60.0
                            ) / (24.0 * 60.0)
                        )
                    ), 0) as base_score,
                    COUNT(*) as execution_count,
                    -- Time of day relevance
                    COALESCE((
                        SELECT 0.5 * COUNT(*)
                        FROM action_executions ae2
                        WHERE ae2.action_id = ?1
                        AND strftime('%H', ae2.execution_timestamp) = strftime('%H', 'now')
                    ), 0) as time_bonus
                FROM action_executions
                WHERE action_id = ?1
            )
            SELECT 
                (base_score * (1.0 + time_bonus)) as rank_score,
                execution_count
            FROM action_stats",
            [action_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?;

        Ok(((rank_score * 1000.0) as usize, count))
    }

    fn initialize_database() -> Result<Connection> {
        let db_path = Self::get_database_path()?;
        let conn = Connection::open(&db_path)?;

        // Initialize schema
        schema::Schema::initialize(&conn)?;

        Ok(conn)
    }

    fn get_database_path() -> Result<PathBuf> {
        let home = env::var("HOME")
            .or_else(|_| env::var("USERPROFILE"))
            .context("Failed to determine home directory")?;

        let config_dir = PathBuf::from(home)
            .join(".local")
            .join("share")
            .join("crowbar");

        fs::create_dir_all(&config_dir).context("Failed to create config directory")?;

        Ok(config_dir.join("crowbar.db"))
    }
}
