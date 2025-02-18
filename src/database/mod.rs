mod models;
mod schema;

use anyhow::{Context, Result};
use rusqlite::Connection;
use std::{env, fs, path::PathBuf};

use crate::actions::{
    action_item::ActionDefinition,
    handlers::{app_handler::AppHandler, bin_handler::BinHandler},
};

pub use models::{DesktopItem, ProgramItem};

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

    pub fn get_actions_filtered(&self, filter: &str) -> Result<Vec<Box<dyn ActionDefinition>>> {
        let mut stmt = self.conn.prepare(
            "
            SELECT 
                a.id,
                a.name,
                a.action_type,
                p.path as program_path,
                d.exec as desktop_exec,
                (
                    -- Base frequency score (number of executions with time decay)
                    SELECT COALESCE(
                        SUM(
                            1.0 / (1.0 + (
                                (julianday('now') - julianday(execution_timestamp)) * 24.0 * 60.0
                            ) / (24.0 * 60.0)
                        )
                    ), 0)
                    FROM action_executions ae
                    WHERE ae.action_id = a.id
                ) * (
                    -- Time of day relevance
                    -- Higher score if current hour matches historical usage patterns
                    1.0 + COALESCE((
                        SELECT 0.5 * COUNT(*)
                        FROM action_executions ae2
                        WHERE ae2.action_id = a.id
                        AND strftime('%H', ae2.execution_timestamp) = strftime('%H', 'now')
                    ), 0)
                ) as rank_score
            FROM actions a
            LEFT JOIN program_items p ON (
                a.action_type = 'program' AND p.id = a.id
            )
            LEFT JOIN desktop_items d ON (
                a.action_type = 'desktop' AND d.id = a.id
            )
            WHERE (
                a.searchname LIKE '%' || ?1 || '%' 
                OR a.name LIKE '%' || ?1 || '%'
            )
            ORDER BY rank_score DESC
            LIMIT 10
            ",
        )?;

        let rows = stmt.query_map([filter], |row| {
            let id: usize = row.get(0)?;
            let action_type: String = row.get(2)?;
            let name: String = row.get(1)?;
            let rank_score: f64 = row.get(5)?;
            let relevance = (rank_score * 1000.0) as usize;

            let handler: Box<dyn ActionDefinition> = match action_type.as_str() {
                "program" => {
                    let path: Option<String> = row.get(3)?;
                    if let Some(path) = path {
                        Box::new(BinHandler {
                            id,
                            path: PathBuf::from(path),
                            name,
                            relevance,
                        })
                    } else {
                        return Err(rusqlite::Error::InvalidColumnType(
                            3,
                            "program_path".into(),
                            rusqlite::types::Type::Text,
                        ));
                    }
                }
                "desktop" => {
                    let exec: Option<String> = row.get(4)?;
                    if let Some(exec) = exec {
                        Box::new(AppHandler {
                            id,
                            command: exec,
                            name,
                            relevance,
                        })
                    } else {
                        return Err(rusqlite::Error::InvalidColumnType(
                            4,
                            "desktop_exec".into(),
                            rusqlite::types::Type::Text,
                        ));
                    }
                }
                _ => panic!("Unknown action type: {}", action_type),
            };

            Ok(handler)
        })?;

        let mut handlers = Vec::new();
        for row in rows {
            handlers.push(row?);
        }

        Ok(handlers)
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
