use gpui::MouseButton;
use gpui::{
    div, prelude::FluentBuilder, rgb, uniform_list, InteractiveElement, IntoElement, ParentElement,
    ScrollStrategy, SharedString, Styled, UniformListScrollHandle, ViewContext,
};

use crate::database::{get_actions, initialize_database, insert_action, Action, ActionList};
use crate::executable_finder::scan_path_executables;

const ITEMS_TO_SHOW: usize = 100;

pub struct ActionItemElement {
    pub name: String,
    pub action: Action,
    pub is_selected: bool,
}

impl gpui::Render for ActionItemElement {
    fn render(&mut self, _cx: &mut ViewContext<Self>) -> impl IntoElement {
        div()
            .child(format!("{}", self.name))
            .px_4()
            .py_2()
            .on_mouse_up(MouseButton::Left, {
                let action = self.action.clone();
                move |_event, _cx| {
                    action.execute();
                }
            })
            .when(self.is_selected, |elem| elem.bg(rgb(0x404040)))
    }
}

pub struct ActionListView {
    actions: ActionList,
    pub filter: SharedString,
    pub selected_index: usize,
    pub list_scroll_handle: UniformListScrollHandle,
}

impl ActionListView {
    pub fn new() -> ActionListView {
        let conn = initialize_database().unwrap();

        let mut actions = get_actions(&conn).unwrap();

        if actions.is_empty() {
            let executables = scan_path_executables().unwrap_or_default();

            for file_info in executables {
                let _ = insert_action(
                    &conn,
                    &file_info.name,
                    crate::database::ActionType::Program {
                        name: file_info.name.clone(),
                        path: file_info.path,
                    },
                );
            }

            actions = get_actions(&conn).unwrap();
        }

        Self {
            actions,
            filter: "".into(),
            selected_index: 0,
            list_scroll_handle: UniformListScrollHandle::new(),
        }
    }

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

    fn filtered_items(&self) -> Vec<ActionItemElement> {
        self.actions
            .clone()
            .fuzzy_search(&self.filter)
            .ranked()
            .collect()
            .iter()
            .map(|x| ActionItemElement {
                name: x.action.name.clone(),
                action: x.action.clone(),
                is_selected: false,
            })
            .collect()
    }

    pub fn set_filter(&mut self, new_filter: String) {
        self.filter = new_filter.into();
        self.selected_index = 0;
        self.list_scroll_handle
            .scroll_to_item(self.selected_index, ScrollStrategy::Top);
    }

    pub fn get_selected_action(&self) -> Option<Action> {
        self.filtered_items()
            .get(self.selected_index)
            .map(|item| item.action.clone())
    }
}

impl gpui::Render for ActionListView {
    fn render(&mut self, cx: &mut ViewContext<Self>) -> impl IntoElement {
        div().size_full().child(
            uniform_list(
                cx.view().clone(),
                "action-list",
                self.filtered_items().len(),
                |this, range, _cx| {
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
