use client_backend::{player_records::Verdict, steamid_ng::SteamID};
use iced::{
    theme,
    widget::{self, column, row, Button, Container, PickList, Rule, Tooltip},
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

pub const FONT_SIZE: u16 = 13;
pub const PFP_FULL_SIZE: u16 = 184;
pub const PFP_SMALL_SIZE: u16 = 28;

pub const VERDICT_OPTIONS: &[Verdict] = &[
    Verdict::Trusted,
    Verdict::Player,
    Verdict::Suspicious,
    Verdict::Cheater,
    Verdict::Bot,
];

#[must_use]
pub fn open_profile_button<'a>(
    button_text: impl ToString,
    steamid: SteamID,
) -> Tooltip<'a, Message> {
    Tooltip::new(
        Button::new(widget::text(button_text).size(FONT_SIZE)).on_press(Message::Open(
            format!("https://steamcommunity.com/profiles/{}", u64::from(steamid)).into(),
        )),
        "Open Profile",
        iced::widget::tooltip::Position::Bottom,
    )
    .size(FONT_SIZE)
    .style(theme::Container::Box)
}

#[must_use]
pub fn copy_button_with_text<'a>(button_text: impl ToString) -> Tooltip<'a, Message> {
    let copy = button_text.to_string();
    Tooltip::new(
        Button::new(widget::text(button_text).size(FONT_SIZE))
            .on_press(Message::CopyToClipboard(copy)),
        "Copy",
        widget::tooltip::Position::Bottom,
    )
    .size(FONT_SIZE)
    .style(theme::Container::Box)
}

#[must_use]
pub fn copy_button<'a>(to_copy: String) -> Button<'a, Message> {
    Button::new(widget::text("Copy").size(FONT_SIZE)).on_press(Message::CopyToClipboard(to_copy))
}

#[must_use]
pub fn verdict_picker<'a>(verdict: Verdict, steamid: SteamID) -> PickList<'a, Verdict, Message> {
    PickList::new(VERDICT_OPTIONS, Some(verdict), move |v| {
        crate::Message::ChangeVerdict(steamid, v)
    })
    .width(100)
    .text_size(FONT_SIZE)
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
        .width(Length::FillPortion(2))
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
        .width(Length::FillPortion(5))
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
    ]
    .spacing(10);

    Container::new(content).height(Length::Fill).padding(10)
}
