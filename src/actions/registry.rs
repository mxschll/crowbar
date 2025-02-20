use crate::action_list_view::ActionListView;
use crate::actions::action_item::{ActionDefinition, ActionItem};
use crate::actions::handlers::{
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
        let total_capacity = self.builtin_actions.len() + 10; // DB returns max 10
        let mut actions = Vec::with_capacity(total_capacity);

        if let Ok(dynamic_actions) = self.db.get_actions_filtered(filter) {
            actions.extend(
                dynamic_actions
                    .into_iter()
                    .map(|action_def| action_def.create_action(self.db.clone(), cx)),
            );
        }

        // Create actions from builtin definitions
        actions.extend(self.builtin_actions.iter().map(|action_def| {
             action_def.create_action(self.db.clone(), cx)
        }));

        actions.retain(|item| item.should_display(filter));
        actions.sort();

        self.filtered_actions = actions;
    }

    pub fn get_actions(&self) -> &Vec<ActionItem> {
        &self.filtered_actions
    }
}
