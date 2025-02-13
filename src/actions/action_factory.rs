use std::{path::PathBuf, sync::Arc};
use anyhow::Result;
use gpui::{div, rgb, Element, ParentElement, Styled};

use crate::database::Database;
use super::{
    action_item::ActionItem,
    action_list::ActionList,
    app_handler::AppHandler,
    bin_handler::BinHandler,
};

pub struct ActionFactory {
    db: Arc<Database>,
}

impl ActionFactory {
    pub fn new(db: Arc<Database>) -> Self {
        Self { db }
    }

    pub fn create_program_action(
        &self,
        id: usize,
        name: String,
        path: PathBuf,
        execution_count: i32,
        relevance_score: usize,
    ) -> ActionItem {
        ActionItem::new(
            id,
            name.clone(),
            vec![],
            "Runs Binary".to_string(),
            BinHandler { path: path.clone() },
            |input: &str| input.contains("http"),
            move || {
                div()
                    .flex()
                    .gap_4()
                    .child(div().flex_none().child(name.clone()))
                    .child(
                        div()
                            .flex_grow()
                            .child(path.to_string_lossy().to_string())
                            .text_color(rgb(0x3B4B4F)),
                    )
                    .child(
                        div()
                            .child(format!("{}", execution_count))
                            .text_color(rgb(0x3B4B4F)),
                    )
                    .into_any()
            },
            relevance_score,
            self.db.clone(),
        )
    }

    pub fn create_desktop_action(
        &self,
        id: usize,
        name: String,
        exec: String,
        execution_count: i32,
        relevance_score: usize,
    ) -> ActionItem {
        ActionItem::new(
            id,
            name.clone(),
            vec![],
            "Runs Application".to_string(),
            AppHandler { path: PathBuf::from(exec.clone()) },
            |input: &str| input.contains("http"),
            move || {
                div()
                    .flex()
                    .gap_4()
                    .child(div().flex_none().child(name.clone()))
                    .child(
                        div()
                            .flex_grow()
                            .child(exec.clone())
                            .text_color(rgb(0x3B4B4F)),
                    )
                    .child(
                        div()
                            .child(format!("{}", execution_count))
                            .text_color(rgb(0x3B4B4F)),
                    )
                    .into_any()
            },
            relevance_score,
            self.db.clone(),
        )
    }
} 