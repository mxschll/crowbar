use crate::actions::action_item::ActionItem;

pub struct ActionList {
    actions: Vec<ActionItem>,
}

impl ActionList {
    pub fn new(actions: Vec<ActionItem>) -> Self {
        ActionList { actions }
    }

    pub fn is_empty(&self) -> bool {
        self.actions.is_empty()
    }

    pub fn fuzzy_search(&self, search_term: &str) -> Vec<&ActionItem> {
        if search_term.is_empty() {
            return self.actions.iter().collect();
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
