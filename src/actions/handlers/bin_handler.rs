use anyhow;
use core::str;
use gpui::{div, rgb, Element, ParentElement, Styled};
use std::path::PathBuf;
use std::sync::Arc;

use crate::actions::action_item::{ActionDefinition, ActionHandler, ActionId, ActionItem};
use crate::database::Database;

#[derive(Clone)]
pub struct BinHandler {
    pub id: usize,
    pub path: PathBuf,
    pub name: String,
    pub relevance: usize,
}

impl ActionHandler for BinHandler {
    fn execute(&self, input: &str) -> anyhow::Result<()> {
        std::process::Command::new(&self.path).arg(input).spawn()?;
        Ok(())
    }

    fn clone_box(&self) -> Box<dyn ActionHandler> {
        Box::new(self.clone())
    }
}

impl ActionDefinition for BinHandler {
    fn create_action(&self, db: Arc<Database>) -> ActionItem {
        let execution_count = db.get_execution_count(self.get_id().as_str()).unwrap_or(0);
        let name = self.get_name();
        let path = self.path.to_string_lossy().to_string();

        ActionItem::new(
            self.get_id(),
            name.clone(),
            vec![],
            "Runs Binary".to_string(),
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
                            .child(path.clone())
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
