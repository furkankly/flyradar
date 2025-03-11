use focusable::{Focus, FocusContainer};

use super::focusable_widget::FocusableWidget;

#[derive(Debug, Focus, FocusContainer)]
pub struct Form {
    pub children: Vec<Box<dyn FocusableWidget>>,
}

impl Form {
    pub fn new(children: Vec<Box<dyn FocusableWidget>>) -> Self {
        Self { children }
    }
    // a method to reset because deriving Focus doesn't take care of that
    pub fn reset_focus(&mut self) {
        self.children.iter_mut().for_each(|c| c.blur());
    }
}

impl FromIterator<Box<dyn FocusableWidget>> for Form {
    fn from_iter<T: IntoIterator<Item = Box<dyn FocusableWidget>>>(items: T) -> Self {
        Self::new(items.into_iter().collect())
    }
}
