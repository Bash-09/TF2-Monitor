use iced::widget::Container;

use crate::{App, IcedContainer};

#[must_use]
pub fn view(_state: &App) -> IcedContainer<'_> {
    Container::new("Records here").center_x().center_y()
}
