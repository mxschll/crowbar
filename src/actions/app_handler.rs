use core::str;
use std::path::PathBuf;

use crate::actions::action_item::ActionHandler;

#[derive(Clone)]
pub struct AppHandler {
    pub path: PathBuf,
}

impl ActionHandler for AppHandler {
    fn execute(&self, _input: &str) -> Result<(), String> {
        let _ = std::process::Command::new(self.path.clone()).spawn();
        Ok(())
    }

    fn clone_box(&self) -> Box<dyn ActionHandler> {
        Box::new(self.clone())
    }
}
