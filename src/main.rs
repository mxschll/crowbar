mod action_list_view;
mod actions;
mod commands;
mod common;
mod config;
mod database;
mod system;
mod text_input;

use action_list_view::ActionListView;
use config::{Config, StatusItem};
use text_input::TextInput;

use chrono::Local;
use std::collections::HashMap;
use std::error::Error;
use std::time::Duration;

use gpui::{
    actions, div, prelude::*, px, App, AppContext, Application, Bounds, Context, Entity,
    FocusHandle, Focusable, KeyBinding, Size, Timer, Window, WindowBounds, WindowOptions,
};

use log::{debug, info};

actions!(
    text_input,
    [
        Enter,
        Backspace,
        Delete,
        Left,
        Right,
        SelectLeft,
        SelectRight,
        SelectAll,
        Home,
        End,
        Paste,
        Cut,
        Copy,
        Escape,
        Up,
        Down,
        Tab,
        ShiftTab
    ]
);

struct Crowbar {
    query_input: Entity<TextInput>,
    action_list: Entity<ActionListView>,
    focus_handle: FocusHandle,
    current_time: String,
    status_formats: HashMap<String, String>,
}

impl Focusable for Crowbar {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Crowbar {
    fn navigate_up(&mut self, _: &Up, wd: &mut Window, cx: &mut Context<Self>) {
        self.action_list.update(cx, |list, cx| {
            list.navigate_up(cx);
        });
        cx.focus_view(&self.query_input, wd);
    }

    fn navigate_down(&mut self, _: &Down, wd: &mut Window, cx: &mut Context<Self>) {
        self.action_list.update(cx, |list, cx| {
            list.navigate_down(cx);
        });
        cx.focus_view(&self.query_input, wd);
    }

    fn handle_tab(&mut self, _: &Tab, _: &mut Window, _: &mut Context<Self>) {}

    fn handle_shift_tab(&mut self, _: &ShiftTab, wd: &mut Window, cx: &mut Context<Self>) {
        debug!("Shift Tab pressed, switching focus");
        cx.focus_view(&self.query_input, wd);
    }

    fn escape(&mut self, _: &Escape, _: &mut Window, cx: &mut Context<Self>) {
        info!("Escape pressed, quitting application");
        cx.quit();
    }

    fn handle_enter(&mut self, _: &Enter, _: &mut Window, cx: &mut Context<Self>) {
        if self
            .action_list
            .update(cx, |list, cx| list.run_selected_action(cx))
        {
            self.query_input.update(cx, |input, _cx| {
                input.reset();
            });
            cx.quit();
        }
    }

    fn update_time(&mut self, cx: &mut Context<Self>) {
        self.current_time = Local::now().format("%H:%M:%S").to_string();

        let theme = cx.global::<Config>();
        for item in theme
            .status_bar_left
            .iter()
            .chain(theme.status_bar_center.iter())
            .chain(theme.status_bar_right.iter())
        {
            if let StatusItem::DateTime { format } = item {
                let formatted = Local::now().format(format).to_string();
                self.status_formats.insert(format.clone(), formatted);
            }
        }

        cx.notify();
    }

    fn render_status_items(&self, items: &[StatusItem]) -> Vec<impl IntoElement> {
        items
            .iter()
            .map(|item| match item {
                StatusItem::Text { content } => div().child(content.clone()),
                StatusItem::DateTime { format } => {
                    let formatted = self
                        .status_formats
                        .get(format)
                        .cloned()
                        .unwrap_or_else(|| Local::now().format(format).to_string());
                    div().child(formatted)
                }
            })
            .collect()
    }
}

impl Render for Crowbar {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let config = cx.global::<Config>();

        cx.spawn_in(window, |view, mut cx| async move {
            loop {
                Timer::after(Duration::from_secs(1)).await;

                let _ = cx.update(|_, cx| {
                    view.update(cx, |view, cx| {
                        view.update_time(cx);
                    })
                    .ok()
                });
            }
        })
        .detach();

        div()
            .id("crowbar")
            .text_size(px(config.font_size))
            .track_focus(&self.focus_handle(cx))
            .on_action(cx.listener(Self::handle_enter))
            .on_action(cx.listener(Self::escape))
            .on_action(cx.listener(Self::navigate_up))
            .on_action(cx.listener(Self::navigate_down))
            .on_action(cx.listener(Self::handle_tab))
            .on_action(cx.listener(Self::handle_shift_tab))
            .font_family(config.font_family.clone())
            .bg(config.background_color)
            .border_1()
            .border_color(config.border_color)
            .text_color(config.text_primary_color)
            .flex()
            .flex_col()
            .size_full()
            // Header
            .child(
                div()
                    .w_full()
                    .text_sm()
                    .px_4()
                    .py_1()
                    .border_b_1()
                    .border_color(config.border_color)
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_between()
                    .children(vec![
                        div()
                            .flex()
                            .flex_row()
                            .gap_2()
                            .items_center()
                            .children(self.render_status_items(&config.status_bar_left)),
                        div()
                            .flex()
                            .flex_row()
                            .gap_2()
                            .items_center()
                            .justify_center()
                            .children(self.render_status_items(&config.status_bar_center)),
                        div()
                            .flex()
                            .flex_row()
                            .gap_2()
                            .items_center()
                            .justify_end()
                            .children(self.render_status_items(&config.status_bar_right)),
                    ]),
            )
            .child(self.action_list.clone())
            .child(
                div()
                    .w_full()
                    .border_t_1()
                    .border_color(config.border_color)
                    .child(
                        div()
                            .mt_auto()
                            .flex()
                            .flex_row()
                            .child(div().child(self.query_input.clone())),
                    ),
            )
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    env_logger::builder().init();

    Application::new().run(|cx: &mut App| {
        Config::init(cx);
        let theme = cx.global::<Config>();

        let size = Size {
            width: px(theme.window_width),
            height: px(theme.window_height),
        };

        let bounds = Bounds::centered(None, size, cx);

        cx.bind_keys([
            KeyBinding::new("enter", Enter, None),
            KeyBinding::new("backspace", Backspace, None),
            KeyBinding::new("delete", Delete, None),
            KeyBinding::new("left", Left, None),
            KeyBinding::new("right", Right, None),
            KeyBinding::new("shift-left", SelectLeft, None),
            KeyBinding::new("shift-right", SelectRight, None),
            KeyBinding::new("ctrl-a", SelectAll, None),
            KeyBinding::new("ctrl-v", Paste, None),
            KeyBinding::new("ctrl-c", Copy, None),
            KeyBinding::new("ctrl-x", Cut, None),
            KeyBinding::new("home", Home, None),
            KeyBinding::new("end", End, None),
            KeyBinding::new("escape", Escape, None),
            KeyBinding::new("up", Up, None),
            KeyBinding::new("down", Down, None),
            KeyBinding::new("ctrl-k", Up, None),
            KeyBinding::new("ctrl-j", Down, None),
            KeyBinding::new("ctrl-p", Up, None),
            KeyBinding::new("ctrl-n", Down, None),
            KeyBinding::new("tab", Tab, None),
            KeyBinding::new("shift-tab", ShiftTab, None),
        ]);

        let window = cx
            .open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    ..Default::default()
                },
                |_, cx| {
                    let text_input = cx.new(|cx| TextInput {
                        focus_handle: cx.focus_handle(),
                        content: "".into(),
                        placeholder: "Type to search or enter a command...".into(),
                        selected_range: 0..0,
                        selection_reversed: false,
                        marked_range: None,
                        last_layout: None,
                        last_bounds: None,
                        is_selecting: false,
                    });

                    let action_list = cx.new(|cx| ActionListView::new(cx));
                    let weak_ref = action_list.downgrade();

                    let crowbar = cx.new(|cx| Crowbar {
                        query_input: text_input.clone(),
                        action_list: action_list.clone(),
                        focus_handle: cx.focus_handle(),
                        current_time: Local::now().format("%H:%M:%S").to_string(),
                        status_formats: HashMap::new(),
                    });

                    cx.subscribe(&text_input, move |_view, event, cx| {
                        let _ = weak_ref.clone().update(cx, move |this, cx| {
                            this.set_filter(&event.content, cx);
                            cx.notify();
                        });
                    })
                    .detach();

                    crowbar
                },
            )
            .unwrap();

        cx.on_keyboard_layout_change({
            move |cx| {
                window.update(cx, |_, _, cx| cx.notify()).ok();
            }
        })
        .detach();

        window
            .update(cx, |view, window, cx| {
                cx.focus_view(&view.query_input, window);
                cx.activate(true);
            })
            .unwrap();
    });

    Ok(())
}
