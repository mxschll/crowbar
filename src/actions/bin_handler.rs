use core::str;
use std::path::PathBuf;

use crate::actions::action_item::ActionHandler;

#[derive(Clone)]
pub struct BinHandler {
    pub path: PathBuf,
}

impl ActionHandler for BinHandler {
    fn execute(&self, _input: &str) -> Result<(), String> {
        let mut cmd = std::process::Command::new(self.path.clone());
        cmd.spawn()
            .map(|_| ())
            .map_err(|e| (self.path.to_string_lossy().to_string(), e));

        Ok(())
    }

    fn clone_box(&self) -> Box<dyn ActionHandler> {
        Box::new(self.clone())
    }
}
