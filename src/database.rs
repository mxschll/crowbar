use std::{env, fs, path::PathBuf, usize, sync::Arc};

use anyhow::Context;
use chrono::{self, Timelike};
use rusqlite::{Connection, Result};

use crate::actions::{
    action_list::ActionList,
    action_factory::ActionFactory,
};

#[derive(Debug, Clone)]
pub enum ActionType {
    /// Binary executable found in the system PATH or with absolute path.
    /// Contains the filename and its full path on the filesystem.
    Program {
        /// Display name of the program
        name: String,
        /// Full path to the executable
        path: PathBuf,
    },
    /// Linux desktop entry (.desktop file).
    /// Contains the application name and the command to execute.
    Desktop {
        /// Name of the application from the desktop entry
        name: String,
        /// The command to execute, as specified in the Exec field
        exec: String,
        /// Whether this desktop entry accepts additional arguments, see ./app_finder.rs
        accepts_args: bool,
    },
}

pub fn insert_action(conn: &Connection, action_name: &str, action_type: ActionType) -> Result<i64> {
    // First, insert or get the program item if it's a program action
    let item_type = match &action_type {
        ActionType::Program { .. } => "program",
        ActionType::Desktop { .. } => "desktop",
    };

    // Insert the action
    conn.execute(
        "INSERT OR IGNORE INTO actions (name, action_type) VALUES (?1, ?2)",
        (action_name, item_type),
    )?;

    let action_id: i64 = conn.query_row(
        "SELECT id FROM actions WHERE name = ?1 AND action_type = ?2",
        (action_name, item_type),
        |row| row.get(0),
    )?;

    match action_type {
        ActionType::Program { name, path } => {
            conn.execute(
                "INSERT OR IGNORE INTO program_items (name, path) VALUES (?1, ?2)",
                (name, path.to_string_lossy().to_string()),
            )?;
        }
        ActionType::Desktop {
            name,
            exec,
            accepts_args,
        } => {
            conn.execute(
                "INSERT OR IGNORE INTO desktop_items (name, exec, accepts_args) VALUES (?1, ?2, ?3)",
                (name, exec, accepts_args),
            )?;
        }
    }

    Ok(action_id)
}

pub fn get_actions(conn: &Connection) -> Result<ActionList> {
    let db = Arc::new(Database { conn: initialize_database()? });
    let factory = ActionFactory::new(db.clone());
    let current_time = chrono::Local::now();
    let current_hour = current_time.hour() as f64;

    // Query that combines execution count with time-based relevance
    let mut stmt = conn.prepare(
        "
            SELECT 
                a.id,
                a.name,
                a.action_type,
                -- Program-specific fields
                p.path as program_path,
                p.name as program_name,
                -- Desktop-specific fields
                d.exec as desktop_exec,
                d.name as desktop_name,
                d.accepts_args as desktop_accepts_args,
                -- Execution statistics
                COUNT(e.id) as execution_count,
                COALESCE(GROUP_CONCAT(strftime('%H', e.execution_timestamp)), '') as execution_hours
            FROM actions a
            -- Join execution history
            LEFT JOIN action_executions e ON a.id = e.action_id
            -- Join action type specific tables
            LEFT JOIN program_items p ON (
                a.action_type = 'program' AND p.name = a.name
            )
            LEFT JOIN desktop_items d ON (
                a.action_type = 'desktop' AND d.name = a.name
            )
            GROUP BY 
                a.id,
                a.name,
                a.action_type,
                program_path,
                program_name,
                desktop_exec,
                desktop_name,
                desktop_accepts_args
        ",
    )?;

    let mut rankings = Vec::new();

    let rows = stmt.query_map([], |row| {
        let action_id: usize = row.get(0)?;
        let action_name: String = row.get(1)?;
        let action_type: String = row.get(2)?;
        let program_path: Option<String> = row.get(3)?;
        let program_name: Option<String> = row.get(4)?;
        let desktop_exec: Option<String> = row.get(5)?;
        let desktop_name: Option<String> = row.get(6)?;
        let desktop_accepts_args: Option<bool> = row.get(7)?;
        let execution_count: i32 = row.get(8)?;
        let hours_str: String = row.get(9)?;

        let hours: Vec<f64> = hours_str
            .split(',')
            .filter_map(|h| h.parse::<f64>().ok())
            .collect();

        let relevance_score =
            calculate_time_relevance(current_hour, &hours, execution_count, &action_type);

        let action_type = match action_type.as_str() {
            "program" => {
                let display_name = program_name.unwrap_or_else(|| action_name.clone());
                let program_path = PathBuf::from(program_path.unwrap());

                factory.create_program_action(
                    action_id,
                    display_name,
                    program_path,
                    execution_count,
                    relevance_score,
                )
            }

            "desktop" => {
                let display_name = desktop_name.unwrap_or_else(|| action_name.clone());
                let program_path = desktop_exec.unwrap();

                factory.create_desktop_action(
                    action_id,
                    display_name,
                    program_path,
                    execution_count,
                    relevance_score,
                )
            }

            _ => panic!("Unknown action type: {}", action_type),
        };

        Ok(action_type)
    })?;

    for row in rows {
        rankings.push(row?);
    }

    Ok(ActionList::new(rankings))
}

/// Calculates a relevance score for an action based on its execution history and time patterns.
///
/// The score is computed using two factors:
/// 1. Time-based weight: Higher weight for actions executed closer to the current hour
/// 2. Execution count: More frequently executed actions get higher scores
///
/// # Arguments
///
/// * `current_hour` - The current hour (0-23)
/// * `hours` - Vec of hours when the action was previously executed
/// * `execution_count` - Total number of times the action has been executed
/// * `action_type` - The action, which affects the relevance score
///
/// # Returns
///
/// A relevance score (≥ 1.0) where higher values indicate greater relevance
///
fn calculate_time_relevance(
    current_hour: f64,
    hours: &[f64],
    execution_count: i32,
    action_type: &str,
) -> usize {
    const BASE_SCORE: usize = 1000; // Base score to ensure non-zero results

    // Desktop items get a 10% boost in relevance
    let type_multiplier = match action_type {
        "desktop" => 1.1,
        _ => 1.0,
    };

    if hours.is_empty() {
        return (BASE_SCORE as f64 * type_multiplier) as usize;
    }

    // Calculate time-based weight
    let time_weights: f64 = hours
        .iter()
        .map(|&hour| {
            let hour_diff = (current_hour - hour).abs();
            // Convert to 0-1 scale where closer hours have higher weight
            if hour_diff > 12.0 {
                0.5 + (24.0 - hour_diff) / 24.0 // Range: 0.5-1.5
            } else {
                0.5 + (12.0 - hour_diff) / 12.0 // Range: 0.5-1.5
            }
        })
        .sum();

    // Calculate average time weight (range: 0.5-1.5)
    let time_factor = time_weights / hours.len() as f64;

    // Base relevance from execution count (always ≥ 1.0)
    let count_factor = 1.0 + (execution_count as f64).ln();

    // Final score combines all factors and converts to usize
    (BASE_SCORE as f64 * count_factor * time_factor * type_multiplier) as usize
}

const TABLE_SCHEMAS: [&str; 5] = [
    "CREATE TABLE actions (
        id INTEGER PRIMARY KEY,
        name TEXT NOT NULL,
        action_type TEXT NOT NULL
    )",
    "CREATE TABLE program_items (
        id INTEGER PRIMARY KEY,
        name TEXT NOT NULL,
        path TEXT NOT NULL,
        UNIQUE(path, name)
    )",
    "CREATE TABLE desktop_items (
        id INTEGER PRIMARY KEY,
        name TEXT NOT NULL,
        exec TEXT NOT NULL,
        accepts_args BOOLEAN NOT NULL DEFAULT 0,
        UNIQUE(exec, name)
    )",
    "CREATE TABLE action_executions (
        id INTEGER PRIMARY KEY,
        action_id INTEGER NOT NULL,
        execution_timestamp TEXT NOT NULL,
        FOREIGN KEY(action_id) REFERENCES actions(id)
    )",
    "CREATE TABLE url_items (
        id INTEGER PRIMARY KEY,
        url TEXT NOT NULL UNIQUE
    )",
];

/// Verifies if database has the correct tables and columns.
fn verify_schema(conn: &Connection) -> Result<bool> {
    let tables = [
        "actions",
        "program_items",
        "desktop_items",
        "action_executions",
        "url_items",
    ];
    let mut schemas = Vec::new();

    for table in tables.iter() {
        let result: Result<Vec<String>> = conn
            .prepare(&format!(
                "SELECT sql FROM sqlite_master WHERE type='table' AND name='{}'",
                table
            ))?
            .query_map([], |row| row.get(0))?
            .collect();
        schemas.push(result);
    }

    let expected_schemas: Vec<String> = TABLE_SCHEMAS
        .iter()
        .map(|s| s.split_whitespace().collect::<String>())
        .collect();

    // Check if all tables exist and match their expected schemas
    for (i, schema_result) in schemas.iter().enumerate() {
        match schema_result {
            Ok(table_schemas) => {
                if let Some(actual_schema) = table_schemas.first() {
                    if actual_schema.split_whitespace().collect::<String>() != expected_schemas[i] {
                        return Ok(false);
                    }
                } else {
                    return Ok(false);
                }
            }
            _ => return Ok(false),
        }
    }
    Ok(true)
}

fn get_database_path() -> Result<PathBuf> {
    let home = env::var("HOME")
        .or_else(|_| env::var("USERPROFILE"))
        .context("Failed to determine home directory")
        .unwrap();

    let config_dir = PathBuf::from(home)
        .join(".local")
        .join("share")
        .join("crowbar");
    fs::create_dir_all(&config_dir)
        .context("Failed to create config directory")
        .unwrap();

    Ok(config_dir.join("crowbar.db"))
}

pub fn initialize_database() -> Result<Connection> {
    // If database exists but schema doesn't match, delete it
    let db_path = &get_database_path().unwrap();

    let conn = Connection::open(db_path)?;
    if !verify_schema(&conn)? {
        drop(conn); // Close connection before removing file
        std::fs::remove_file(db_path).map_err(|e| {
            rusqlite::Error::SqliteFailure(
                rusqlite::ffi::Error::new(1), // Using code 1 for IO error
                Some(e.to_string()),
            )
        })?;
        println!("Creating database.");
    }

    let conn = Connection::open(db_path)?;

    // Create all tables using the schema definitions
    for schema in TABLE_SCHEMAS.iter() {
        let create_stmt = schema.replace("CREATE TABLE", "CREATE TABLE IF NOT EXISTS");
        conn.execute(&create_stmt, ())?;
    }

    Ok(conn)
}

#[derive(Debug)]
pub struct Database {
    conn: Connection,
}

impl Database {
    pub fn new() -> Result<Self> {
        let conn = initialize_database()?;
        Ok(Database { conn })
    }

    pub fn insert_action(&self, action_name: &str, action_type: ActionType) -> Result<i64> {
        insert_action(&self.conn, action_name, action_type)
    }

    pub fn log_execution(&self, action_id: usize) -> Result<()> {
        let timestamp = chrono::Local::now().to_rfc3339();
        
        self.conn.execute(
            "INSERT INTO action_executions (action_id, execution_timestamp) VALUES (?1, ?2)",
            (action_id, timestamp),
        )?;

        Ok(())
    }

    pub fn get_actions(&self) -> Result<ActionList> {
        get_actions(&self.conn)
    }
}
