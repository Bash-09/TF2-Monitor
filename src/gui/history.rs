use iced::{
    widget::{self, Scrollable},
    Length,
};

use crate::{App, IcedElement};

use super::player;

#[must_use]
pub fn view(state: &App) -> IcedElement<'_> {
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

    Scrollable::new(contents.padding(15))
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}
