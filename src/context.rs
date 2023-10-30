use std::cell::{Ref, RefCell, RefMut};
use std::rc::Rc;

use crate::state::{Settings, State};

/// Context to access the external traits
pub trait Context {
    /// Returns state reference
    fn get_state(&self) -> Ref<'_, State>;

    /// Returns mutable state reference
    fn mut_state(&self) -> RefMut<'_, State>;

    /// Resets context state to the default one
    fn reset(&mut self) {
        self.mut_state().reset(Settings::default());
    }
}

#[derive(Default)]
pub struct ContextImpl {
    state: RefCell<State>,
}

impl Context for ContextImpl {
    fn get_state(&self) -> Ref<'_, State> {
        self.state.borrow()
    }

    fn mut_state(&self) -> RefMut<'_, State> {
        self.state.borrow_mut()
    }
}

pub fn get_base_context(context: &Rc<RefCell<impl Context + 'static>>) -> Rc<RefCell<dyn Context>> {
    let context: Rc<RefCell<dyn Context>> = context.clone();
    context
}
