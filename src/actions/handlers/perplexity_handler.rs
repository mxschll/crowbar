use anyhow;
use url::Url;
use gpui::{div, rgb, Styled, ParentElement, IntoElement, Element};
use std::sync::Arc;

use crate::actions::action_item::{ActionDefinition, ActionHandler, ActionItem, ActionId};
use crate::actions::action_ids;
use crate::database::Database;

#[derive(Clone)]
pub struct PerplexityHandler;

impl ActionHandler for PerplexityHandler {
    fn execute(&self, input: &str) -> anyhow::Result<()> {
        let encoded_query = urlencoding::encode(input);
        let search_url = format!("https://www.perplexity.ai/?q={}", encoded_query);
        open::that(search_url)?;
        Ok(())
    }

    fn clone_box(&self) -> Box<dyn ActionHandler> {
        Box::new(self.clone())
    }
}

impl ActionDefinition for PerplexityHandler {
    fn create_action(&self, db: Arc<Database>) -> ActionItem {
        let execution_count = db.get_execution_count(self.get_id().as_str()).unwrap_or(0);
        let name = self.get_name();

        ActionItem::new(
            self.get_id(),
            name.clone(),
            vec![],
            "Search with Perplexity AI".to_string(),
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
                            .child("AI Search Engine")
                            .text_color(rgb(0xA89984)),
                    )
                    .child(
                        div()
                            .child(format!("{}", execution_count))
                            .text_color(rgb(0xA89984)),
                    )
                    .into_any()
            },
            0,
            db,
        )
    }

    fn get_id(&self) -> ActionId {
        ActionId::Builtin(action_ids::PERPLEXITY_SEARCH)
    }

    fn get_name(&self) -> String {
        "Perplexity Search".to_string()
    }
} 