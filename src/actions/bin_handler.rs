use core::str;
use std::path::PathBuf;
use anyhow;

use crate::actions::action_item::ActionHandler;

#[derive(Clone)]
pub struct BinHandler {
    pub path: PathBuf,
}

impl ActionHandler for BinHandler {
    fn execute(&self, _input: &str) -> anyhow::Result<()> {
        std::process::Command::new(self.path.clone()).spawn()?;
        Ok(())
    }

    fn clone_box(&self) -> Box<dyn ActionHandler> {
        Box::new(self.clone())
    }
}
