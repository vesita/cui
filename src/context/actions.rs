use super::{ActionDispatcher, Context};
use crate::action::{ActionRequest, ActionResult};

impl ActionDispatcher for Context {
    fn component_action(&mut self, request: &ActionRequest) -> ActionResult {
        Context::component_action(self, request)
    }
}
