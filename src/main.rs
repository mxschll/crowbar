mod action_list;
mod app_finder;
mod config;
mod database;
mod executable_finder;

use action_list::ActionListView;
use executable_finder::scan_path_executables;

use config::Config;

use std::error::Error;
use std::ops::Range;
use unicode_segmentation::*;

use gpui::{
    actions, div, fill, hsla, point, prelude::*, px, relative, rgb, rgba, size, App, AppContext,
    Bounds, ClipboardItem, CursorStyle, ElementId, ElementInputHandler, EventEmitter, FocusHandle,
    FocusableView, GlobalElementId, KeyBinding, LayoutId, MouseButton, MouseDownEvent,
    MouseMoveEvent, MouseUpEvent, PaintQuad, Pixels, Point, ShapedLine, SharedString, Size, Style,
    TextRun, UTF16Selection, UnderlineStyle, View, ViewContext, ViewInputHandler, WindowBounds,
    WindowContext, WindowOptions,
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
    ]
);

struct TextInput {
    focus_handle: FocusHandle,
    content: SharedString,
    placeholder: SharedString,
    selected_range: Range<usize>,
    selection_reversed: bool,
    marked_range: Option<Range<usize>>,
    last_layout: Option<ShapedLine>,
    last_bounds: Option<Bounds<Pixels>>,
    is_selecting: bool,
}

impl TextInput {
    fn left(&mut self, _: &Left, cx: &mut ViewContext<Self>) {
        if self.selected_range.is_empty() {
            self.move_to(self.previous_boundary(self.cursor_offset()), cx);
        } else {
            self.move_to(self.selected_range.start, cx)
        }
    }

    fn right(&mut self, _: &Right, cx: &mut ViewContext<Self>) {
        if self.selected_range.is_empty() {
            self.move_to(self.next_boundary(self.selected_range.end), cx);
        } else {
            self.move_to(self.selected_range.end, cx)
        }
    }

    fn select_left(&mut self, _: &SelectLeft, cx: &mut ViewContext<Self>) {
        self.select_to(self.previous_boundary(self.cursor_offset()), cx);
    }

    fn select_right(&mut self, _: &SelectRight, cx: &mut ViewContext<Self>) {
        self.select_to(self.next_boundary(self.cursor_offset()), cx);
    }

    fn select_all(&mut self, _: &SelectAll, cx: &mut ViewContext<Self>) {
        self.move_to(0, cx);
        self.select_to(self.content.len(), cx)
    }

    fn home(&mut self, _: &Home, cx: &mut ViewContext<Self>) {
        self.move_to(0, cx);
    }

    fn end(&mut self, _: &End, cx: &mut ViewContext<Self>) {
        self.move_to(self.content.len(), cx);
    }

    fn backspace(&mut self, _: &Backspace, cx: &mut ViewContext<Self>) {
        if self.selected_range.is_empty() {
            self.select_to(self.previous_boundary(self.cursor_offset()), cx)
        }
        self.replace_text_in_range(None, "", cx)
    }

    fn delete(&mut self, _: &Delete, cx: &mut ViewContext<Self>) {
        if self.selected_range.is_empty() {
            self.select_to(self.next_boundary(self.cursor_offset()), cx)
        }
        self.replace_text_in_range(None, "", cx)
    }

    fn on_mouse_down(&mut self, event: &MouseDownEvent, cx: &mut ViewContext<Self>) {
        debug!(
            "Mouse down at position: {:?}, shift: {}",
            event.position, event.modifiers.shift
        );
        self.is_selecting = true;

        if event.modifiers.shift {
            self.select_to(self.index_for_mouse_position(event.position), cx);
        } else {
            self.move_to(self.index_for_mouse_position(event.position), cx)
        }
    }

    fn on_mouse_up(&mut self, _: &MouseUpEvent, _: &mut ViewContext<Self>) {
        self.is_selecting = false;
    }

    fn on_mouse_move(&mut self, event: &MouseMoveEvent, cx: &mut ViewContext<Self>) {
        if self.is_selecting {
            self.select_to(self.index_for_mouse_position(event.position), cx);
        }
    }

    fn paste(&mut self, _: &Paste, cx: &mut ViewContext<Self>) {
        if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
            self.replace_text_in_range(None, &text.replace("\n", " "), cx);
        }
    }

    fn copy(&mut self, _: &Copy, cx: &mut ViewContext<Self>) {
        if !self.selected_range.is_empty() {
            cx.write_to_clipboard(ClipboardItem::new_string(
                (&self.content[self.selected_range.clone()]).to_string(),
            ));
        }
    }

    fn cut(&mut self, _: &Cut, cx: &mut ViewContext<Self>) {
        if !self.selected_range.is_empty() {
            cx.write_to_clipboard(ClipboardItem::new_string(
                (&self.content[self.selected_range.clone()]).to_string(),
            ));
            self.replace_text_in_range(None, "", cx)
        }
    }

    fn move_to(&mut self, offset: usize, cx: &mut ViewContext<Self>) {
        self.selected_range = offset..offset;
        cx.notify()
    }

    fn cursor_offset(&self) -> usize {
        if self.selection_reversed {
            self.selected_range.start
        } else {
            self.selected_range.end
        }
    }

    fn index_for_mouse_position(&self, position: Point<Pixels>) -> usize {
        if self.content.is_empty() {
            return 0;
        }

        let (Some(bounds), Some(line)) = (self.last_bounds.as_ref(), self.last_layout.as_ref())
        else {
            return 0;
        };
        if position.y < bounds.top() {
            return 0;
        }
        if position.y > bounds.bottom() {
            return self.content.len();
        }
        line.closest_index_for_x(position.x - bounds.left())
    }

    fn select_to(&mut self, offset: usize, cx: &mut ViewContext<Self>) {
        if self.selection_reversed {
            self.selected_range.start = offset
        } else {
            self.selected_range.end = offset
        };
        if self.selected_range.end < self.selected_range.start {
            self.selection_reversed = !self.selection_reversed;
            self.selected_range = self.selected_range.end..self.selected_range.start;
        }
        cx.notify()
    }

    fn offset_from_utf16(&self, offset: usize) -> usize {
        let mut utf8_offset = 0;
        let mut utf16_count = 0;

        for ch in self.content.chars() {
            if utf16_count >= offset {
                break;
            }
            utf16_count += ch.len_utf16();
            utf8_offset += ch.len_utf8();
        }

        utf8_offset
    }

    fn offset_to_utf16(&self, offset: usize) -> usize {
        let mut utf16_offset = 0;
        let mut utf8_count = 0;

        for ch in self.content.chars() {
            if utf8_count >= offset {
                break;
            }
            utf8_count += ch.len_utf8();
            utf16_offset += ch.len_utf16();
        }

        utf16_offset
    }

    fn range_to_utf16(&self, range: &Range<usize>) -> Range<usize> {
        self.offset_to_utf16(range.start)..self.offset_to_utf16(range.end)
    }

    fn range_from_utf16(&self, range_utf16: &Range<usize>) -> Range<usize> {
        self.offset_from_utf16(range_utf16.start)..self.offset_from_utf16(range_utf16.end)
    }

    fn previous_boundary(&self, offset: usize) -> usize {
        self.content
            .grapheme_indices(true)
            .rev()
            .find_map(|(idx, _)| (idx < offset).then_some(idx))
            .unwrap_or(0)
    }

    fn next_boundary(&self, offset: usize) -> usize {
        self.content
            .grapheme_indices(true)
            .find_map(|(idx, _)| (idx > offset).then_some(idx))
            .unwrap_or(self.content.len())
    }

    fn reset(&mut self) {
        debug!("Resetting text input state");
        self.content = "".into();
        self.selected_range = 0..0;
        self.selection_reversed = false;
        self.marked_range = None;
        self.last_layout = None;
        self.last_bounds = None;
        self.is_selecting = false;
    }
}

struct TextInputChange {
    content: SharedString,
}

impl EventEmitter<TextInputChange> for TextInput {}

impl ViewInputHandler for TextInput {
    fn text_for_range(
        &mut self,
        range_utf16: Range<usize>,
        actual_range: &mut Option<Range<usize>>,
        _cx: &mut ViewContext<Self>,
    ) -> Option<String> {
        let range = self.range_from_utf16(&range_utf16);
        actual_range.replace(self.range_to_utf16(&range));
        Some(self.content[range].to_string())
    }

    fn selected_text_range(
        &mut self,
        _ignore_disabled_input: bool,
        _cx: &mut ViewContext<Self>,
    ) -> Option<UTF16Selection> {
        Some(UTF16Selection {
            range: self.range_to_utf16(&self.selected_range),
            reversed: self.selection_reversed,
        })
    }

    fn marked_text_range(&self, _cx: &mut ViewContext<Self>) -> Option<Range<usize>> {
        self.marked_range
            .as_ref()
            .map(|range| self.range_to_utf16(range))
    }

    fn unmark_text(&mut self, _cx: &mut ViewContext<Self>) {
        self.marked_range = None;
    }

    fn replace_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        cx: &mut ViewContext<Self>,
    ) {
        let range = range_utf16
            .as_ref()
            .map(|range_utf16| self.range_from_utf16(range_utf16))
            .or(self.marked_range.clone())
            .unwrap_or(self.selected_range.clone());

        debug!(
            "Replacing text in range {:?} with '{}' (length: {})",
            range,
            new_text,
            new_text.len()
        );

        self.content =
            (self.content[0..range.start].to_owned() + new_text + &self.content[range.end..])
                .into();
        self.selected_range = range.start + new_text.len()..range.start + new_text.len();
        self.marked_range.take();

        cx.emit(TextInputChange {
            content: self.content.clone(),
        });

        cx.notify();
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        new_selected_range_utf16: Option<Range<usize>>,
        cx: &mut ViewContext<Self>,
    ) {
        let range = range_utf16
            .as_ref()
            .map(|range_utf16| self.range_from_utf16(range_utf16))
            .or(self.marked_range.clone())
            .unwrap_or(self.selected_range.clone());

        self.content =
            (self.content[0..range.start].to_owned() + new_text + &self.content[range.end..])
                .into();
        self.marked_range = Some(range.start..range.start + new_text.len());
        self.selected_range = new_selected_range_utf16
            .as_ref()
            .map(|range_utf16| self.range_from_utf16(range_utf16))
            .map(|new_range| new_range.start + range.start..new_range.end + range.end)
            .unwrap_or_else(|| range.start + new_text.len()..range.start + new_text.len());

        cx.notify();
    }

    fn bounds_for_range(
        &mut self,
        range_utf16: Range<usize>,
        bounds: Bounds<Pixels>,
        _cx: &mut ViewContext<Self>,
    ) -> Option<Bounds<Pixels>> {
        let last_layout = self.last_layout.as_ref()?;
        let range = self.range_from_utf16(&range_utf16);
        Some(Bounds::from_corners(
            point(
                bounds.left() + last_layout.x_for_index(range.start),
                bounds.top(),
            ),
            point(
                bounds.left() + last_layout.x_for_index(range.end),
                bounds.bottom(),
            ),
        ))
    }
}

struct TextElement {
    input: View<TextInput>,
}

struct PrepaintState {
    line: Option<ShapedLine>,
    cursor: Option<PaintQuad>,
    selection: Option<PaintQuad>,
}

impl IntoElement for TextElement {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for TextElement {
    type RequestLayoutState = ();

    type PrepaintState = PrepaintState;

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        cx: &mut WindowContext,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let mut style = Style::default();
        style.size.width = relative(1.).into();
        style.size.height = cx.line_height().into();
        (cx.request_layout(style, []), ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        cx: &mut WindowContext,
    ) -> Self::PrepaintState {
        let input = self.input.read(cx);
        let content = input.content.clone();
        let selected_range = input.selected_range.clone();
        let cursor = input.cursor_offset();
        let style = cx.text_style();

        let (display_text, text_color) = if content.is_empty() {
            (input.placeholder.clone(), hsla(1., 1., 1., 0.2))
        } else {
            (content.clone(), style.color)
        };

        let run = TextRun {
            len: display_text.len(),
            font: style.font(),
            color: text_color,
            background_color: None,
            underline: None,
            strikethrough: None,
        };
        let runs = if let Some(marked_range) = input.marked_range.as_ref() {
            vec![
                TextRun {
                    len: marked_range.start,
                    ..run.clone()
                },
                TextRun {
                    len: marked_range.end - marked_range.start,
                    underline: Some(UnderlineStyle {
                        color: Some(run.color),
                        thickness: px(1.0),
                        wavy: false,
                    }),
                    ..run.clone()
                },
                TextRun {
                    len: display_text.len() - marked_range.end,
                    ..run.clone()
                },
            ]
            .into_iter()
            .filter(|run| run.len > 0)
            .collect()
        } else {
            vec![run]
        };

        let font_size = style.font_size.to_pixels(cx.rem_size());
        let line = cx
            .text_system()
            .shape_line(display_text, font_size, &runs)
            .unwrap();

        let cursor_pos = line.x_for_index(cursor);
        let (selection, cursor) = if selected_range.is_empty() {
            (
                None,
                Some(fill(
                    Bounds::new(
                        point(bounds.left() + cursor_pos, bounds.top()),
                        size(px(2.), bounds.bottom() - bounds.top()),
                    ),
                    hsla(1., 1., 1., 0.2),
                )),
            )
        } else {
            (
                Some(fill(
                    Bounds::from_corners(
                        point(
                            bounds.left() + line.x_for_index(selected_range.start),
                            bounds.top(),
                        ),
                        point(
                            bounds.left() + line.x_for_index(selected_range.end),
                            bounds.bottom(),
                        ),
                    ),
                    rgba(0x3311ff30),
                )),
                None,
            )
        };
        PrepaintState {
            line: Some(line),
            cursor,
            selection,
        }
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        prepaint: &mut Self::PrepaintState,
        cx: &mut WindowContext,
    ) {
        let focus_handle = self.input.read(cx).focus_handle.clone();
        cx.handle_input(
            &focus_handle,
            ElementInputHandler::new(bounds, self.input.clone()),
        );
        if let Some(selection) = prepaint.selection.take() {
            cx.paint_quad(selection)
        }
        let line = prepaint.line.take().unwrap();
        line.paint(bounds.origin, cx.line_height(), cx).unwrap();

        if focus_handle.is_focused(cx) {
            if let Some(cursor) = prepaint.cursor.take() {
                cx.paint_quad(cursor);
            }
        }

        self.input.update(cx, |input, _cx| {
            input.last_layout = Some(line);
            input.last_bounds = Some(bounds);
        });
    }
}

impl Render for TextInput {
    fn render(&mut self, cx: &mut ViewContext<Self>) -> impl IntoElement {
        div()
            .bg(rgb(0xeeeeee))
            .flex()
            .key_context("TextInput")
            .track_focus(&self.focus_handle(cx))
            .cursor(CursorStyle::IBeam)
            // .on_action(cx.listener(Self::enter))
            .on_action(cx.listener(Self::backspace))
            .on_action(cx.listener(Self::delete))
            .on_action(cx.listener(Self::left))
            .on_action(cx.listener(Self::right))
            .on_action(cx.listener(Self::select_left))
            .on_action(cx.listener(Self::select_right))
            .on_action(cx.listener(Self::select_all))
            .on_action(cx.listener(Self::home))
            .on_action(cx.listener(Self::end))
            .on_action(cx.listener(Self::paste))
            .on_action(cx.listener(Self::cut))
            .on_action(cx.listener(Self::copy))
            .on_mouse_down(MouseButton::Left, cx.listener(Self::on_mouse_down))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::on_mouse_up))
            .on_mouse_up_out(MouseButton::Left, cx.listener(Self::on_mouse_up))
            .on_mouse_move(cx.listener(Self::on_mouse_move))
            .line_height(px(30.))
            .child(
                div()
                    .h(px(30. + 8. * 2.))
                    .w_full()
                    .px_4()
                    .py_2()
                    .bg(rgb(0x141D21))
                    .border_t_1()
                    .border_color(rgb(0x3B4B4F))
                    .text_color(rgb(0xffffff))
                    .child(TextElement {
                        input: cx.view().clone(),
                    }),
            )
    }
}

impl FocusableView for TextInput {
    fn focus_handle(&self, _: &AppContext) -> FocusHandle {
        self.focus_handle.clone()
    }
}

struct Crowbar {
    config: Config,
    text_input: View<TextInput>,
    action_list: View<ActionListView>,
    focus_handle: FocusHandle,
}

impl FocusableView for Crowbar {
    fn focus_handle(&self, _: &AppContext) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Crowbar {
    fn navigate_up(&mut self, _: &Up, cx: &mut ViewContext<Self>) {
        debug!("Navigating up");
        self.action_list.update(cx, |list, cx| {
            list.navigate_up(cx);
        });
    }

    fn navigate_down(&mut self, _: &Down, cx: &mut ViewContext<Self>) {
        debug!("Navigating down");
        self.action_list.update(cx, |list, cx| {
            list.navigate_down(cx);
        });
    }

    fn escape(&mut self, _: &Escape, cx: &mut ViewContext<Self>) {
        info!("Escape pressed, quitting application");
        cx.quit();
    }

    fn handle_enter(&mut self, _: &Enter, cx: &mut ViewContext<Self>) {
        if let Some(action) = self.action_list.read(cx).get_selected_action() {
            action.execute();
            self.text_input.update(cx, |input, _cx| {
                input.reset();
            });

            cx.quit();
        }
    }
}

impl Render for Crowbar {
    fn render(&mut self, cx: &mut ViewContext<Self>) -> impl IntoElement {
        div()
            .id("crowbar")
            .text_size(px(self.config.font_size))
            .track_focus(&self.focus_handle(cx))
            .on_action(cx.listener(Self::handle_enter))
            .on_action(cx.listener(Self::escape))
            .on_action(cx.listener(Self::navigate_up))
            .on_action(cx.listener(Self::navigate_down))
            .font_family(self.config.font_family.clone())
            .bg(rgb(0x141D21))
            .text_color(rgb(0xA4FBFE))
            .flex()
            .flex_col()
            .size_full()
            .child(self.action_list.clone())
            .child(div().mt_auto().child(self.text_input.clone()))
    }
}

/// Scan for path executables and fill db on first run
fn setup() {}

fn main() -> Result<(), Box<dyn Error>> {
    env_logger::builder().init();

    info!("Starting Crowbar application");

    let config = Config::load().unwrap();

    App::new().run(|cx: &mut AppContext| {
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
            KeyBinding::new("tab", Down, None),
            KeyBinding::new("shift-tab", Up, None),
        ]);

        let window = cx
            .open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    ..Default::default()
                },
                |cx| {
                    info!("Scanning for path executables");
                    let executables = scan_path_executables().unwrap_or_default();
                    info!("Found {} path executables", executables.len());

                    let text_input = cx.new_view(|cx| TextInput {
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

                    let action_list = cx.new_view(|cx| ActionListView::new(cx));

                    let crowbar = cx.new_view(|cx| {
                        let crowbar = Crowbar {
                            config,
                            text_input: text_input.clone(),
                            action_list: action_list.clone(),
                            focus_handle: cx.focus_handle(),
                        };

                        crowbar
                    });

                    cx.subscribe(&text_input, move |_view, event, cx| {
                        action_list.clone().update(cx, |this, cx| {
                            this.set_filter(event.content.to_string());
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
                window.update(cx, |_, cx| cx.notify()).ok();
            }
        })
        .detach();

        window
            .update(cx, |view, cx| {
                cx.focus_view(&view.text_input);
                cx.activate(true);
            })
            .unwrap();
    });

    Ok(())
}
