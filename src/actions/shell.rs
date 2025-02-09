use super::{Action, ActionEntry, ActionType};
use std::process::Command;

#[derive(Debug, Clone)]
pub struct ShellAction {
    name: String,
    command: String,
    accepts_args: bool,
}

impl ShellAction {
    pub fn new(name: String, command: String, accepts_args: bool) -> Self {
        Self {
            name,
            command,
            accepts_args,
        }
    }

    pub fn create_entry(
        name: String,
        command: String,
        accepts_args: bool,
        keywords: Vec<String>,
        description: Option<String>,
    ) -> ActionEntry {
        ActionEntry {
            name: name.clone(),
            description,
            keywords,
            action: Box::new(Self::new(name, command, accepts_args)),
        }
    }
}

impl Action for ShellAction {
    fn execute(&self, args: Option<String>) -> bool {
        let cmd = if let Some(args) = args {
            format!("{} {}", self.command, args)
        } else {
            self.command.clone()
        };

        if let Ok(mut child) = Command::new("sh")
            .arg("-c")
            .arg(&cmd)
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
        ActionType::Shell
    }

    fn execute_clone(&self) -> Box<dyn Action> {
        Box::new(self.clone())
    }
} 