use anyhow;
use gpui::{div, Context, Element, ParentElement, Styled};
use std::sync::Arc;
use std::usize;

use crate::action_list_view::ActionListView;
use crate::actions::action_item::{ActionDefinition, ActionHandler, ActionId, ActionItem};
use crate::config::Config;
use crate::database::Database;

static RELEVANCE_BOOST: usize = 2;

#[derive(Clone)]
pub struct AppHandler {
    pub id: usize,
    pub command: String,
    pub name: String,
    pub relevance: usize,
}

impl ActionHandler for AppHandler {
    fn execute(&self, _: &str) -> anyhow::Result<()> {
        let mut parts = self.command.split_whitespace();
        let program = parts.next().unwrap();
        let _ = std::process::Command::new(program).spawn()?;
        Ok(())
    }

    fn clone_box(&self) -> Box<dyn ActionHandler> {
        Box::new(self.clone())
    }
}

impl ActionDefinition for AppHandler {
    fn create_action(&self, db: Arc<Database>, cx: &mut Context<ActionListView>) -> ActionItem {
        let config = cx.global::<Config>();
        let text_secondary_color = config.text_secondary_color;
        let execution_count = db.get_execution_count(self.get_id().as_str()).unwrap_or(0);
        let name = self.get_name();

        ActionItem::new(
            self.get_id(),
            name.clone(),
            vec![],
            "Runs Application".to_string(),
            self.clone(),
            |_input: &str| true,
            move || {
                div()
                    .flex()
                    .gap_4()
                    .child(div().flex_none().child(name.clone()))
                    .child(
                        div()
                            .flex_grow()
                            .child("Application")
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
