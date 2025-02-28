use anyhow::{anyhow, Result};
use gpui::{div, Context, Element, ParentElement, Styled};
use log::{debug, info};
use rusqlite::{Connection, OpenFlags};
use std::collections::{HashMap, HashSet};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::action_list_view::ActionListView;
use crate::actions::action_handler::{
    ActionDefinition, ActionHandler, ActionId, ActionItem, HandlerFactory,
};
use crate::actions::action_ids::BROWSER_HISTORY;
use crate::config::Config;
use crate::database::Database;

pub struct BrowserHistoryHandlerFactory;

impl HandlerFactory for BrowserHistoryHandlerFactory {
    fn get_id(&self) -> &'static str {
       BROWSER_HISTORY 
    }

    fn create_handlers_for_query(
        &self,
        query: &str,
        db: Arc<Database>,
        cx: &mut Context<ActionListView>,
    ) -> Vec<ActionItem> {
        BrowserHistoryFactory::create_actions_for_query(query, db, cx)
    }
}

/// Represents a browser history entry across different browsers
#[derive(Debug, Clone)]
pub struct HistoryEntry {
    pub title: String,
    pub url: String,
    pub visit_count: i64,
    pub last_visit: i64,
}

/// Type of browser
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum BrowserType {
    Firefox,
    Chrome,
    Chromium,
    Brave,
    Opera,
    OperaDeveloper,
    Vivaldi,
}

/// Installation type for browsers
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum InstallType {
    Standard,
    Snap,
    Flatpak,
}

/// Cache for browser history entries
lazy_static::lazy_static! {
    static ref HISTORY_CACHE: Mutex<Option<Vec<HistoryEntry>>> = Mutex::new(None);
    static ref LAST_CACHE_UPDATE: Mutex<SystemTime> = Mutex::new(UNIX_EPOCH);
}

// ============================================================================
// Browser History Handler - Main Handler
// ============================================================================

/// Handler for browser history actions
#[derive(Clone)]
pub struct BrowserHistoryHandler {
    entry: Option<HistoryEntry>,
}

impl BrowserHistoryHandler {
    pub fn new() -> Self {
        Self { entry: None }
    }

    pub fn with_entry(entry: HistoryEntry) -> Self {
        Self { entry: Some(entry) }
    }

    /// Get history entries for a specific search query
    pub fn get_history_entries_for_query(query: &str) -> Vec<HistoryEntry> {
        // Only use cache for empty queries
        if query.is_empty() {
            let cache_mutex = HISTORY_CACHE.lock().unwrap();
            let last_update_mutex = LAST_CACHE_UPDATE.lock().unwrap();

            // Check if cache is still valid (less than 5 minutes old)
            if cache_mutex.is_some()
                && last_update_mutex
                    .elapsed()
                    .unwrap_or(Duration::from_secs(600))
                    < Duration::from_secs(300)
            {
                return cache_mutex.clone().unwrap_or_default();
            }
            drop(cache_mutex);
            drop(last_update_mutex);

            // Cache is invalid or doesn't exist, refresh it
            let entries = Self::refresh_history_cache("");

            // Update the cache
            let mut cache = HISTORY_CACHE.lock().unwrap();
            *cache = Some(entries.clone());

            // Update the last cache update time
            let mut last_update = LAST_CACHE_UPDATE.lock().unwrap();
            *last_update = SystemTime::now();

            entries
        } else {
            // For specific queries, always get fresh results
            Self::refresh_history_cache(query)
        }
    }

    /// Refresh the history cache by collecting entries from all browsers
    fn refresh_history_cache(query: &str) -> Vec<HistoryEntry> {
        if query.is_empty() {
            info!("Refreshing browser history cache");
        } else {
            info!("Searching browser history for query: '{}'", query);
        }

        let entries = HistoryCollector::collect_all_browser_histories(query);

        // Remove duplicate URLs across browsers and sort by recency
        let unique_entries = Self::deduplicate_entries(entries);

        info!(
            "Found {} unique browser history entries across all browsers",
            unique_entries.len()
        );
        unique_entries
    }

    /// Deduplicate history entries from different browsers, keeping the most recent version of each URL
    fn deduplicate_entries(entries: Vec<HistoryEntry>) -> Vec<HistoryEntry> {
        let mut unique_entries = Vec::new();
        let mut seen_urls = HashSet::new();

        // Sort all entries by last_visit timestamp (descending)
        let mut all_entries = entries;
        all_entries.sort_by(|a, b| b.last_visit.cmp(&a.last_visit));

        // Keep only the first occurrence of each URL (which will be the most recent due to sorting)
        for entry in all_entries {
            if !seen_urls.contains(&entry.url) {
                seen_urls.insert(entry.url.clone());
                unique_entries.push(entry);
            }
        }

        unique_entries
    }
}

// Implementation of ActionHandler trait
impl ActionHandler for BrowserHistoryHandler {
    fn execute(&self, _input: &str) -> anyhow::Result<()> {
        if let Some(entry) = &self.entry {
            // Open the URL in the default browser
            open::that(&entry.url)?;
            Ok(())
        } else {
            Err(anyhow!("No history entry to execute"))
        }
    }

    fn clone_box(&self) -> Box<dyn ActionHandler> {
        Box::new(self.clone())
    }
}

// Implementation of ActionDefinition trait
impl ActionDefinition for BrowserHistoryHandler {
    fn create_action(&self, db: Arc<Database>, cx: &mut Context<ActionListView>) -> ActionItem {
        let config = cx.global::<Config>();
        let text_secondary_color = config.text_secondary_color;

        // The main handler doesn't have a specific entry
        // Each entry will create its own handler when filtering
        ActionItem::new(
            self.get_id(),
            self.clone(),
            move || {
                div()
                    .flex()
                    .gap_4()
                    .child(div().flex_none().child("Browser History"))
                    .child(
                        div()
                            .flex_grow()
                            .child("History Handler")
                            .text_color(text_secondary_color),
                    )
                    .into_any()
            },
            0,
            0,
            db,
        )
    }

    fn get_id(&self) -> ActionId {
        ActionId::Builtin("browser-history")
    }

    fn get_name(&self) -> String {
        "Browser History".to_string()
    }
}

// ============================================================================
// History Collector - Manages retrieving history from various browsers
// ============================================================================

/// Collects browser history from all supported browsers
struct HistoryCollector;

impl HistoryCollector {
    /// Collect history from all browser types
    fn collect_all_browser_histories(search_term: &str) -> Vec<HistoryEntry> {
        let mut entries = Vec::new();

        // Define all supported browsers
        let browsers = Self::get_supported_browsers();

        // Collect history from each browser
        for (browser_type, browser_paths) in browsers {
            if let Ok(browser_entries) =
                Self::get_browser_history(browser_type, &browser_paths, search_term)
            {
                info!(
                    "Found {} {} history entries",
                    browser_entries.len(),
                    Self::browser_type_to_string(browser_type)
                );
                entries.extend(browser_entries);
            }
        }

        entries
    }

    /// Get history for a specific browser type
    fn get_browser_history(
        browser_type: BrowserType,
        db_paths: &[PathBuf],
        search_term: &str,
    ) -> Result<Vec<HistoryEntry>> {
        match browser_type {
            BrowserType::Firefox => Self::get_firefox_history(db_paths, search_term),
            _ => Self::get_chromium_based_history(browser_type, db_paths, search_term),
        }
    }

    /// Get Firefox history from all possible profile directories
    fn get_firefox_history(
        firefox_dirs: &[PathBuf],
        search_term: &str,
    ) -> Result<Vec<HistoryEntry>> {
        let mut entries = Vec::new();

        info!("Checking Firefox profile directories: {:?}", firefox_dirs);

        for firefox_dir in firefox_dirs {
            if !firefox_dir.exists() {
                debug!("Firefox directory not found: {:?}", firefox_dir);
                continue;
            }

            debug!("Found Firefox directory: {:?}", firefox_dir);

            // Find profile directories that contain places.sqlite
            for dir_entry in fs::read_dir(firefox_dir)? {
                let dir_entry = dir_entry?;
                let path = dir_entry.path();

                if path.is_dir() {
                    let places_db = path.join("places.sqlite");
                    if places_db.exists() {
                        info!("Found Firefox database at: {:?}", places_db);

                        // Try to copy the database to a temporary location since it might be locked
                        let temp_db = Self::create_temp_db_path("firefox_places");

                        if let Err(e) = fs::copy(&places_db, &temp_db) {
                            debug!("Failed to copy Firefox places database: {}", e);
                            continue;
                        }

                        info!(
                            "Successfully copied Firefox database to temporary location: {:?}",
                            temp_db
                        );

                        if let Ok(profile_entries) =
                            SqliteHistory::read_firefox_db(&temp_db, search_term)
                        {
                            info!(
                                "Successfully read {} entries from Firefox profile: {:?}",
                                profile_entries.len(),
                                path
                            );
                            entries.extend(profile_entries);
                        } else {
                            debug!("Failed to read entries from Firefox profile: {:?}", path);
                        }

                        // Clean up
                        let _ = fs::remove_file(temp_db);
                    }
                }
            }
        }

        info!("Total Firefox history entries found: {}", entries.len());
        Ok(entries)
    }

    /// Get history from Chromium-based browsers (Chrome, Brave, etc.)
    fn get_chromium_based_history(
        browser_type: BrowserType,
        db_paths: &[PathBuf],
        search_term: &str,
    ) -> Result<Vec<HistoryEntry>> {
        let mut entries = Vec::new();

        for db_path in db_paths {
            if !db_path.exists() {
                continue;
            }

            debug!(
                "Found {} history database: {:?}",
                Self::browser_type_to_string(browser_type),
                db_path
            );

            // Copy the database to a temporary location since it might be locked
            let temp_db = Self::create_temp_db_path("chromium_history");

            if let Err(e) = fs::copy(db_path, &temp_db) {
                debug!("Failed to copy history database: {}", e);
                continue;
            }

            if let Ok(browser_entries) = SqliteHistory::read_chromium_db(&temp_db, search_term) {
                entries.extend(browser_entries);
            }

            // Clean up
            let _ = fs::remove_file(&temp_db);
        }

        Ok(entries)
    }

    /// Create a temporary database path with a unique name
    fn create_temp_db_path(prefix: &str) -> PathBuf {
        std::env::temp_dir().join(format!("{}_{}.sqlite", prefix, std::process::id()))
    }

    /// Get all supported browsers with their possible install paths
    fn get_supported_browsers() -> HashMap<BrowserType, Vec<PathBuf>> {
        let home_dir = match env::var("HOME") {
            Ok(dir) => dir,
            Err(_) => return HashMap::new(),
        };

        let mut browsers = HashMap::new();
        let install_types = [
            InstallType::Standard,
            InstallType::Snap,
            InstallType::Flatpak,
        ];

        // Add paths for all browser types
        for browser_type in [
            BrowserType::Firefox,
            BrowserType::Chrome,
            BrowserType::Chromium,
            BrowserType::Brave,
            BrowserType::Opera,
            BrowserType::OperaDeveloper,
            BrowserType::Vivaldi,
        ] {
            // For Opera Developer, we only support standard installation
            let types = if browser_type == BrowserType::OperaDeveloper {
                &[InstallType::Standard][..]
            } else {
                &install_types[..]
            };

            browsers.insert(
                browser_type,
                Self::build_browser_paths(&home_dir, browser_type, types),
            );
        }

        browsers
    }

    /// Build all possible paths for a browser type across different installation types
    fn build_browser_paths(
        home_dir: &str,
        browser_type: BrowserType,
        install_types: &[InstallType],
    ) -> Vec<PathBuf> {
        // Firefox is special because we need to search directories for profiles
        if browser_type == BrowserType::Firefox {
            let firefox_paths: Vec<PathBuf> = install_types
                .iter()
                .map(|&install_type| match install_type {
                    InstallType::Standard => Path::new(home_dir).join(".mozilla/firefox"),
                    InstallType::Snap => {
                        Path::new(home_dir).join("snap/firefox/common/.mozilla/firefox")
                    }
                    InstallType::Flatpak => {
                        Path::new(home_dir).join(".var/app/org.mozilla.firefox/.mozilla/firefox")
                    }
                })
                .collect();

            debug!("Firefox profile directories to check: {:?}", firefox_paths);
            return firefox_paths;
        }

        // For other browsers, we have specific paths to check
        let base_paths = match browser_type {
            BrowserType::Firefox => unreachable!(), // Handled above
            BrowserType::Chrome => vec![
                ".config/google-chrome/Default/History",
                ".config/google-chrome/Profile 1/History",
            ],
            BrowserType::Chromium => vec![
                ".config/chromium/Default/History",
                ".config/chromium/Profile 1/History",
            ],
            BrowserType::Brave => vec![
                ".config/BraveSoftware/Brave-Browser/Default/History",
                ".config/BraveSoftware/Brave-Browser/Profile 1/History",
            ],
            BrowserType::Opera => vec![".config/opera/History"],
            BrowserType::OperaDeveloper => vec![".config/opera-developer/History"],
            BrowserType::Vivaldi => vec![".config/vivaldi/Default/History"],
        };

        // For each installation type, create paths for all base paths
        let mut paths = Vec::with_capacity(install_types.len() * base_paths.len());

        for &install_type in install_types {
            let prefix = Self::get_install_prefix(install_type, browser_type);

            for base_path in &base_paths {
                paths.push(Path::new(home_dir).join(&prefix).join(base_path));
            }
        }

        if browser_type != BrowserType::Firefox {
            debug!(
                "{} browser paths to check: {:?}",
                Self::browser_type_to_string(browser_type),
                paths
            );
        }

        paths
    }

    /// Get the installation prefix based on installation type and browser type
    fn get_install_prefix(install_type: InstallType, browser_type: BrowserType) -> PathBuf {
        match install_type {
            InstallType::Standard => PathBuf::new(),
            InstallType::Snap => {
                let app_name = match browser_type {
                    BrowserType::Firefox => "firefox",
                    BrowserType::Chrome => "google-chrome",
                    BrowserType::Chromium => "chromium",
                    BrowserType::Brave => "brave",
                    BrowserType::Opera => "opera",
                    BrowserType::OperaDeveloper => "opera-developer",
                    BrowserType::Vivaldi => "vivaldi",
                };

                // Firefox has a different path structure in snap
                if browser_type == BrowserType::Firefox {
                    PathBuf::from("snap").join(app_name).join("common")
                } else {
                    PathBuf::from("snap").join(app_name).join("current")
                }
            }
            InstallType::Flatpak => {
                let app_id = match browser_type {
                    BrowserType::Firefox => "org.mozilla.firefox",
                    BrowserType::Chrome => "com.google.Chrome",
                    BrowserType::Chromium => "org.chromium.Chromium",
                    BrowserType::Brave => "com.brave.Browser",
                    BrowserType::Opera => "com.opera.Opera",
                    BrowserType::OperaDeveloper => "com.opera.OperaDeveloper",
                    BrowserType::Vivaldi => "com.vivaldi.Vivaldi",
                };
                PathBuf::from(".var/app").join(app_id)
            }
        }
    }

    /// Convert browser type to string for logging
    fn browser_type_to_string(browser_type: BrowserType) -> &'static str {
        match browser_type {
            BrowserType::Firefox => "Firefox",
            BrowserType::Chrome => "Chrome",
            BrowserType::Chromium => "Chromium",
            BrowserType::Brave => "Brave",
            BrowserType::Opera => "Opera",
            BrowserType::OperaDeveloper => "Opera Developer",
            BrowserType::Vivaldi => "Vivaldi",
        }
    }
}

/// Manages SQLite database access for browser history
struct SqliteHistory;

impl SqliteHistory {
    /// The SQL query for Firefox history
    fn firefox_history_query(search_term: &str) -> String {
        let search_condition = if search_term.is_empty() {
            String::new()
        } else {
            format!(
                "AND (p.title LIKE '%{}%' OR p.url LIKE '%{}%') ",
                search_term, search_term
            )
        };

        format!(
            "SELECT p.title, p.url, p.visit_count, MAX(h.visit_date) as last_visit 
         FROM moz_places p 
         JOIN moz_historyvisits h ON p.id = h.place_id 
         WHERE p.title IS NOT NULL 
         AND p.title != '' 
         AND p.url NOT LIKE 'data:%'
         AND p.url NOT LIKE 'about:%'
         AND p.url NOT LIKE 'chrome:%'
         AND p.url NOT LIKE 'file:%'
         AND p.url NOT LIKE 'view-source:%'
         AND p.url NOT LIKE 'edge:%'
         AND p.url NOT LIKE 'brave:%'
         AND p.url NOT LIKE 'devtools:%'
         AND p.url NOT LIKE 'blob:%'
         AND length(p.url) < 1000
         -- Exclude titles that are likely not useful
         AND p.title NOT LIKE '% - Google Search'
         AND p.title NOT LIKE '% - Brave Search'
         AND p.title NOT LIKE '% - DuckDuckGo'
         AND p.title NOT LIKE 'localhost:%'
         -- Search filtering
         {0}
         GROUP BY p.url 
         ORDER BY last_visit DESC 
         LIMIT 5",
            search_condition
        )
    }

    /// The SQL query for Chromium-based browsers
    fn chromium_history_query(search_term: &str) -> String {
        let search_condition = if search_term.is_empty() {
            String::new()
        } else {
            format!(
                "AND (title LIKE '%{}%' OR url LIKE '%{}%') ",
                search_term, search_term
            )
        };

        format!(
            "SELECT title, url, visit_count, MAX(last_visit_time) as last_visit_time 
         FROM urls 
         WHERE title != '' 
         AND url NOT LIKE 'data:%'
         AND url NOT LIKE 'about:%'
         AND url NOT LIKE 'chrome:%'
         AND url NOT LIKE 'file:%'
         AND url NOT LIKE 'view-source:%'
         AND url NOT LIKE 'edge:%'
         AND url NOT LIKE 'brave:%'
         AND url NOT LIKE 'devtools:%'
         AND url NOT LIKE 'blob:%'
         AND length(url) < 1000
         -- Exclude titles that are likely not useful
         AND title NOT LIKE '% - Google Search'
         AND title NOT LIKE '% - Brave Search'
         AND title NOT LIKE '% - DuckDuckGo'
         AND title NOT LIKE 'localhost:%'
         -- Search filtering
         {0}
         GROUP BY url
         ORDER BY last_visit_time DESC 
         LIMIT 5",
            search_condition
        )
    }

    /// Read history from Firefox places.sqlite database
    fn read_firefox_db(db_path: &Path, search_term: &str) -> Result<Vec<HistoryEntry>> {
        let conn = Self::open_connection(db_path)?;
        let mut entries = Vec::new();

        let query = Self::firefox_history_query(search_term);
        let mut stmt = conn.prepare(&query)?;

        let rows = stmt.query_map([], |row| {
            Ok(HistoryEntry {
                title: row.get(0)?,
                url: row.get(1)?,
                visit_count: row.get(2)?,
                last_visit: row.get(3)?,
            })
        })?;

        for row in rows {
            if let Ok(entry) = row {
                entries.push(entry);
            }
        }

        Ok(entries)
    }

    /// Read history from Chromium-based database
    fn read_chromium_db(db_path: &Path, search_term: &str) -> Result<Vec<HistoryEntry>> {
        let conn = Self::open_connection(db_path)?;
        let mut entries = Vec::new();

        let query = Self::chromium_history_query(search_term);
        let mut stmt = match conn.prepare(&query) {
            Ok(stmt) => stmt,
            Err(e) => {
                debug!("Failed to prepare Chromium history query: {}", e);
                return Err(anyhow!("Failed to prepare query: {}", e));
            }
        };

        let rows = match stmt.query_map([], |row| {
            Ok(HistoryEntry {
                title: row.get(0)?,
                url: row.get(1)?,
                visit_count: row.get(2)?,
                last_visit: row.get(3)?,
            })
        }) {
            Ok(rows) => rows,
            Err(e) => {
                debug!("Failed to query Chromium history: {}", e);
                return Err(anyhow!("Failed to query history: {}", e));
            }
        };

        for row in rows {
            if let Ok(entry) = row {
                entries.push(entry);
            }
        }

        Ok(entries)
    }

    /// Open a SQLite connection with appropriate flags and timeout
    fn open_connection(db_path: &Path) -> Result<Connection> {
        let conn = Connection::open_with_flags(
            db_path,
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_URI,
        )?;

        // Set a busy timeout to handle potential locks
        conn.busy_timeout(std::time::Duration::from_millis(500))?;

        Ok(conn)
    }
}

/// Factory class that creates individual history entry actions
pub struct BrowserHistoryFactory;

impl BrowserHistoryFactory {
    /// Create actions for all matching history entries
    pub fn create_actions_for_query(
        query: &str,
        db: Arc<Database>,
        cx: &mut Context<ActionListView>,
    ) -> Vec<ActionItem> {
        if query.trim().is_empty() {
            return Vec::new();
        }

        info!("Searching browser history for '{}'", query);

        let config = cx.global::<Config>();
        let text_secondary_color = config.text_secondary_color;

        // Use the query parameter to search in the database directly
        let matching_entries = BrowserHistoryHandler::get_history_entries_for_query(query);

        info!(
            "Found {} matching browser history entries",
            matching_entries.len()
        );

        matching_entries
            .into_iter()
            .map(|entry| Self::create_action_from_entry(entry, db.clone(), &config))
            .collect()
    }

    /// Create an action item from a history entry
    fn create_action_from_entry(
        entry: HistoryEntry,
        db: Arc<Database>,
        config: &Config,
    ) -> ActionItem {
        let handler = BrowserHistoryHandler::with_entry(entry.clone());
        let display_title = if entry.title.is_empty() {
            entry.url.clone()
        } else {
            entry.title.clone()
        };
        let display_url = entry.url.clone();
        let name = display_title.clone();
        let text_secondary_color = config.text_secondary_color;

        // Create a static string ID that lives for the entire program
        let id_str = Box::leak(
            format!(
                "browser-history-{}",
                entry.url.chars().take(20).collect::<String>()
            )
            .into_boxed_str(),
        );

        ActionItem::new(
            ActionId::Builtin(id_str),
            handler,
            move || {
                div()
                    .flex()
                    .gap_4()
                    .child(div().flex_none().child(name.clone()))
                    .child(
                        div()
                            .flex_grow()
                            .child(display_url.clone())
                            .text_color(text_secondary_color),
                    )
                    .into_any()
            },
            50 + entry.visit_count.min(100) as usize,
            10,
            db,
        )
    }
}
