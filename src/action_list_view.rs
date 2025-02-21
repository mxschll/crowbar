use gpui::{
    div, prelude::FluentBuilder, rgb, uniform_list, white, Context, InteractiveElement,
    IntoElement, ParentElement, ScrollStrategy, Styled, UniformListScrollHandle, Window,
};

use crate::actions::action_item::ActionItem;
use crate::actions::registry::ActionRegistry;
use crate::config::Config;
use std::sync::Arc;

const ITEMS_TO_SHOW: usize = 30;

pub struct ActionListView {
    actions: ActionRegistry,
    filter: Arc<str>,
    args: Vec<String>,
    selected_index: usize,
    list_scroll_handle: UniformListScrollHandle,
}

impl ActionListView {
    pub fn new(cx: &mut Context<Self>) -> ActionListView {
        let actions = ActionRegistry::new(cx);

        Self {
            actions,
            filter: Default::default(),
            args: Default::default(),
            selected_index: 0,
            list_scroll_handle: UniformListScrollHandle::new(),
        }
    }

    pub fn navigate_up(&mut self, cx: &mut Context<Self>) {
        if !self.actions.get_actions().is_empty() {
            self.selected_index = self
                .selected_index
                .checked_sub(1)
                .unwrap_or(self.actions.get_actions().len().min(ITEMS_TO_SHOW) - 1);

            self.list_scroll_handle
                .scroll_to_item(self.selected_index, ScrollStrategy::Top);

            cx.notify();
        }
    }

    pub fn navigate_down(&mut self, cx: &mut Context<Self>) {
        if !self.actions.get_actions().is_empty() {
            self.selected_index =
                (self.selected_index + 1) % self.actions.get_actions().len().min(ITEMS_TO_SHOW);
            self.list_scroll_handle
                .scroll_to_item(self.selected_index, ScrollStrategy::Top);
            cx.notify();
        }
    }

    pub fn set_filter(&mut self, new_filter: &str, cx: &mut Context<Self>) {
        self.actions.set_filter(new_filter, cx);
        self.selected_index = 0;
        self.list_scroll_handle
            .scroll_to_item(self.selected_index, ScrollStrategy::Top);
    }

    pub fn get_selected_action(&self, cx: &mut Context<Self>) -> Option<ActionItem> {
        self.actions.get_actions().get(self.selected_index).cloned()
    }

    pub fn run_selected_action(&self, cx: &mut Context<Self>) -> bool {
        let filter = &self.filter.to_string();
        if let Some(action) = self.get_selected_action(cx) {
            let _ = action.execute(filter);
            return true;
        }

        return false;
    }
}

fn loading_screen() -> gpui::Div {
    div()
        .size_full()
        .flex_none()
        .items_center()
        .justify_center()
        .text_color(white())
        .text_lg()
        .child(
            div()
                .size_full()
                .flex()
                .items_center()
                .justify_center()
                .child("Scanning system executables..."),
        )
}

impl gpui::Render for ActionListView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let items = self.actions.get_actions();

        if self.filter.is_empty() && self.actions.needs_scan() {
            self.actions.scan(cx);
            loading_screen()
        } else {
            div().size_full().child(
                uniform_list(
                    cx.entity().clone(),
                    "action-list",
                    items.len(),
                    |this, range, _window, cx| {
                        let items = this
                            .actions
                            .get_actions()
                            .into_iter()
                            .skip(range.start)
                            .take(range.end - range.start)
                            .enumerate();

                        let theme = cx.global::<Config>();

                        items
                            .map(|(index, item)| {
                                let is_selected = index + range.start == this.selected_index;
                                div()
                                    .id(index + range.start)
                                    .px_4()
                                    .py_2()
                                    .child(item.clone())
                                    .when(is_selected, |x| x.bg(theme.selected_background_color))
                            })
                            .collect()
                    },
                )
                .track_scroll(self.list_scroll_handle.clone())
                .h_full(),
            )
        }
    }
}
