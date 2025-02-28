use std::collections::HashMap;
use std::sync::Arc;

use crate::database::Database;

pub type CommandFn = Arc<dyn Fn(&[&str]) -> String + Send + Sync>;

// Command definition struct to easily register commands
pub struct CommandDefinition {
    pub name: &'static str,
    pub handler: fn(&[&str]) -> String,
}

pub struct CommandRegistry {
    commands: HashMap<String, CommandFn>,
}

impl CommandRegistry {
    pub fn new() -> Self {
        let mut registry = Self {
            commands: HashMap::new(),
        };
        registry.register_default_commands();
        registry
    }

    pub fn execute_command(&self, command_line: &str) -> CommandResult {
        // Skip the colon prefix and trim whitespace
        let command_line = command_line
            .strip_prefix(':')
            .unwrap_or(command_line)
            .trim();

        let args = command_line.split_whitespace().collect::<Vec<&str>>();
        let command = args[0];
        let args = &args[1..];

        let result = self.commands.get(command).unwrap()(args);

        CommandResult {
            success: true,
            message: result,
        }
    }

    pub fn get_command_list(&self) -> Vec<String> {
        self.commands.keys().cloned().collect()
    }

    fn register_default_commands(&mut self) {
        // Define commands using the CommandDefinition struct
        let default_commands = [
            CommandDefinition {
                name: "disable",
                handler: |args| {
                    let db = Arc::new(Database::new().unwrap());
                    let _ = db.set_handler_enabled(args[0], false);
                    "Disable a module".to_string()
                },
            },
            CommandDefinition {
                name: "enable",
                handler: |args| {
                    let db = Arc::new(Database::new().unwrap());
                    let _ = db.set_handler_enabled(args[0], true);
                    "Enable a module".to_string()
                },
            },
        ];

        // Register all commands
        for def in default_commands {
            let handler = def.handler;
            self.commands
                .insert(def.name.to_string(), Arc::new(move |args| handler(args)));
        }
    }
}

pub struct CommandResult {
    pub success: bool,
    pub message: String,
}
