use crate::action_list_view::ActionListView;
use crate::actions::action_handler::ActionItem;
use crate::actions::handlers::{
    browser_history_handler::BrowserHistoryHandlerFactory,
    duckduckgo_handler::DuckDuckGoHandlerFactory, google_handler::GoogleHandlerFactory,
    perplexity_handler::PerplexityHandlerFactory, url_handler::UrlHandlerFactory,
    yandex_handler::YandexHandlerFactory,
};
use crate::database::Database;
use gpui::Context;
use log::info;
use std::sync::Arc;

use super::action_handler::HandlerFactory;
use super::handlers::executable_handler::AppHandlerFactory;
use super::scanner::ActionScanner;
use crate::database::ActionHandlerModel;
pub struct ActionRegistry {
    db: Arc<Database>,
    filtered_actions: Vec<ActionItem>,
    handler_factories: Vec<Box<dyn HandlerFactory>>,
}

impl ActionRegistry {
    pub fn new(cx: &mut Context<ActionListView>) -> Self {
        let db = Arc::new(Database::new().unwrap());

        let mut registry = Self {
            db: db.clone(),
            filtered_actions: Vec::new(),
            handler_factories: Vec::new(),
        };

        // Register built-in actions
        registry.register_factory(Box::new(AppHandlerFactory));
        registry.register_factory(Box::new(UrlHandlerFactory));
        registry.register_factory(Box::new(BrowserHistoryHandlerFactory));
        registry.register_factory(Box::new(GoogleHandlerFactory));
        registry.register_factory(Box::new(PerplexityHandlerFactory));
        registry.register_factory(Box::new(DuckDuckGoHandlerFactory));
        registry.register_factory(Box::new(YandexHandlerFactory));

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

    pub fn register_factory(&mut self, factory: Box<dyn HandlerFactory>) {
        let id = factory.get_id();

        ActionHandlerModel::insert(self.db.connection(), id).unwrap();
        let active_handlers =
            ActionHandlerModel::get_active_handlers(self.db.connection()).unwrap();
        if active_handlers.contains(&id.to_string()) {
            self.handler_factories.push(factory);
        }
    }

    pub fn set_filter(&mut self, filter: &str, cx: &mut Context<ActionListView>) {
        let mut combined_handlers = Vec::new();

        for factory in &self.handler_factories {
            combined_handlers.extend(factory.create_handlers_for_query(
                filter,
                self.db.clone(),
                cx,
            ));
        }

        combined_handlers.sort();

        let end = combined_handlers.len().min(10);
        self.filtered_actions = combined_handlers[0..end].to_vec();
    }

    pub fn get_actions(&self) -> &Vec<ActionItem> {
        &self.filtered_actions
    }
}
