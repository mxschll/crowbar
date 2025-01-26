use core::error;
use std::{env, fs, path::PathBuf};

use anyhow::Context;
use chrono::{self, Timelike};
use log::{debug, error, info};
use rusqlite::{Connection, Result};
use shlex;
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
            ActionType::Desktop { name, .. } => name,
        }
    }

    pub fn execute(&self, args: Option<Vec<&str>>) {
        let result = match &self.action_type {
            ActionType::Program { path, .. } => {
                let mut cmd = std::process::Command::new(path);
                if let Some(args) = args {
                    cmd.args(args);
                }
                cmd.spawn()
                    .map_err(|e| (path.to_string_lossy().to_string(), e))
            }
            ActionType::Desktop { exec, .. } => {
                let parts: Vec<String> = shlex::split(exec).unwrap_or_else(|| vec![exec.clone()]);

                if parts.is_empty() {
                    error!("Empty command");
                    return ();
                }

                let (command, base_args) = (parts[0].clone(), &parts[1..]);
                let mut cmd = std::process::Command::new(&command);
                cmd.args(base_args);

                if let Some(args) = args {
                    cmd.args(args);
                }

                cmd.spawn().map_err(|e| (exec.clone(), e))
            }
        };

        match result {
            Ok(_) => {
                info!("Launching {}", self.display_name());
                if let Ok(conn) = initialize_database() {
                    let _ = log_execution(&conn, &self);
                }
            }
            Err((cmd, e)) => eprintln!("Failed to start {}: {}", cmd, e),
        }
    }
}

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
    },
}

#[derive(Debug, Clone)]
pub struct ActionRanking {
    pub action: Action,
    pub execution_count: i32,
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
        ActionType::Desktop { name, exec } => {
            conn.execute(
                "INSERT OR IGNORE INTO desktop_items (name, exec) VALUES (?1, ?2)",
                (name, exec),
            )?;
        }
    }

    Ok(action_id)
}

pub fn log_execution(conn: &Connection, action: &Action) -> Result<()> {
    info!("Logging action execution");
    let timestamp = chrono::Local::now().to_rfc3339();

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
                desktop_name
        ",
    )?;

    let mut rankings = Vec::new();

    let rows = stmt.query_map([], |row| {
        let action_id: i64 = row.get(0)?;
        let action_name: String = row.get(1)?;
        let action_type: String = row.get(2)?;
        let program_path: Option<String> = row.get(3)?;
        let program_name: Option<String> = row.get(4)?;
        let desktop_exec: Option<String> = row.get(5)?;
        let desktop_name: Option<String> = row.get(6)?;
        let execution_count: i32 = row.get(7)?;
        let hours_str: String = row.get(8)?;

        let hours: Vec<f64> = hours_str
            .split(',')
            .filter_map(|h| h.parse::<f64>().ok())
            .collect();

        let relevance_score =
            calculate_time_relevance(current_hour, &hours, execution_count, &action_type);

        let action_type = match action_type.as_str() {
            "program" => ActionType::Program {
                name: program_name.unwrap_or_else(|| action_name.clone()),
                path: PathBuf::from(program_path.unwrap_or_default()),
            },
            "desktop" => ActionType::Desktop {
                name: desktop_name.unwrap_or_else(|| action_name.clone()),
                exec: desktop_exec.unwrap_or_default(),
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
) -> f64 {
    // Desktop items get a 10% boost in relevance
    let type_multiplier = match action_type {
        "desktop" => 1.1,
        _ => 1.0,
    };

    if hours.is_empty() {
        return 1.0 * type_multiplier;
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
    count_factor * time_factor * type_multiplier
}

const TABLE_SCHEMAS: [&str; 4] = [
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
        UNIQUE(exec, name)
    )",
    "CREATE TABLE action_executions (
        id INTEGER PRIMARY KEY,
        action_id INTEGER NOT NULL,
        execution_timestamp TEXT NOT NULL,
        FOREIGN KEY(action_id) REFERENCES actions(id)
    )",
];

/// Verifies if database has the correct tables and columns.
fn verify_schema(conn: &Connection) -> Result<bool> {
    let tables = [
        "actions",
        "program_items",
        "desktop_items",
        "action_executions",
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
