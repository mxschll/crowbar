use core::str;

use log::debug;

use crate::actions::action_item::ActionHandler;

#[derive(Clone)]
pub struct UrlHandler;

impl ActionHandler for UrlHandler {
    fn execute(&self, input: &str) -> Result<(), String> {
        if let Err(e) = open::that(input) {
            debug!("{}", e);
            Ok(())
        } else {
            Ok(())
        }
    }

    fn clone_box(&self) -> Box<dyn ActionHandler> {
        Box::new(self.clone())
    }
}
