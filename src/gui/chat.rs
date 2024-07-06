use client_backend::player::Team;
use iced::{
    widget::{self, scrollable::Id, Container, Scrollable},
    Alignment, Length,
};

use crate::{App, IcedContainer, Message};

use super::{
    styles::{colours, ButtonColor},
    FONT_SIZE,
};

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

                let mut name =
                    widget::button(widget::text(&chat.player_name).size(FONT_SIZE)).padding(2);

                if let Some(steamid) = chat.steamid {
                    match state.mac.players.game_info.get(&steamid).map(|gi| gi.team) {
                        Some(Team::Red) => {
                            name = name.style(iced::theme::Button::custom(ButtonColor(
                                colours::team_red_darker(),
                            )));
                        }
                        Some(Team::Blu) => {
                            name = name.style(iced::theme::Button::custom(ButtonColor(
                                colours::team_blu_darker(),
                            )));
                        }
                        _ => {}
                    }

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
