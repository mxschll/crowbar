use anyhow;
use gpui::{div, Context, Element, ParentElement, Styled};
use std::sync::Arc;

use crate::action_list_view::ActionListView;
use crate::actions::action_handler::{
    ActionDefinition, ActionHandler, ActionId, ActionItem, HandlerFactory,
};
use crate::actions::action_ids::{self, GOOGLE_SEARCH};
use crate::config::Config;
use crate::database::Database;

pub struct GoogleHandlerFactory;

impl HandlerFactory for GoogleHandlerFactory {
    fn get_id(&self) -> &'static str {
        GOOGLE_SEARCH
    }

    fn create_handlers_for_query(
        &self,
        query: &str,
        db: Arc<Database>,
        cx: &mut Context<ActionListView>,
    ) -> Vec<ActionItem> {
        let mut handlers = Vec::new();
        handlers.push(GoogleHandler.create_action(db.clone(), cx));
        handlers
    }
}

#[derive(Clone)]
pub struct GoogleHandler;
impl ActionHandler for GoogleHandler {
    fn execute(&self, input: &str) -> anyhow::Result<()> {
        let encoded_query = urlencoding::encode(input);
        let search_url = format!("https://www.google.com/search?q={}", encoded_query);

        open::that(search_url)?;
        Ok(())
    }

    fn clone_box(&self) -> Box<dyn ActionHandler> {
        Box::new(self.clone())
    }
}

impl ActionDefinition for GoogleHandler {
    fn create_action(&self, db: Arc<Database>, cx: &mut Context<ActionListView>) -> ActionItem {
        let config = cx.global::<Config>();
        let text_secondary_color = config.text_secondary_color;

        let (relevance, execution_count) = db.get_action_relevance(self.get_id().as_str()).unwrap();
        let name = self.get_name();

        ActionItem::new(
            self.get_id(),
            self.clone(),
            move || {
                div()
                    .flex()
                    .gap_4()
                    .child(div().flex_none().child(name.clone()))
                    .child(
                        div()
                            .flex_grow()
                            .child("Search Engine")
                            .text_color(text_secondary_color),
                    )
                    .child(
                        div()
                            .child(format!("{}", execution_count))
                            .text_color(text_secondary_color),
                    )
                    .into_any()
            },
            relevance,
            1,
            db,
        )
    }

    fn get_id(&self) -> ActionId {
        ActionId::Builtin(action_ids::GOOGLE_SEARCH)
    }

    fn get_name(&self) -> String {
        "Google Search".to_string()
    }
}
