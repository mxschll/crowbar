mod action_list;
mod actions;
mod app_finder;
mod common;
mod config;
mod database;
mod desktop_entry_categories;
mod executable_finder;
mod text_input;

use action_list::ActionListView;
use database::ActionType;

use config::Config;
use text_input::TextInput;

use std::error::Error;

use gpui::{
    actions, div, prelude::*, px, rgb, Action, App, AppContext, Application, Bounds, Context,
    Entity, FocusHandle, Focusable, KeyBinding, Size, Window, WindowBounds, WindowOptions,
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
    config: Config,
    query_input: Entity<TextInput>,
    argument_input: Entity<TextInput>,
    show_argument_input: bool,
    action_list: Entity<ActionListView>,
    focus_handle: FocusHandle,
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

        let action = self.action_list.read(cx).get_selected_action().unwrap();
        self.show_argument_input = match action.action_type {
            ActionType::Desktop { accepts_args, .. } => accepts_args,
            _ => false,
        };
        cx.focus_view(&self.query_input, wd);
    }

    fn navigate_down(&mut self, _: &Down, wd: &mut Window, cx: &mut Context<Self>) {
        self.action_list.update(cx, |list, cx| {
            list.navigate_down(cx);
        });

        let action = self.action_list.read(cx).get_selected_action().unwrap();
        self.show_argument_input = match action.action_type {
            ActionType::Desktop { accepts_args, .. } => accepts_args,
            _ => false,
        };
        cx.focus_view(&self.query_input, wd);
    }

    fn handle_tab(&mut self, _: &Tab, wd: &mut Window, cx: &mut Context<Self>) {
        if self.show_argument_input {
            debug!("Tab pressed, switching focus to argument input");
            cx.focus_view(&self.argument_input, wd);
        }
    }

    fn handle_shift_tab(&mut self, _: &ShiftTab, wd: &mut Window, cx: &mut Context<Self>) {
        debug!("Shift Tab pressed, switching focus");
        cx.focus_view(&self.query_input, wd);
    }

    fn escape(&mut self, _: &Escape, _: &mut Window, cx: &mut Context<Self>) {
        info!("Escape pressed, quitting application");
        cx.quit();
    }

    fn handle_enter(&mut self, _: &Enter, _: &mut Window, cx: &mut Context<Self>) {
        if self.action_list.read(cx).run_selected_action() {
            self.query_input.update(cx, |input, _cx| {
                input.reset();
            });
            self.argument_input.update(cx, |input, _cx| {
                input.reset();
            });

            cx.quit();
        }
    }
}

impl Render for Crowbar {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("crowbar")
            .text_size(px(self.config.font_size))
            .track_focus(&self.focus_handle(cx))
            .on_action(cx.listener(Self::handle_enter))
            .on_action(cx.listener(Self::escape))
            .on_action(cx.listener(Self::navigate_up))
            .on_action(cx.listener(Self::navigate_down))
            .on_action(cx.listener(Self::handle_tab))
            .on_action(cx.listener(Self::handle_shift_tab))
            .font_family(self.config.font_family.clone())
            .bg(rgb(0x141D21))
            .text_color(rgb(0xA4FBFE))
            .flex()
            .flex_col()
            .size_full()
            .child(self.action_list.clone())
            .child(
                div()
                    .w_full()
                    .border_t_1()
                    .border_color(rgb(0x3B4B4F))
                    .child(
                        div()
                            .mt_auto()
                            .flex()
                            .flex_row()
                            .child(div().child(self.query_input.clone()))
                            .child(div().child(if self.show_argument_input {
                                div().child(self.argument_input.clone())
                            } else {
                                div()
                            })),
                    ),
            )
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    env_logger::builder().init();

    let config = Config::load().unwrap();

    Application::new().run(|cx: &mut App| {
        let size = Size {
            width: px(config.window_width),
            height: px(config.window_heigth),
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
                        placeholder: "Type here...".into(),
                        selected_range: 0..0,
                        selection_reversed: false,
                        marked_range: None,
                        last_layout: None,
                        last_bounds: None,
                        is_selecting: false,
                    });

                    let text_input2 = cx.new(|cx| TextInput {
                        focus_handle: cx.focus_handle(),
                        content: "".into(),
                        placeholder: "Query (Press Tab)".into(),
                        selected_range: 0..0,
                        selection_reversed: false,
                        marked_range: None,
                        last_layout: None,
                        last_bounds: None,
                        is_selecting: false,
                    });

                    let action_list = cx.new(|cx| ActionListView::new(cx));
                    let weak_ref = action_list.downgrade();
                    let weak_ref2 = weak_ref.clone();

                    let crowbar = cx.new(|cx| {
                        let crowbar = Crowbar {
                            config,
                            query_input: text_input.clone(),
                            action_list: action_list.clone(),
                            focus_handle: cx.focus_handle(),
                            argument_input: text_input2.clone(),
                            show_argument_input: false,
                        };

                        crowbar
                    });

                    cx.subscribe(&text_input, move |_view, event, cx| {
                        let _ = weak_ref.clone().update(cx, move |this, cx| {
                            this.set_filter(&event.content);
                            cx.notify();
                        });
                    })
                    .detach();

                    cx.subscribe(&text_input2, move |_view, event, cx| {
                        let _ = weak_ref2.update(cx, move |this, cx| {
                            this.set_args(&event.content);
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
