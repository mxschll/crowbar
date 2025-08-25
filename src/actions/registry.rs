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

        registry.lazy_register_factories();
        registry.set_filter("", cx);

        registry
    }

    fn lazy_register_factories(&mut self) {
        let factories: Vec<Box<dyn HandlerFactory>> = vec![
            Box::new(AppHandlerFactory),
            Box::new(UrlHandlerFactory),
            Box::new(BrowserHistoryHandlerFactory),
            Box::new(GoogleHandlerFactory),
            Box::new(PerplexityHandlerFactory),
            Box::new(DuckDuckGoHandlerFactory),
            Box::new(YandexHandlerFactory),
        ];

        for factory in factories {
            let id = factory.get_id();
            let _ = ActionHandlerModel::insert(self.db.connection(), id);
            
            let active_handlers = ActionHandlerModel::get_active_handlers(self.db.connection())
                .unwrap_or_default();
            if active_handlers.contains(&id.to_string()) {
                self.handler_factories.push(factory);
            }
        }
    }

    pub fn needs_scan(&self) -> bool {
        ActionScanner::needs_scan(self.db.connection())
    }

    pub fn scan(&self, cx: &mut Context<ActionListView>) {
        if ActionScanner::needs_scan(self.db.connection()) {
            info!("Starting background system scan");
            let db = self.db.clone();
            cx.spawn(|view, mut cx| async move {
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
        let _ = ActionHandlerModel::insert(self.db.connection(), id);
        
        let active_handlers = ActionHandlerModel::get_active_handlers(self.db.connection())
            .unwrap_or_default();
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
