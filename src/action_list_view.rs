use gpui::{
    div, prelude::FluentBuilder, uniform_list, white, AnyElement, Context, InteractiveElement,
    IntoElement, ParentElement, ScrollStrategy, Styled, UniformListScrollHandle, Window,
};

use crate::actions::registry::ActionRegistry;
use crate::commands::CommandRegistry;
use crate::config::Config;
use std::sync::Arc;

const ITEMS_TO_SHOW: usize = 30;

pub enum ItemMode {
    Action,
    Command,
}

pub struct ActionListView {
    actions: ActionRegistry,
    commands: CommandRegistry,
    filter: Arc<str>,
    selected_index: usize,
    list_scroll_handle: UniformListScrollHandle,
    mode: ItemMode,
}

impl ActionListView {
    pub fn new(cx: &mut Context<Self>) -> ActionListView {
        let actions = ActionRegistry::new(cx);
        let commands = CommandRegistry::new();

        Self {
            actions,
            commands,
            filter: Default::default(),
            selected_index: 0,
            list_scroll_handle: UniformListScrollHandle::new(),
            mode: ItemMode::Action,
        }
    }

    // Get the number of items in the current mode
    fn items_len(&self) -> usize {
        match self.mode {
            ItemMode::Command => self.commands.get_command_list().len(),
            ItemMode::Action => self.actions.get_actions().len(),
        }
    }

    // Navigate with a delta (-1 for up, 1 for down)
    fn navigate(&mut self, delta: isize, cx: &mut Context<Self>) {
        let items_len = self.items_len();

        if items_len == 0 {
            return;
        }

        self.selected_index = if delta < 0 {
            // Navigate up
            self.selected_index
                .checked_sub(delta.abs() as usize)
                .unwrap_or(items_len.min(ITEMS_TO_SHOW) - 1)
        } else {
            // Navigate down
            (self.selected_index + delta as usize) % items_len.min(ITEMS_TO_SHOW)
        };

        self.list_scroll_handle
            .scroll_to_item(self.selected_index, ScrollStrategy::Top);

        cx.notify();
    }

    pub fn navigate_up(&mut self, cx: &mut Context<Self>) {
        self.navigate(-1, cx);
    }

    pub fn navigate_down(&mut self, cx: &mut Context<Self>) {
        self.navigate(1, cx);
    }

    pub fn set_filter(&mut self, new_filter: &str, cx: &mut Context<Self>) {
        // Determine the mode based on the filter
        let is_command_mode = new_filter.starts_with(':');
        self.mode = if is_command_mode {
            ItemMode::Command
        } else {
            ItemMode::Action
        };

        match self.mode {
            ItemMode::Command => {}
            ItemMode::Action => {
                self.actions.set_filter(new_filter, cx);
            }
        }

        // Reset selection
        self.filter = new_filter.into();
        self.selected_index = 0;
        self.list_scroll_handle
            .scroll_to_item(self.selected_index, ScrollStrategy::Top);
    }

    pub fn run_selected_action(&self, cx: &mut Context<Self>) -> bool {
        let filter = &self.filter.to_string();

        match self.mode {
            ItemMode::Command => {
                let result = self.commands.execute_command(filter);
                result.success
            }
            ItemMode::Action => {
                let action = self.actions.get_actions().get(self.selected_index).unwrap();
                let _ = action.execute(filter);
                true
            }
            _ => false,
        }
    }

    // Render a command list
    fn render_command_list(&self, cx: &mut Context<Self>) -> AnyElement {
        let command_items = self.commands.get_command_list();
        let theme = cx.global::<Config>();

        div()
            .size_full()
            .flex()
            .flex_col()
            .child(
                // Command mode indicator
                div()
                    .px_4()
                    .py_2()
                    .bg(theme.background_color)
                    .text_color(theme.text_secondary_color)
                    .child(div().flex().flex_col().child("Available commands"))
                    .child(
                        div().flex().flex_col().children(
                            command_items
                                .iter()
                                .map(|command| div().px_4().child(command.clone()))
                                .collect::<Vec<_>>(),
                        ),
                    ),
            )
            .into_any_element()
    }

    // Render an action list
    fn render_action_list(&self, cx: &mut Context<Self>) -> AnyElement {
        let items = self.actions.get_actions();

        if self.filter.is_empty() && self.actions.needs_scan() {
            self.actions.scan(cx);
            loading_screen().into_any_element()
        } else {
            div()
                .size_full()
                .child(
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
                                        .when(is_selected, |x| {
                                            x.bg(theme.selected_background_color)
                                        })
                                })
                                .collect()
                        },
                    )
                    .track_scroll(self.list_scroll_handle.clone())
                    .h_full(),
                )
                .into_any_element()
        }
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
        div().size_full().child(match self.mode {
            ItemMode::Command => self.render_command_list(cx),
            ItemMode::Action => self.render_action_list(cx),
        })
    }
}
