use crate::action_list_view::ActionListView;
use crate::actions::action_item::{ActionDefinition, ActionItem};
use crate::actions::handlers::{
    duckduckgo_handler::DuckDuckGoHandler, google_handler::GoogleHandler,
    perplexity_handler::PerplexityHandler, url_handler::UrlHandler, yandex_handler::YandexHandler,
};
use crate::database::Database;
use gpui::{div, rgb, Context, Element, ParentElement, Styled};
use log::info;
use std::sync::Arc;

use super::scanner::ActionScanner;

pub struct ActionRegistry {
    db: Arc<Database>,
    builtin_actions: Vec<Box<dyn ActionDefinition>>,
}

impl ActionRegistry {
    pub fn new(cx: &mut Context<ActionListView>) -> Self {
        let db = Arc::new(Database::new().unwrap());

        // Check if we need to scan for dynamic actions
        if ActionScanner::needs_scan(db.connection()) {
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

        let mut registry = Self {
            db: db.clone(),
            builtin_actions: Vec::new(),
        };

        // Register built-in actions
        registry.register_builtin(Box::new(GoogleHandler));
        registry.register_builtin(Box::new(DuckDuckGoHandler));
        registry.register_builtin(Box::new(YandexHandler));
        registry.register_builtin(Box::new(PerplexityHandler));
        registry.register_builtin(Box::new(UrlHandler));

        registry
    }

    pub fn register_builtin(&mut self, action: Box<dyn ActionDefinition>) {
        self.builtin_actions.push(action);
    }

    pub fn get_actions_filtered(&self, filter: &str, cx: &mut Context<ActionListView>) -> Vec<ActionItem> {
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
            let id = action_def.get_id();
            // Get execution count for built-in action
            let execution_count = self.db.get_execution_count(id.as_str()).unwrap_or(0);
            let mut action = action_def.create_action(self.db.clone(), cx);

            action
        }));

        actions.retain(|item| item.should_display(filter));
        actions.sort_unstable();

        actions
    }
}
