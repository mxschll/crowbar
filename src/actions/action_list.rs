use crate::actions::action_item::ActionItem;

pub struct ActionList {
    actions: Vec<ActionItem>,
}

impl ActionList {
    pub fn new(actions: Vec<ActionItem>) -> Self {
        ActionList { actions }
    }

    pub fn fuzzy_search(&self, search_term: &str) -> Vec<&ActionItem> {
        if search_term.is_empty() {
            let mut actions: Vec<&ActionItem> = self.actions.iter().collect();
            actions.sort();
            return actions;
        }

        let mut actions: Vec<&ActionItem> = self
            .actions
            .iter()
            .filter(|item| item.should_display(search_term))
            .collect();

        actions.sort();

        actions
    }
}
