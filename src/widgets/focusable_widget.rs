use std::any::Any;
use std::fmt::Debug;

use focusable::Focus;
use ratatui::widgets::WidgetRef;

pub trait FocusableWidget: Debug + WidgetRef + Focus + Send + Any {
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
    fn boxed(self) -> Box<dyn FocusableWidget>
    where
        Self: 'static + Sized,
    {
        Box::new(self)
    }
}
