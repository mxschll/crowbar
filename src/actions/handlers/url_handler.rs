use anyhow;
use gpui::{div, Context, Element, ParentElement, Styled};
use std::sync::Arc;
use url::Url;

use crate::action_list_view::ActionListView;
use crate::actions::action_ids;
use crate::actions::action_item::{ActionDefinition, ActionHandler, ActionId, ActionItem};
use crate::config::Config;
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
    fn create_action(&self, db: Arc<Database>, cx: &mut Context<ActionListView>) -> ActionItem {
        let config = cx.global::<Config>();
        let text_secondary_color = config.text_secondary_color;

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
                            .text_color(text_secondary_color),
                    )
                    .child(
                        div()
                            .child(format!("{}", execution_count))
                            .text_color(text_secondary_color),
                    )
                    .into_any()
            },
            1,
            10,
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
