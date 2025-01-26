use std::time::Duration;

use gpui::{
    black, bounce, ease_in_out, percentage, pulsating_between, px, svg, white, Animation,
    AnimationExt, MouseButton, Transformation, VisualContext,
};
use gpui::{
    div, prelude::FluentBuilder, rgb, uniform_list, InteractiveElement, IntoElement, ParentElement,
    ScrollStrategy, SharedString, Styled, UniformListScrollHandle, ViewContext,
};

use crate::app_finder::scan_desktopentries;
use crate::database::{
    get_actions, initialize_database, insert_action, Action, ActionList, ActionRanking, ActionType,
};
use crate::executable_finder::scan_path_executables;

const ITEMS_TO_SHOW: usize = 100;

pub struct ActionListView {
    actions: ActionList,
    pub filter: SharedString,
    pub selected_index: usize,
    pub list_scroll_handle: UniformListScrollHandle,
}

impl ActionListView {
    pub fn new(cx: &mut ViewContext<Self>) -> ActionListView {
        let conn = initialize_database().unwrap();

        let actions = get_actions(&conn).unwrap();

        if actions.is_empty() {
            cx.spawn(|view, mut cx| async move {
                let conn = initialize_database().unwrap();

                let executables = scan_path_executables().unwrap_or_default();
                executables.iter().for_each(|elem| {
                    let _ = insert_action(
                        &conn,
                        &elem.name,
                        crate::database::ActionType::Program {
                            name: elem.name.clone(),
                            path: elem.path.clone(),
                        },
                    );
                });

                let desktopentries = scan_desktopentries();
                desktopentries.iter().for_each(|elem| {
                    let _ = insert_action(
                        &conn,
                        &elem.name,
                        ActionType::Desktop {
                            name: elem.name.clone(),
                            exec: elem.exec.clone(),
                        },
                    );
                });

                let _ = view.update(&mut cx, |this, cx| {
                    let actions = get_actions(&conn).unwrap();
                    this.actions = actions;
                    cx.notify();
                });
            })
            .detach();
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

    fn filtered_items(&self) -> Vec<ActionRanking> {
        self.actions
            .clone()
            .fuzzy_search(&self.filter)
            .ranked()
            .collect()
            .into_iter()
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
                .child("Scanning system executables..."), //.child(
                                                          //    div().child(".").with_animation(
                                                          //        "dot-1",
                                                          //        Animation::new(Duration::from_secs(2))
                                                          //            .repeat()
                                                          //            .with_easing(pulsating_between(0., 1.)),
                                                          //        move |this, delta| this.text_color(white().opacity(delta)),
                                                          //    ),
                                                          //)
                                                          //.child(
                                                          //    div().child(".").with_animation(
                                                          //        "dot-2",
                                                          //        Animation::new(Duration::from_secs(2))
                                                          //            .repeat()
                                                          //            .with_easing(move |t| pulsating_between(0., 1.)((t + 0.7) % 1.0)),
                                                          //        move |this, delta| this.text_color(white().opacity(delta)),
                                                          //    ),
                                                          //)
                                                          //.child(
                                                          //    div().child(".").with_animation(
                                                          //        "dot-3",
                                                          //        Animation::new(Duration::from_secs(2))
                                                          //            .repeat()
                                                          //            .with_easing(move |t| pulsating_between(0., 1.)((t + 0.6) % 1.0)),
                                                          //        move |this, delta| this.text_color(white().opacity(delta)),
                                                          //    ),
                                                          //),
        )
}

fn render_action_item(
    name: &str,
    details: &str,
    execution_count: i32,
    is_selected: bool,
) -> gpui::Div {
    let secondary_text = |elem: gpui::Div| {
        elem.text_color(rgb(0x3B4B4F))
            .when(is_selected, |elem| elem.text_color(rgb(0x91B0B0)))
    };

    div()
        .flex()
        .gap_4()
        .child(div().flex_none().child(name.to_string()))
        .child(
            secondary_text(div().flex_grow()).child(if details.len() > 50 {
                format!("{}...", &details[..50])
            } else {
                details.to_string()
            }),
        )
        .child(secondary_text(div()).child(format!("{} launches", execution_count)))
}

impl gpui::Render for ActionListView {
    fn render(&mut self, cx: &mut ViewContext<Self>) -> impl IntoElement {
        let items = self.filtered_items();

        if items.is_empty() && self.filter.is_empty() {
            loading_screen()
        } else {
            div().size_full().child(
                uniform_list(
                    cx.view().clone(),
                    "action-list",
                    items.len(),
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
                                    .child(match &item.action.action_type {
                                        ActionType::Program { name, path } => render_action_item(
                                            name,
                                            &path.to_string_lossy(),
                                            item.execution_count,
                                            is_selected,
                                        ),
                                        ActionType::Desktop { name, exec } => render_action_item(
                                            name,
                                            exec,
                                            item.execution_count,
                                            is_selected,
                                        ),
                                    })
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
}
