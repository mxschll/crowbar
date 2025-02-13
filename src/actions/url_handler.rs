use anyhow;

use crate::actions::action_item::ActionHandler;

#[derive(Clone)]
pub struct UrlHandler;

impl ActionHandler for UrlHandler {
    fn execute(&self, input: &str) -> anyhow::Result<()> {
        open::that(input)?;
        Ok(())
    }

    fn clone_box(&self) -> Box<dyn ActionHandler> {
        Box::new(self.clone())
    }
}
