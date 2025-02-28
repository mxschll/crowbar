use crate::action_list_view::ActionListView;
use crate::actions::action_handler::{ActionDefinition, ActionItem};
use crate::actions::handlers::{
    browser_history_handler::BrowserHistoryFactory,
    browser_history_handler::BrowserHistoryHandler,
    duckduckgo_handler::DuckDuckGoHandler, google_handler::GoogleHandler,
    perplexity_handler::PerplexityHandler, url_handler::UrlHandler, yandex_handler::YandexHandler,
};
use crate::database::Database;
use gpui::Context;
use log::info;
use std::sync::Arc;

use super::scanner::ActionScanner;

pub struct ActionRegistry {
    db: Arc<Database>,
    builtin_actions: Vec<Box<dyn ActionDefinition>>,
    filtered_actions: Vec<ActionItem>,
}

impl ActionRegistry {
    pub fn new(cx: &mut Context<ActionListView>) -> Self {
        let db = Arc::new(Database::new().unwrap());

        let mut registry = Self {
            db: db.clone(),
            builtin_actions: Vec::new(),
            filtered_actions: Vec::new(),
        };

        // Register built-in actions
        registry.register_builtin(Box::new(GoogleHandler));
        registry.register_builtin(Box::new(DuckDuckGoHandler));
        registry.register_builtin(Box::new(YandexHandler));
        registry.register_builtin(Box::new(PerplexityHandler));
        registry.register_builtin(Box::new(UrlHandler));
        registry.register_builtin(Box::new(BrowserHistoryHandler::new()));

        registry.set_filter("", cx);

        registry
    }

    pub fn needs_scan(&self) -> bool {
        ActionScanner::needs_scan(self.db.connection())
    }

    pub fn scan(&self, cx: &mut Context<ActionListView>) {
        // Check if we need to scan for dynamic actions
        if ActionScanner::needs_scan(self.db.connection()) {
            info!("No dynamic actions found, starting background scan");
            cx.spawn(|view, mut cx| async move {
                let db = Arc::new(Database::new().unwrap());
                ActionScanner::scan_system(&db);
                let _ = view.update(&mut cx, |_this, cx| {
                    cx.notify();
                });
            })
            .detach();
        }
    }

    pub fn register_builtin(&mut self, action: Box<dyn ActionDefinition>) {
        self.builtin_actions.push(action);
    }

    pub fn set_filter(&mut self, filter: &str, cx: &mut Context<ActionListView>) {
        let total_capacity = self.builtin_actions.len() + 30; // DB returns max 10 + history results
        let mut normal_actions = Vec::with_capacity(total_capacity);
        let mut fallback_actions = Vec::with_capacity(self.builtin_actions.len());

        // Get dynamic actions from DB - these are always normal priority
        if let Ok(dynamic_actions) = self.db.get_actions_filtered(filter) {
            normal_actions.extend(
                dynamic_actions
                    .into_iter()
                    .map(|action_def| action_def.create_action(self.db.clone(), cx)),
            );
        }
        
        // Add browser history actions for the current filter
        if !filter.is_empty() {
            let history_actions = BrowserHistoryFactory::create_actions_for_query(filter, self.db.clone(), cx);
            normal_actions.extend(history_actions);
        }

        // Process built-in actions based on priority
        for action_def in self.builtin_actions.iter() {
            let action_item = action_def.create_action(self.db.clone(), cx);
            
            // Skip actions that wouldn't display anyway
            if !action_item.should_display(filter) {
                continue;
            }
            
            if action_def.is_fallback() {
                fallback_actions.push(action_item);
            } else {
                normal_actions.push(action_item);
            }
        }

        // Sort both groups by their internal relevance
        normal_actions.sort();
        fallback_actions.sort();
        
        // Reserve space for the combined list
        let mut combined_actions = Vec::with_capacity(normal_actions.len() + fallback_actions.len());
        
        // Add all normal actions first
        combined_actions.extend(normal_actions);
        
        // Then add all fallback actions
        // This ensures fallbacks always appear after normal actions regardless of their relevance score
        combined_actions.extend(fallback_actions);
        
        self.filtered_actions = combined_actions;
    }

    pub fn get_actions(&self) -> &Vec<ActionItem> {
        &self.filtered_actions
    }
}
