use gpui::{
    div, prelude::FluentBuilder, rgb, InteractiveElement, IntoElement, MouseButton, ParentElement,
    Styled, ViewContext,
};

#[derive(Clone)]
pub enum Action {
    OpenProgram { path: String, name: String },
    SwitchView { view_name: String },
}

impl Action {
    fn display_name(&self) -> &str {
        match self {
            Action::OpenProgram { name, .. } => name,
            Action::SwitchView { view_name } => view_name,
        }
    }

    pub fn execute(&self) {
        match self {
            Action::OpenProgram { path, .. } => match std::process::Command::new(path).spawn() {
                Ok(_) => (),
                Err(e) => eprintln!("Failed to start {}: {}", path, e),
            },
            Action::SwitchView { view_name } => {
                println!("Switching to view: {}", view_name);
                // Implement view switching logic here
            }
        }
    }
}

pub struct ActionItem {
    pub name: String,
    pub action: Action,
    pub is_selected: bool,
}

impl ActionItem {}

impl gpui::Render for ActionItem {
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
