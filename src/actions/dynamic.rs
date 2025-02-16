use std::path::PathBuf;

#[derive(Debug, Clone)]
pub enum DynamicActionType {
    /// Binary executable found in the system PATH or with absolute path
    Program {
        name: String,
        path: PathBuf,
    },
    /// Linux desktop entry (.desktop file)
    Desktop {
        name: String,
        exec: String,
        accepts_args: bool,
    },
}

impl DynamicActionType {
    pub fn get_name(&self) -> &str {
        match self {
            Self::Program { name, .. } => name,
            Self::Desktop { name, .. } => name,
        }
    }

    pub fn get_type_str(&self) -> &'static str {
        match self {
            Self::Program { .. } => "program",
            Self::Desktop { .. } => "desktop",
        }
    }
}

pub trait DynamicAction: Send + Sync {
    fn get_type(&self) -> DynamicActionType;
    fn get_relevance_boost(&self) -> f64 {
        match self.get_type() {
            DynamicActionType::Desktop { .. } => 1.1, // Desktop entries get a 10% boost
            _ => 1.0,
        }
    }
} 