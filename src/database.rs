use std::{env, fs, path::PathBuf};

use anyhow::Context;
use chrono::{self, Timelike};
use log::info;
use rusqlite::{Connection, Result};
use strsim::jaro_winkler;

#[derive(Debug, Clone)]
pub struct Action {
    pub id: i64,
    pub name: String,
    pub action_type: ActionType,
}

impl Action {
    fn display_name(&self) -> &str {
        match &self.action_type {
            ActionType::Program { name, .. } => name,
        }
    }

    pub fn execute(&self) {
        match &self.action_type {
            ActionType::Program { path, .. } => match std::process::Command::new(path).spawn() {
                Ok(_) => {
                    info!("Launching {}", self.display_name());
                    let conn = initialize_database().unwrap();
                    let _ = log_execution(&conn, &self);
                }
                Err(e) => eprintln!(
                    "Failed to start {}: {}",
                    path.to_string_lossy().to_string(),
                    e
                ),
            },
        }
    }
}

#[derive(Debug, Clone)]
pub enum ActionType {
    Program { name: String, path: PathBuf },
}

#[derive(Debug, Clone)]
pub struct ActionRanking {
    pub action: Action,
    execution_count: i32,
    relevance_score: f64,
}

#[derive(Debug, Clone)]
pub struct ActionList {
    actions: Vec<ActionRanking>,
}

impl ActionList {
    fn new(actions: Vec<ActionRanking>) -> Self {
        ActionList { actions }
    }

    pub fn is_empty(&self) -> bool {
        self.actions.is_empty()
    }

    pub fn fuzzy_search(self, search_term: &str) -> Self {
        if search_term.is_empty() {
            return self;
        }

        let filtered = self
            .actions
            .into_iter()
            .filter_map(|rank| {
                let similarity = jaro_winkler(
                    &rank.action.name.to_lowercase(),
                    &search_term.to_lowercase(),
                );
                if similarity > 0.6 {
                    Some(ActionRanking {
                        relevance_score: rank.relevance_score * similarity,
                        ..rank
                    })
                } else {
                    None
                }
            })
            .collect();

        ActionList { actions: filtered }
    }

    pub fn ranked(mut self) -> Self {
        self.actions
            .sort_by(|a, b| b.relevance_score.partial_cmp(&a.relevance_score).unwrap());
        self
    }

    pub fn collect(self) -> Vec<ActionRanking> {
        self.actions
    }
}

pub fn insert_action(conn: &Connection, action_name: &str, action_type: ActionType) -> Result<i64> {
    // First, insert or get the program item if it's a program action
    let item_type = match &action_type {
        ActionType::Program { name: _, path: _ } => "program",
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

    if let ActionType::Program { name, path } = action_type {
        conn.execute(
            "INSERT OR IGNORE INTO program_items (name, path) VALUES (?1, ?2)",
            (name, path.to_string_lossy().to_string()),
        )?;
    }

    Ok(action_id)
}

pub fn log_execution(conn: &Connection, action: &Action) -> Result<()> {
    info!("Logging action execution");
    let timestamp = chrono::Local::now().to_rfc3339();

    dbg!(action);

    conn.execute(
        "INSERT INTO action_executions (action_id, execution_timestamp) VALUES (?1, ?2)",
        (action.id, timestamp),
    )?;

    Ok(())
}

pub fn get_actions(conn: &Connection) -> Result<ActionList> {
    let current_time = chrono::Local::now();
    let current_hour = current_time.hour() as f64;

    // Query that combines execution count with time-based relevance
    let mut stmt = conn.prepare(
        "
            WITH action_stats AS (
                SELECT 
                    a.id,
                    a.name,
                    a.action_type,
                    p.path as program_path,
                    p.name as program_name,
                    COUNT(e.id) as execution_count,
                    COALESCE(GROUP_CONCAT(strftime('%H', e.execution_timestamp)), '') as execution_hours
                FROM actions a
                LEFT JOIN action_executions e ON a.id = e.action_id
                LEFT JOIN program_items p ON (
                    a.action_type = 'program' 
                    AND p.name = a.name
                )
                GROUP BY a.id, a.name, a.action_type, p.path, p.name
            )
            SELECT 
                id,
                name,
                action_type,
                program_path,
                program_name,
                execution_count,
                execution_hours
            FROM action_stats
        ",
    )?;

    let mut rankings = Vec::new();

    let rows = stmt.query_map([], |row| {
        let action_id: i64 = row.get(0)?;
        let action_name: String = row.get(1)?;
        let action_type: String = row.get(2)?;
        let program_path: Option<String> = row.get(3)?;
        let program_name: Option<String> = row.get(4)?;
        let execution_count: i32 = row.get(5)?;
        let hours_str: String = row.get(6)?;

        let hours: Vec<f64> = hours_str
            .split(',')
            .filter_map(|h| h.parse::<f64>().ok())
            .collect();

        let relevance_score = calculate_time_relevance(current_hour, &hours, execution_count);

        let action_type = match action_type.as_str() {
            "program" => ActionType::Program {
                name: program_name.unwrap_or_else(|| action_name.clone()),
                path: PathBuf::from(program_path.unwrap_or_default()),
            },
            _ => panic!("Unknown action type: {}", action_type),
        };

        Ok(ActionRanking {
            action: Action {
                id: action_id,
                name: action_name,
                action_type,
            },
            execution_count,
            relevance_score,
        })
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
///
/// # Returns
///
/// A relevance score (≥ 1.0) where higher values indicate greater relevance
///
fn calculate_time_relevance(current_hour: f64, hours: &[f64], execution_count: i32) -> f64 {
    if hours.is_empty() {
        return 1.0;
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

    // Final score combines both factors, ensuring result is ≥ 1.0
    count_factor * time_factor
}

/// Verifies if database has the correct tables and columns.
fn verify_schema(conn: &Connection) -> Result<bool> {
    // Check if tables exist and have correct columns
    let actions_result: Result<Vec<String>> = conn
        .prepare("SELECT sql FROM sqlite_master WHERE type='table' AND name='actions'")?
        .query_map([], |row| row.get(0))?
        .collect();

    let program_items_result: Result<Vec<String>> = conn
        .prepare("SELECT sql FROM sqlite_master WHERE type='table' AND name='program_items'")?
        .query_map([], |row| row.get(0))?
        .collect();

    let action_executions_result: Result<Vec<String>> = conn
        .prepare("SELECT sql FROM sqlite_master WHERE type='table' AND name='action_executions'")?
        .query_map([], |row| row.get(0))?
        .collect();

    let expected_actions_schema = "CREATE TABLE actions (
            id INTEGER PRIMARY KEY,
            name TEXT NOT NULL,
            action_type TEXT NOT NULL
        )"
    .split_whitespace()
    .collect::<String>();

    let expected_program_items_schema = "CREATE TABLE program_items (
            id INTEGER PRIMARY KEY,
            path TEXT NOT NULL,
            name TEXT NOT NULL,
            UNIQUE(path, name)
        )"
    .split_whitespace()
    .collect::<String>();

    let expected_action_executions_schema = "CREATE TABLE action_executions (
            id INTEGER PRIMARY KEY,
            action_id INTEGER NOT NULL,
            execution_timestamp TEXT NOT NULL,
            FOREIGN KEY(action_id) REFERENCES actions(id)
        )"
    .split_whitespace()
    .collect::<String>();

    match (
        actions_result,
        program_items_result,
        action_executions_result,
    ) {
        (Ok(actions_schemas), Ok(program_items_schemas), Ok(action_executions_schemas)) => {
            if let (
                Some(actions_schema),
                Some(program_items_schema),
                Some(action_executions_schema),
            ) = (
                actions_schemas.first(),
                program_items_schemas.first(),
                action_executions_schemas.first(),
            ) {
                return Ok(actions_schema.split_whitespace().collect::<String>()
                    == expected_actions_schema
                    && program_items_schema.split_whitespace().collect::<String>()
                        == expected_program_items_schema
                    && action_executions_schema
                        .split_whitespace()
                        .collect::<String>()
                        == expected_action_executions_schema);
            }
        }
        _ => return Ok(false),
    }
    Ok(false)
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

    conn.execute(
        "CREATE TABLE IF NOT EXISTS actions (
            id INTEGER PRIMARY KEY,
            name TEXT NOT NULL,
            action_type TEXT NOT NULL
        )",
        (),
    )?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS program_items (
            id INTEGER PRIMARY KEY,
            path TEXT NOT NULL,
            name TEXT NOT NULL,
            UNIQUE(path, name)
        )",
        (),
    )?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS action_executions (
            id INTEGER PRIMARY KEY,
            action_id INTEGER NOT NULL,
            execution_timestamp TEXT NOT NULL,
            FOREIGN KEY(action_id) REFERENCES actions(id)
        )",
        (),
    )?;

    Ok(conn)
}
