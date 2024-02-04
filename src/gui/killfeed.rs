use iced::widget::Container;

use crate::{App, IcedContainer};

#[must_use]
pub fn view(_state: &App) -> IcedContainer<'_> {
    Container::new("Killfeed here").center_x().center_y()
}
