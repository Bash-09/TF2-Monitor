use iced::{
    widget::{self, Container, Scrollable},
    Length,
};

use crate::{App, IcedContainer};

use super::player;

#[must_use]
pub fn view(state: &App) -> IcedContainer<'_> {
    let mut contents = widget::column![].spacing(7);

    for (gi, s) in state
        .mac
        .players
        .history
        .iter()
        .rev()
        .filter_map(|s| state.mac.players.game_info.get(s).map(|gi| (gi, s)))
    {
        contents = contents.push(player::row(state, gi, *s));
    }

    Container::new(Scrollable::new(contents))
        .width(Length::Fill)
        .height(Length::Fill)
        .padding(15)
}
