use std::fmt::Display;

pub mod desktop;
pub mod shell;

/// Represents an entry for an action with its associated metadata.
#[derive(Debug)]
pub struct ActionEntry {
    pub name: String,
    pub description: Option<String>,
    pub keywords: Vec<String>,
    pub action: Box<dyn Action>,
}

impl Clone for ActionEntry {
    fn clone(&self) -> Self {
        Self {
            name: self.name.clone(),
            description: self.description.clone(),
            keywords: self.keywords.clone(),
            action: self.action.execute_clone(),
        }
    }
}

/// Trait representing an executable action.
pub trait Action: Send + Sync + std::fmt::Debug {
    /// Executes the action with the provided arguments.
    fn execute(&self, args: Option<String>) -> bool;
    
    /// Indicates if the action accepts arguments.
    fn accepts_arguments(&self) -> bool;
    
    /// Returns the display name of the action.
    fn display_name(&self) -> &str;
    
    /// Returns the type of the action.
    fn action_type(&self) -> ActionType;
    
    /// Clones the action and returns a boxed version of it.
    fn execute_clone(&self) -> Box<dyn Action>;
}

/// Different types of actions that can be performed.
#[derive(Debug, Clone, PartialEq)]
pub enum ActionType {
    Desktop,
    Shell,
}

impl Display for ActionType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ActionType::Desktop => write!(f, "Desktop"),
            ActionType::Shell => write!(f, "Shell"),
        }
    }
} 