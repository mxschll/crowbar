use anyhow::Result;
use gpui::{div, Context, Element, ParentElement, Styled};
use rusqlite::{self, Row};
use std::path::PathBuf;
use std::sync::Arc;

use crate::action_list_view::ActionListView;
use crate::actions::action_handler::{
    ActionDefinition, ActionHandler, ActionId, ActionItem, HandlerFactory,
};
use crate::actions::action_ids::EXECUTABLE_HANDLER;
use crate::config::Config;
use crate::database::Database;

// Constant values
const RELEVANCE_BOOST: usize = 30;
const MAX_RESULTS: usize = 10;
const TRIGRAM_SIMILARITY_THRESHOLD: f64 = 0.1;
const FUZZY_MATCH_WEIGHT: f64 = 30.0;

// SQL Queries
const SQL_POPULAR_ACTIONS: &str = "
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
";

const SQL_DIRECT_MATCH: &str = "
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
LIMIT 10
";

const SQL_FUZZY_CANDIDATES: &str = "
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
LIMIT 5
";

/// Factory for creating application handlers
pub struct AppHandlerFactory;

impl HandlerFactory for AppHandlerFactory {
    fn get_id(&self) -> &'static str {
        EXECUTABLE_HANDLER
    }

    fn create_handlers_for_query(
        &self,
        query: &str,
        db: Arc<Database>,
        cx: &mut Context<ActionListView>,
    ) -> Vec<ActionItem> {
        match get_actions_filtered(&db, query) {
            Ok(actions) => actions
                .into_iter()
                .map(|action| action.create_action(db.clone(), cx))
                .collect(),
            Err(_) => Vec::new(),
        }
    }
}

/// Represents the type of executable
#[derive(Clone)]
pub enum ExecutableType {
    /// An application with a command string
    Application(String),
    /// A binary with a specific file path
    Binary(PathBuf),
}

/// Combined handler for both applications and binaries
#[derive(Clone)]
pub struct ExecutableHandler {
    pub id: usize,
    pub name: String,
    pub executable_type: ExecutableType,
    pub relevance: usize,
}

impl ActionHandler for ExecutableHandler {
    fn execute(&self, _input: &str) -> Result<()> {
        match &self.executable_type {
            ExecutableType::Application(command) => {
                let mut parts = command.split_whitespace();
                if let Some(program) = parts.next() {
                    let args: Vec<&str> = parts.collect();
                    std::process::Command::new(program).args(args).spawn()?;
                }
            }
            ExecutableType::Binary(path) => {
                std::process::Command::new(path).spawn()?;
            }
        }
        Ok(())
    }

    fn clone_box(&self) -> Box<dyn ActionHandler> {
        Box::new(self.clone())
    }
}

impl ActionDefinition for ExecutableHandler {
    fn create_action(&self, db: Arc<Database>, cx: &mut Context<ActionListView>) -> ActionItem {
        let config = cx.global::<Config>();
        let text_secondary_color = config.text_secondary_color;
        let execution_count = db.get_execution_count(self.get_id().as_str()).unwrap_or(0);
        let name = self.get_name();

        let (description, detail) = match &self.executable_type {
            ExecutableType::Application(_) => {
                ("Runs Application".to_string(), "Application".to_string())
            }
            ExecutableType::Binary(path) => (
                "Runs Binary".to_string(),
                path.to_string_lossy().to_string(),
            ),
        };

        ActionItem::new(
            self.get_id(),
            self.clone(),
            move || {
                div()
                    .flex()
                    .gap_4()
                    .child(div().flex_none().child(name.clone()))
                    .child(
                        div()
                            .flex_grow()
                            .child(detail.clone())
                            .text_color(text_secondary_color),
                    )
                    .child(
                        div()
                            .child(format!("{}", execution_count))
                            .text_color(text_secondary_color),
                    )
                    .into_any()
            },
            self.relevance,
            RELEVANCE_BOOST,
            db,
        )
    }

    fn get_id(&self) -> ActionId {
        ActionId::Dynamic(self.id)
    }

    fn get_name(&self) -> String {
        self.name.clone()
    }

    fn get_relevance(&self) -> usize {
        self.relevance
    }
}

/// Get filtered actions based on the search query
pub fn get_actions_filtered(db: &Database, filter: &str) -> Result<Vec<Box<dyn ActionDefinition>>> {
    // Skip empty filter case - just return popular items
    if filter.trim().is_empty() {
        return get_popular_actions(db);
    }

    // Process the filter to improve search quality
    let filter = filter.to_lowercase();
    let filter_tokens: Vec<&str> = filter.split_whitespace().collect();

    // Generate trigrams for fuzzy matching
    let filter_trigrams = generate_trigrams(&filter);

    // First try direct matching
    let mut handlers = search_with_direct_match(db, &filter)?;

    // If direct matching didn't find enough results, try fuzzy matching
    if handlers.len() < 5 {
        let fuzzy_matches = search_with_fuzzy_match(db, &filter, &filter_trigrams, &filter_tokens)?;

        // Add only fuzzy matches that aren't already in the results
        for fuzzy_match in fuzzy_matches {
            if !handlers
                .iter()
                .any(|h| matches_action_id(h.get_id(), fuzzy_match.get_id()))
            {
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

    // Limit to MAX_RESULTS
    if handlers.len() > MAX_RESULTS {
        handlers.truncate(MAX_RESULTS);
    }

    Ok(handlers)
}

/// Compare two ActionIds for equality
fn matches_action_id(id1: ActionId, id2: ActionId) -> bool {
    match (id1, id2) {
        (ActionId::Builtin(a), ActionId::Builtin(b)) => a == b,
        (ActionId::Dynamic(a), ActionId::Dynamic(b)) => a == b,
        _ => false,
    }
}

/// Generate trigrams from a string for fuzzy matching
fn generate_trigrams(text: &str) -> Vec<String> {
    let text = text.to_lowercase();
    let chars: Vec<char> = text.chars().collect();

    // Add special padding for words shorter than 3 chars
    if chars.len() < 3 {
        return vec![text.to_string()];
    }

    // Generate trigrams (groups of 3 consecutive characters)
    chars
        .windows(3)
        .map(|window| window.iter().collect::<String>())
        .collect()
}

/// Direct match search using traditional LIKE operators
fn search_with_direct_match(db: &Database, filter: &str) -> Result<Vec<Box<dyn ActionDefinition>>> {
    let mut stmt = db.connection().prepare(SQL_DIRECT_MATCH)?;

    // Use the filter for all the query parameters
    let rows = stmt.query_map([&filter, &filter, &filter, &filter, &filter], |row| {
        row_to_action_definition(db, row, &filter.split_whitespace().collect::<Vec<&str>>())
    })?;

    let mut handlers = Vec::new();
    for row in rows {
        handlers.push(row?);
    }

    Ok(handlers)
}

/// Fuzzy search using trigram similarity
fn search_with_fuzzy_match(
    db: &Database,
    filter: &str,
    filter_trigrams: &[String],
    filter_tokens: &[&str],
) -> Result<Vec<Box<dyn ActionDefinition>>> {
    // Get all potential candidates
    let mut stmt = db.connection().prepare(SQL_FUZZY_CANDIDATES)?;

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
                Ok((result, path, None))
            }
            "desktop" => {
                let exec: Option<String> = row.get(4)?;
                Ok((result, None, exec))
            }
            _ => Err(rusqlite::Error::InvalidColumnType(
                2,
                "action_type".into(),
                rusqlite::types::Type::Text,
            )),
        }
    })?;

    let mut candidates = Vec::new();
    for row_result in rows {
        candidates.push(row_result?);
    }

    // Calculate fuzzy match scores and filter out poor matches
    let mut handlers = Vec::new();

    for ((id, action_type, name, base_score, searchname), path_opt, exec_opt) in candidates {
        // Generate trigrams for the search name
        let name_trigrams = generate_trigrams(&searchname);

        // Calculate similarity score based on trigram overlap
        let similarity = calculate_trigram_similarity(filter_trigrams, &name_trigrams);

        // Calculate final relevance score
        let search_score = calculate_search_score(filter_tokens, &searchname);
        let fuzzy_score = similarity * FUZZY_MATCH_WEIGHT;
        let relevance = (base_score * (1.0 + search_score + fuzzy_score)) as usize;

        // Only include results with reasonable similarity
        if similarity > TRIGRAM_SIMILARITY_THRESHOLD {
            let handler: Box<dyn ActionDefinition> = match action_type.as_str() {
                "program" => {
                    if let Some(path) = path_opt {
                        Box::new(ExecutableHandler {
                            id,
                            name,
                            executable_type: ExecutableType::Binary(PathBuf::from(path)),
                            relevance,
                        })
                    } else {
                        continue;
                    }
                }
                "desktop" => {
                    if let Some(exec) = exec_opt {
                        Box::new(ExecutableHandler {
                            id,
                            name,
                            executable_type: ExecutableType::Application(exec),
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

    // Limit to MAX_RESULTS
    if handlers.len() > MAX_RESULTS {
        handlers.truncate(MAX_RESULTS);
    }

    Ok(handlers)
}

/// Calculate similarity between two sets of trigrams
fn calculate_trigram_similarity(trigrams1: &[String], trigrams2: &[String]) -> f64 {
    if trigrams1.is_empty() || trigrams2.is_empty() {
        return 0.0;
    }

    // Count matching trigrams
    let matches = trigrams1.iter().filter(|t1| trigrams2.contains(t1)).count();

    // Return similarity score (ratio of matches to total unique trigrams)
    let total_unique = trigrams1.len() + trigrams2.len() - matches;
    if total_unique == 0 {
        return 1.0;
    }

    matches as f64 / total_unique as f64
}

/// Helper method to convert a row to an ActionDefinition
fn row_to_action_definition(
    db: &Database,
    row: &Row,
    filter_tokens: &[&str],
) -> rusqlite::Result<Box<dyn ActionDefinition>> {
    let id: usize = row.get(0)?;
    let action_type: String = row.get(2)?;
    let name: String = row.get(1)?;
    let base_score: f64 = row.get(5)?;
    let match_quality: f64 = row.get(6)?;
    let searchname: String = row.get(7)?;

    // Calculate final relevance score combining match quality and usage patterns
    let search_score = calculate_search_score(filter_tokens, &searchname);
    let relevance = ((base_score * match_quality) * (1.0 + search_score)) as usize;

    let handler: Box<dyn ActionDefinition> = match action_type.as_str() {
        "program" => {
            let path: Option<String> = row.get(3)?;
            if let Some(path) = path {
                Box::new(ExecutableHandler {
                    id,
                    name,
                    executable_type: ExecutableType::Binary(PathBuf::from(path)),
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
                Box::new(ExecutableHandler {
                    id,
                    name,
                    executable_type: ExecutableType::Application(exec),
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
        _ => {
            return Err(rusqlite::Error::InvalidColumnType(
                2,
                "action_type".into(),
                rusqlite::types::Type::Text,
            ))
        }
    };

    Ok(handler)
}

/// Helper method to get popular actions when there's no filter
fn get_popular_actions(db: &Database) -> Result<Vec<Box<dyn ActionDefinition>>> {
    let mut stmt = db.connection().prepare(SQL_POPULAR_ACTIONS)?;

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
                    Box::new(ExecutableHandler {
                        id,
                        name,
                        executable_type: ExecutableType::Binary(PathBuf::from(path)),
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
                    Box::new(ExecutableHandler {
                        id,
                        name,
                        executable_type: ExecutableType::Application(exec),
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
            _ => {
                return Err(rusqlite::Error::InvalidColumnType(
                    2,
                    "action_type".into(),
                    rusqlite::types::Type::Text,
                ))
            }
        };

        Ok(handler)
    })?;

    let mut handlers = Vec::new();
    for row_result in rows {
        handlers.push(row_result?);
    }

    Ok(handlers)
}

/// Helper to calculate a more sophisticated search score
fn calculate_search_score(filter_tokens: &[&str], searchname: &str) -> f64 {
    if filter_tokens.is_empty() {
        return 0.0;
    }

    // Count how many tokens match
    let searchname = searchname.to_lowercase();
    let mut matched_tokens = 0.0;

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
