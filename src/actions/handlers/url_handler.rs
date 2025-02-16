use anyhow;
use gpui::{div, rgb, Styled, ParentElement, IntoElement, Element};
use std::sync::Arc;
use url::Url;

use crate::actions::action_item::{ActionDefinition, ActionHandler, ActionItem, ActionId};
use crate::actions::action_ids;
use crate::database::Database;

#[derive(Clone)]
pub struct UrlHandler;

impl ActionHandler for UrlHandler {
    fn execute(&self, input: &str) -> anyhow::Result<()> {
        open::that(input)?;
        Ok(())
    }

    fn clone_box(&self) -> Box<dyn ActionHandler> {
        Box::new(self.clone())
    }
}

impl ActionDefinition for UrlHandler {
    fn create_action(&self, db: Arc<Database>) -> ActionItem {
        let execution_count = db.get_execution_count(self.get_id().as_str()).unwrap_or(0);
        let name = self.get_name();

        ActionItem::new(
            self.get_id(),
            name.clone(),
            vec![],
            "Opens URL in default browser".to_string(),
            self.clone(),
            |input: &str| Url::parse(input).is_ok(),
            move || {
                div()
                    .flex()
                    .gap_4()
                    .child(div().flex_none().child(name.clone()))
                    .child(
                        div()
                            .flex_grow()
                            .child("URL Handler")
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
        ActionId::Builtin(action_ids::URL_OPEN)
    }

    fn get_name(&self) -> String {
        "Open URL".to_string()
    }
}
