use iced::{
    widget::{self, scrollable::Id, Container, Scrollable},
    Alignment, Length,
};

use crate::{App, IcedContainer, Message};

use super::FONT_SIZE;

pub const SCROLLABLE_ID: &str = "Chat";

#[must_use]
pub fn view(state: &App) -> IcedContainer<'_> {
    // TODO - Virtualise this by using the on_scroll thing

    let contents = state.mac.server.chat_history().iter().fold(
        widget::Column::new()
            .align_items(Alignment::Start)
            .padding(10)
            .spacing(5),
        |contents, chat| {
            contents.push({
                let mut row = widget::Row::new().align_items(Alignment::Center).spacing(5);
                let name =
                    widget::button(widget::text(&chat.player_name).size(FONT_SIZE)).padding(2);

                if let Some(steamid) = chat.steamid {
                    row = row.push(name.on_press(Message::SelectPlayer(steamid)));
                } else {
                    row = row.push(name);
                }

                row = row.push(widget::text(&chat.message).size(FONT_SIZE));
                row = row.push(widget::horizontal_space(Length::Fill));

                row
            })
        },
    );

    Container::new(
        Scrollable::new(contents)
            .id(Id::new(SCROLLABLE_ID))
            .on_scroll(|v| Message::ScrolledChat(v.relative_offset())),
    )
}
