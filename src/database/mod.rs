mod models;
mod schema;

use anyhow::{Context, Result};
use rusqlite::Connection;
use std::{env, fs, path::PathBuf};

use crate::actions::{
    action_handler::{ActionDefinition, ActionId},
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
        // Skip empty filter case - just return popular items
        if filter.trim().is_empty() {
            return self.get_popular_actions();
        }
        
        // Process the filter to improve search quality
        let filter = filter.to_lowercase();
        let filter_tokens: Vec<&str> = filter
            .split_whitespace()
            .collect();
        
        // Generate trigrams for fuzzy matching
        let filter_trigrams = self.generate_trigrams(&filter);
        
        let mut handlers = Vec::new();
        
        // First try direct matching
        let direct_matches = self.search_with_direct_match(&filter)?;
        handlers.extend(direct_matches);
        
        // If direct matching didn't find enough results, try fuzzy matching
        if handlers.len() < 5 {
            let fuzzy_matches = self.search_with_fuzzy_match(&filter, &filter_trigrams, &filter_tokens)?;
            // Add only fuzzy matches that aren't already in the results
            for fuzzy_match in fuzzy_matches {
                if !handlers.iter().any(|h| {
                    let h_id = h.get_id();
                    let fm_id = fuzzy_match.get_id();
                    match (h_id, fm_id) {
                        (ActionId::Builtin(a), ActionId::Builtin(b)) => a == b,
                        (ActionId::Dynamic(a), ActionId::Dynamic(b)) => a == b,
                        _ => false,
                    }
                }) {
                    handlers.push(fuzzy_match);
                }
            }
        }
        
        // Sort by relevance
        handlers.sort_by(|a, b| {
            // First compare by relevance, then by name if relevance is equal
            let relevance_comparison = b.get_relevance().cmp(&a.get_relevance());
            if relevance_comparison == std::cmp::Ordering::Equal {
                a.get_name().cmp(&b.get_name())
            } else {
                relevance_comparison
            }
        });
        
        // Limit to 10 results
        if handlers.len() > 10 {
            handlers.truncate(10);
        }
        
        Ok(handlers)
    }

    // Generate trigrams from a string for fuzzy matching
    fn generate_trigrams(&self, text: &str) -> Vec<String> {
        let text = text.to_lowercase();
        let chars: Vec<char> = text.chars().collect();
        let mut trigrams = Vec::new();
        
        // Add special padding for words shorter than 3 chars
        if chars.len() < 3 {
            trigrams.push(text.to_string());
            return trigrams;
        }
        
        // Generate trigrams (groups of 3 consecutive characters)
        for i in 0..chars.len() - 2 {
            let trigram: String = chars[i..i+3].iter().collect();
            trigrams.push(trigram);
        }
        
        trigrams
    }

    // Direct match search using traditional LIKE operators
    fn search_with_direct_match(&self, filter: &str) -> Result<Vec<Box<dyn ActionDefinition>>> {
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
                    1.0 + COALESCE((
                        SELECT 0.5 * COUNT(*)
                        FROM action_executions ae2
                        WHERE ae2.action_id = a.id
                        AND strftime('%H', ae2.execution_timestamp) = strftime('%H', 'now')
                    ), 0)
                ) as base_score,
                -- Match quality scoring
                CASE
                    -- Exact match - highest priority
                    WHEN a.searchname = ? THEN 100.0
                    -- Starts with - high priority (prefix match)
                    WHEN a.searchname LIKE ? || '%' THEN 50.0
                    -- Contains all tokens - medium priority
                    WHEN a.searchname LIKE '%' || ? || '%' THEN 10.0
                    -- Partial match - lower priority
                    ELSE 1.0
                END as match_quality,
                a.searchname
            FROM actions a
            LEFT JOIN program_items p ON (
                a.action_type = 'program' AND p.id = a.id
            )
            LEFT JOIN desktop_items d ON (
                a.action_type = 'desktop' AND d.id = a.id
            )
            WHERE (
                -- Matching logic
                a.searchname LIKE '%' || ? || '%' 
                OR a.name LIKE '%' || ? || '%'
            )
            ORDER BY match_quality DESC, base_score DESC
            LIMIT 5
            ",
        )?;

        // Use the filter for all the query parameters
        let rows = stmt.query_map([&filter, &filter, &filter, &filter, &filter], |row| {
            self.row_to_action_definition(row, &filter.split_whitespace().collect::<Vec<&str>>())
        })?;

        let mut handlers = Vec::new();
        for row in rows {
            handlers.push(row?);
        }

        Ok(handlers)
    }

    // Fuzzy search using trigram similarity
    fn search_with_fuzzy_match(&self, filter: &str, filter_trigrams: &[String], filter_tokens: &[&str]) -> Result<Vec<Box<dyn ActionDefinition>>> {
        // Get all potential candidates
        let mut stmt = self.conn.prepare(
            "
            SELECT 
                a.id,
                a.name,
                a.action_type,
                p.path as program_path,
                d.exec as desktop_exec,
                (
                    SELECT COALESCE(
                        SUM(
                            1.0 / (1.0 + (
                                (julianday('now') - julianday(execution_timestamp)) * 24.0 * 60.0
                            ) / (24.0 * 60.0)
                        )
                    ), 0)
                    FROM action_executions ae
                    WHERE ae.action_id = a.id
                ) as base_score,
                a.searchname
            FROM actions a
            LEFT JOIN program_items p ON (
                a.action_type = 'program' AND p.id = a.id
            )
            LEFT JOIN desktop_items d ON (
                a.action_type = 'desktop' AND d.id = a.id
            )
            ORDER BY base_score DESC
            LIMIT 100
            ",
        )?;

        let rows = stmt.query_map([], |row| {
            let id: usize = row.get(0)?;
            let action_type: String = row.get(2)?;
            let name: String = row.get(1)?;
            let base_score: f64 = row.get(5)?;
            let searchname: String = row.get(6)?;
            
            // Calculate fuzzy match score later
            let result = (id, action_type.clone(), name, base_score, searchname);
            
            match action_type.as_str() {
                "program" => {
                    let path: Option<String> = row.get(3)?;
                    if path.is_none() {
                        return Err(rusqlite::Error::InvalidColumnType(
                            3,
                            "program_path".into(),
                            rusqlite::types::Type::Text,
                        ));
                    }
                    Ok((result, path, None))
                }
                "desktop" => {
                    let exec: Option<String> = row.get(4)?;
                    if exec.is_none() {
                        return Err(rusqlite::Error::InvalidColumnType(
                            4,
                            "desktop_exec".into(),
                            rusqlite::types::Type::Text,
                        ));
                    }
                    Ok((result, None, exec))
                }
                _ => panic!("Unknown action type: {}", action_type),
            }
        })?;

        let mut candidates = Vec::new();
        for row in rows {
            candidates.push(row?);
        }
        
        // Calculate fuzzy match scores and filter out poor matches
        let mut handlers = Vec::new();
        
        for ((id, action_type, name, base_score, searchname), path_opt, exec_opt) in candidates {
            // Generate trigrams for the search name
            let name_trigrams = self.generate_trigrams(&searchname);
            
            // Calculate similarity score based on trigram overlap
            let similarity = self.calculate_trigram_similarity(filter_trigrams, &name_trigrams);
            
            // Calculate final relevance score
            let search_score = self.calculate_search_score(filter_tokens, &searchname);
            let fuzzy_score = similarity * 30.0; // Weight for fuzzy matching
            let relevance = ((base_score * (1.0 + search_score + fuzzy_score))) as usize;
            
            // Only include results with reasonable similarity
            if similarity > 0.1 {
                let handler: Box<dyn ActionDefinition> = match action_type.as_str() {
                    "program" => {
                        if let Some(path) = path_opt {
                            Box::new(BinHandler {
                                id,
                                path: PathBuf::from(path),
                                name: name.clone(),
                                relevance,
                            })
                        } else {
                            continue;
                        }
                    }
                    "desktop" => {
                        if let Some(exec) = exec_opt {
                            Box::new(AppHandler {
                                id,
                                command: exec,
                                name: name.clone(),
                                relevance,
                            })
                        } else {
                            continue;
                        }
                    }
                    _ => continue,
                };
                
                handlers.push(handler);
            }
        }
        
        // Sort by relevance score (higher is better)
        handlers.sort_by(|a, b| b.get_relevance().cmp(&a.get_relevance()));
        
        // Limit to 10 results
        if handlers.len() > 10 {
            handlers.truncate(10);
        }
        
        Ok(handlers)
    }

    // Calculate similarity between two sets of trigrams
    fn calculate_trigram_similarity(&self, trigrams1: &[String], trigrams2: &[String]) -> f64 {
        if trigrams1.is_empty() || trigrams2.is_empty() {
            return 0.0;
        }
        
        // Count matching trigrams
        let mut matches = 0;
        
        for t1 in trigrams1 {
            if trigrams2.contains(t1) {
                matches += 1;
            }
        }
        
        // Return similarity score (ratio of matches to total unique trigrams)
        let total_unique = trigrams1.len() + trigrams2.len() - matches;
        if total_unique == 0 {
            return 1.0;
        }
        
        matches as f64 / total_unique as f64
    }

    // Helper method to convert a row to an ActionDefinition
    fn row_to_action_definition(&self, row: &rusqlite::Row, filter_tokens: &[&str]) -> rusqlite::Result<Box<dyn ActionDefinition>> {
        let id: usize = row.get(0)?;
        let action_type: String = row.get(2)?;
        let name: String = row.get(1)?;
        let base_score: f64 = row.get(5)?;
        let match_quality: f64 = row.get(6)?;
        let searchname: String = row.get(7)?;
        
        // Calculate final relevance score combining match quality and usage patterns
        let search_score = self.calculate_search_score(filter_tokens, &searchname);
        let relevance = ((base_score * match_quality) * (1.0 + search_score)) as usize;

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
    }

    // Helper method to get popular actions when there's no filter
    fn get_popular_actions(&self) -> Result<Vec<Box<dyn ActionDefinition>>> {
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
                ) as rank_score
            FROM actions a
            LEFT JOIN program_items p ON (
                a.action_type = 'program' AND p.id = a.id
            )
            LEFT JOIN desktop_items d ON (
                a.action_type = 'desktop' AND d.id = a.id
            )
            ORDER BY rank_score DESC
            LIMIT 10
            ",
        )?;

        let rows = stmt.query_map([], |row| {
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

    // Helper to calculate a more sophisticated search score
    fn calculate_search_score(&self, filter_tokens: &[&str], searchname: &str) -> f64 {
        if filter_tokens.is_empty() {
            return 0.0;
        }
        
        // Count how many tokens match
        let mut matched_tokens = 0.0;
        let searchname = searchname.to_lowercase();
        
        for token in filter_tokens {
            // Check if token is in the searchname
            if searchname.contains(token) {
                matched_tokens += 1.0;
                
                // Bonus for tokens that are at the start of words
                if searchname.starts_with(token) {
                    matched_tokens += 0.5;
                } else {
                    // Check if token is at the start of any word
                    for word in searchname.split_whitespace() {
                        if word.starts_with(token) {
                            matched_tokens += 0.3;
                            break;
                        }
                    }
                }
            }
        }
        
        // Calculate the final score as a percentage of matched tokens
        matched_tokens / filter_tokens.len() as f64
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
