use crate::actions::action_item::{ActionDefinition, ActionItem};
use crate::actions::handlers::{
    duckduckgo_handler::DuckDuckGoHandler, google_handler::GoogleHandler,
    perplexity_handler::PerplexityHandler, url_handler::UrlHandler, yandex_handler::YandexHandler,
};
use crate::database::Database;
use gpui::{div, rgb, Element, ParentElement, Styled};
use log::info;
use std::sync::Arc;

pub struct ActionRegistry {
    db: Arc<Database>,
    builtin_actions: Vec<Box<dyn ActionDefinition>>,
    dynamic_actions: Vec<Box<dyn ActionDefinition>>,
}

impl ActionRegistry {
    pub fn new(db: Arc<Database>) -> Self {
        let mut registry = Self {
            db: db.clone(),
            builtin_actions: Vec::new(),
            dynamic_actions: Vec::new(),
        };

        // Register built-in actions
        registry.register_builtin(Box::new(GoogleHandler));
        registry.register_builtin(Box::new(DuckDuckGoHandler));
        registry.register_builtin(Box::new(YandexHandler));
        registry.register_builtin(Box::new(PerplexityHandler));
        registry.register_builtin(Box::new(UrlHandler));

        // Load dynamic actions from database using the Database instance's method
        // info!("Loading dynamic actions...");
        // match db.get_actions() {
        //     Ok(dynamic_actions) => {
        //         println!(
        //             "Successfully loaded {} dynamic actions",
        //             dynamic_actions.len()
        //         );
        //         for action in dynamic_actions {
        //             registry.register_dynamic(action);
        //         }
        //     }
        //     Err(e) => {
        //         println!("Error loading dynamic actions: {:?}", e);
        //     }
        // }

        registry
    }

    pub fn register_builtin(&mut self, action: Box<dyn ActionDefinition>) {
        self.builtin_actions.push(action);
    }

    pub fn register_dynamic(&mut self, action: Box<dyn ActionDefinition>) {
        self.dynamic_actions.push(action);
    }

    pub fn get_actions_filtered(&self, filter: &str) -> Vec<ActionItem> {
        let mut actions = Vec::new();

        // Create actions from builtin definitions
        for action_def in &self.builtin_actions {
            let id = action_def.get_id();
            // Get execution count for built-in action
            let execution_count = self.db.get_execution_count(id.as_str()).unwrap_or(0);
            let mut action = action_def.create_action(self.db.clone());

            // Update the render function to include execution count
            let name = action.name.clone();
            action.render = Box::new(move || {
                div()
                    .flex()
                    .gap_4()
                    .child(div().flex_none().child(name.clone()))
                    .child(div().flex_grow())
                    .child(
                        div()
                            .child(format!("{}", execution_count))
                            .text_color(rgb(0xA89984)),
                    )
                    .into_any()
            });

            actions.push(action);
        }

        let dynamic_actions = self.db.get_actions_filtered(filter).unwrap_or_default();
        for action_def in dynamic_actions {
            actions.push(action_def.create_action(self.db.clone()));
        }

        actions
    }
}
