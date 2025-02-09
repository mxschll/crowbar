use super::{Action, ActionEntry, ActionType};
use std::process::Command;

#[derive(Debug, Clone)]
pub struct DesktopAction {
    name: String,
    exec: String,
    accepts_args: bool,
}

impl DesktopAction {
    pub fn new(name: String, exec: String, accepts_args: bool) -> Self {
        Self {
            name,
            exec,
            accepts_args,
        }
    }

    pub fn create_entry(
        name: String,
        exec: String,
        accepts_args: bool,
        keywords: Vec<String>,
        description: Option<String>,
    ) -> ActionEntry {
        ActionEntry {
            name: name.clone(),
            description,
            keywords,
            action: Box::new(Self::new(name, exec, accepts_args)),
        }
    }
}

impl Action for DesktopAction {
    fn execute(&self, args: Option<String>) -> bool {
        let exec_cmd = if let Some(args) = args {
            format!("{} {}", self.exec, args)
        } else {
            self.exec.clone()
        };

        if let Ok(mut child) = Command::new("sh")
            .arg("-c")
            .arg(&exec_cmd)
            .spawn() {
            let _ = child.wait();
            true
        } else {
            false
        }
    }

    fn accepts_arguments(&self) -> bool {
        self.accepts_args
    }

    fn display_name(&self) -> &str {
        &self.name
    }

    fn action_type(&self) -> ActionType {
        ActionType::Desktop
    }

    fn execute_clone(&self) -> Box<dyn Action> {
        Box::new(self.clone())
    }
} 