use std::{clone, sync::Arc};

use gpui::Context;

use crate::{
    action_list_view::ActionListView, actions::action_item::ActionItem, database::Database,
};

use super::registry::{ActionRegistry};

pub struct ActionList {
    db: Arc<Database>,
    registry: ActionRegistry,
}

impl ActionList {
    pub fn new(cx: &mut Context<ActionListView>, db: Arc<Database>) -> Self {
        let registry = ActionRegistry::new(db.clone());

        ActionList { db, registry }
    }

    pub fn fuzzy_search(&self, search_term: &str) -> Vec<ActionItem> {
        let actions = self.registry.get_actions_filtered(search_term);

        if search_term.is_empty() {
            let mut actions: Vec<&ActionItem> = actions.iter().collect();
            actions.sort();
            return actions.into_iter().cloned().collect();
        }

        let mut actions: Vec<&ActionItem> = actions
            .iter()
            .filter(|item| item.should_display(search_term))
            .collect();

        actions.sort();

        actions.into_iter().cloned().collect()
    }
}
