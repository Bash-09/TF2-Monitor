use iced::{
    widget::{column, row, Button, Container, Rule},
    Length,
};

use crate::{App, IcedContainer, Message};

pub mod chat;
pub mod history;
pub mod killfeed;
pub mod player;
pub mod records;
pub mod server;
pub mod settings;

#[derive(Debug, Clone)]
pub enum View {
    Server,
    History,
    Settings,
    Records,
}

#[must_use]
pub fn main_window(state: &App) -> IcedContainer<'_> {
    // Right panel is either chat + killfeed or the currently selected player
    let right_panel = state
        .selected_player
        .map_or_else(
            || {
                Container::new(column![
                    chat::view(state)
                        .width(Length::Fill)
                        .height(Length::FillPortion(1)),
                    Rule::horizontal(1),
                    killfeed::view(state)
                        .width(Length::Fill)
                        .height(Length::FillPortion(1))
                ])
            },
            |p| player::view(state, p, &state.pfp_cache),
        )
        .width(Length::FillPortion(1))
        .height(Length::Fill);

    // Rest of the view
    let content = row![
        view_select(state),
        Rule::vertical(1),
        match state.view {
            View::Server => server::view(state),
            View::History => history::view(state),
            View::Settings => settings::view(state),
            View::Records => records::view(state),
        }
        .width(Length::FillPortion(3))
        .height(Length::Fill),
        Rule::vertical(1),
        right_panel,
    ];

    Container::new(content)
        .width(Length::Fill)
        .height(Length::Fill)
        .center_x()
        .center_y()
}

#[must_use]
pub fn view_select(_: &App) -> IcedContainer<'_> {
    let content = column![
        Button::new("Server").on_press(Message::SetView(View::Server)),
        Button::new("History").on_press(Message::SetView(View::History)),
        Button::new("Records").on_press(Message::SetView(View::Records)),
        Button::new("Settings").on_press(Message::SetView(View::Settings)),
    ];

    Container::new(content).height(Length::Fill).padding(10)
}
