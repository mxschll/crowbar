use anyhow;
use gpui::{div, rgb, Element, ParentElement, Styled};
use std::sync::Arc;

use crate::actions::action_item::{ActionDefinition, ActionHandler, ActionId, ActionItem};
use crate::database::Database;

#[derive(Clone)]
pub struct AppHandler {
    pub id: usize,
    pub command: String,
    pub name: String,
    pub relevance: usize,
}

impl ActionHandler for AppHandler {
    fn execute(&self, input: &str) -> anyhow::Result<()> {
        let mut parts = self.command.split_whitespace();
        let program = parts.next().unwrap();
        let args: Vec<_> = parts.collect();

        let mut command = std::process::Command::new(program);
        command.args(args);
        if !input.is_empty() {
            command.arg(input);
        }
        command.spawn()?;
        Ok(())
    }

    fn clone_box(&self) -> Box<dyn ActionHandler> {
        Box::new(self.clone())
    }
}

impl ActionDefinition for AppHandler {
    fn create_action(&self, db: Arc<Database>) -> ActionItem {
        let execution_count = db.get_execution_count(self.get_id().as_str()).unwrap_or(0);
        let name = self.get_name();

        ActionItem::new(
            self.get_id(),
            name.clone(),
            vec![],
            "Runs Application".to_string(),
            self.clone(),
            |_input: &str| false,
            move || {
                div()
                    .flex()
                    .gap_4()
                    .child(div().flex_none().child(name.clone()))
                    .child(
                        div()
                            .flex_grow()
                            .child("Application")
                            .text_color(rgb(0xA89984)),
                    )
                    .child(
                        div()
                            .child(format!("{}", execution_count))
                            .text_color(rgb(0xA89984)),
                    )
                    .into_any()
            },
            self.relevance,
            db,
        )
    }

    fn get_id(&self) -> ActionId {
        ActionId::Dynamic(self.id)
    }

    fn get_name(&self) -> String {
        self.name.clone()
    }
}
