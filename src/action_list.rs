use gpui::{
    div, prelude::FluentBuilder, rgb, uniform_list, InteractiveElement, IntoElement, ParentElement,
    ScrollStrategy, SharedString, Styled, UniformListScrollHandle, ViewContext,
};

use crate::action_item::{Action, ActionItem};

const ITEMS_TO_SHOW: usize = 100;

pub struct ActionList {
    pub items: Vec<ActionItem>,
    pub filter: SharedString,
    pub selected_index: usize,
    pub list_scroll_handle: UniformListScrollHandle,
}

impl ActionList {
    pub fn navigate_up(&mut self, cx: &mut ViewContext<Self>) {
        if !self.filtered_items().is_empty() {
            self.selected_index = self
                .selected_index
                .checked_sub(1)
                .unwrap_or(self.filtered_items().len().min(ITEMS_TO_SHOW) - 1);

            self.list_scroll_handle
                .scroll_to_item(self.selected_index, ScrollStrategy::Top);

            cx.notify();
        }
    }

    pub fn navigate_down(&mut self, cx: &mut ViewContext<Self>) {
        if !self.filtered_items().is_empty() {
            self.selected_index =
                (self.selected_index + 1) % self.filtered_items().len().min(ITEMS_TO_SHOW);
            self.list_scroll_handle
                .scroll_to_item(self.selected_index, ScrollStrategy::Top);
            cx.notify();
        }
    }

    fn is_fuzzy_match(pattern: &str, text: &str) -> bool {
        let pattern = pattern.to_lowercase();
        let text = text.to_lowercase();

        let pattern_chars: Vec<char> = pattern.chars().collect();
        let text_chars: Vec<char> = text.chars().collect();

        let mut pattern_idx = 0;
        let mut text_idx = 0;

        while pattern_idx < pattern_chars.len() && text_idx < text_chars.len() {
            if pattern_chars[pattern_idx] == text_chars[text_idx] {
                pattern_idx += 1;
            }
            text_idx += 1;
        }

        pattern_idx == pattern_chars.len()
    }

    fn filtered_items(&self) -> Vec<&ActionItem> {
        if self.filter.is_empty() {
            return self.items.iter().collect();
        }

        self.items
            .iter()
            .filter(|item| Self::is_fuzzy_match(&self.filter, &item.name))
            .collect()
    }

    pub fn set_filter(&mut self, new_filter: String) {
        self.filter = new_filter.into();
        self.selected_index = 0; // Reset selection when filter changes
    }

    pub fn get_selected_action(&self) -> Option<Action> {
        self.filtered_items()
            .get(self.selected_index)
            .map(|item| item.action.clone())
    }
}

impl gpui::Render for ActionList {
    fn render(&mut self, cx: &mut ViewContext<Self>) -> impl IntoElement {
        div().size_full().child(
            uniform_list(
                cx.view().clone(),
                "action-list",
                self.filtered_items().len().min(ITEMS_TO_SHOW),
                |this, range, cx| {
                    this.filtered_items()
                        .into_iter()
                        .skip(range.start)
                        .take(range.end - range.start)
                        .enumerate()
                        .map(|(index, item)| {
                            let is_selected = index + range.start == this.selected_index;
                            div()
                                .id(index + range.start)
                                .px_4()
                                .py_2()
                                .child(item.name.clone())
                                .when(is_selected, |x| x.bg(rgb(0x404040)))
                        })
                        .collect()
                },
            )
            .track_scroll(self.list_scroll_handle.clone())
            .h_full(),
        )
    }
}
