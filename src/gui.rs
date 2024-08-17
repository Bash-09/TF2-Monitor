use std::rc::Rc;

use iced::{
    theme,
    widget::{self, column, row, Button, PickList, Rule, Tooltip},
    Color, Length,
};
use tf2_monitor_core::{player_records::Verdict, steamid_ng::SteamID};

use crate::{App, IcedContainer, IcedElement, Message};

use self::styles::picklist::VerdictPickList;

pub mod chat;
pub mod demos;
pub mod history;
pub mod icons;
pub mod killfeed;
pub mod player;
pub mod records;
pub mod replay;
pub mod server;
pub mod settings;
pub mod styles;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum View {
    Server,
    History,
    Settings,
    Records,
    Demos,
    AnalysedDemo(usize),
    Replay,
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

// taken from https://sashamaps.net/docs/resources/20-colors/
const COLOR_PALETTE: [Color; 21] = [
    Color::from_rgb(230.0 / 255.0, 25.0 / 255.0, 75.0 / 255.0),
    Color::from_rgb(60.0 / 255.0, 180.0 / 255.0, 75.0 / 255.0),
    Color::from_rgb(1.0, 225.0 / 255.0, 25.0 / 255.0),
    Color::from_rgb(0.0 / 255.0, 130.0 / 255.0, 200.0 / 255.0),
    Color::from_rgb(245.0 / 255.0, 130.0 / 255.0, 48.0 / 255.0),
    Color::from_rgb(145.0 / 255.0, 30.0 / 255.0, 180.0 / 255.0),
    Color::from_rgb(70.0 / 255.0, 240.0 / 255.0, 240.0 / 255.0),
    Color::from_rgb(240.0 / 255.0, 50.0 / 255.0, 230.0 / 255.0),
    Color::from_rgb(210.0 / 255.0, 245.0 / 255.0, 60.0 / 255.0),
    Color::from_rgb(250.0 / 255.0, 190.0 / 255.0, 212.0 / 255.0),
    Color::from_rgb(0.0 / 255.0, 128.0 / 255.0, 128.0 / 255.0),
    Color::from_rgb(220.0 / 255.0, 190.0 / 255.0, 1.0),
    Color::from_rgb(170.0 / 255.0, 110.0 / 255.0, 40.0 / 255.0),
    Color::from_rgb(1.0, 250.0 / 255.0, 200.0 / 255.0),
    Color::from_rgb(128.0 / 255.0, 0.0 / 255.0, 0.0 / 255.0),
    Color::from_rgb(170.0 / 255.0, 1.0, 195.0 / 255.0),
    Color::from_rgb(128.0 / 255.0, 128.0 / 255.0, 0.0 / 255.0),
    Color::from_rgb(1.0, 215.0 / 255.0, 180.0 / 255.0),
    Color::from_rgb(0.0 / 255.0, 0.0 / 255.0, 128.0 / 255.0),
    Color::from_rgb(128.0 / 255.0, 128.0 / 255.0, 128.0 / 255.0),
    Color::from_rgb(1.0, 1.0, 1.0),
];

#[must_use]
pub fn open_profile_button<'a>(
    button_text: impl ToString,
    steamid: SteamID,
) -> Tooltip<'a, Message> {
    Tooltip::new(
        Button::new(widget::text(button_text).size(FONT_SIZE)).on_press(Message::Open(format!(
            "https://steamcommunity.com/profiles/{}",
            u64::from(steamid)
        ))),
        widget::text("Open Profile").size(FONT_SIZE),
        iced::widget::tooltip::Position::Bottom,
    )
    .style(theme::Container::Box)
}

#[must_use]
pub fn copy_button_with_text<'a>(button_text: impl ToString) -> Tooltip<'a, Message> {
    let copy = button_text.to_string();
    Tooltip::new(
        Button::new(widget::text(button_text).size(FONT_SIZE))
            .on_press(Message::CopyToClipboard(copy)),
        widget::text("Copy").size(FONT_SIZE),
        widget::tooltip::Position::Bottom,
    )
    .style(theme::Container::Box)
}

#[must_use]
pub fn copy_button<'a>(to_copy: String) -> Button<'a, Message> {
    Button::new(widget::text("Copy").size(FONT_SIZE)).on_press(Message::CopyToClipboard(to_copy))
}

#[must_use]
pub fn verdict_picker<'a>(
    verdict: Verdict,
    steamid: SteamID,
) -> PickList<'a, Verdict, &'a [Verdict], Verdict, Message> {
    let style = iced::theme::PickList::Custom(
        Rc::new(VerdictPickList(verdict)),
        Rc::new(VerdictPickList(verdict)),
    );

    PickList::new(VERDICT_OPTIONS, Some(verdict), move |v| {
        crate::Message::ChangeVerdict(steamid, v)
    })
    .width(100)
    .text_size(FONT_SIZE)
    .style(style)
}

#[must_use]
pub fn main_window(state: &App) -> impl Into<IcedElement<'_>> {
    const SPLIT: [u16; 2] = [7, 3];
    // Right panel is either chat + killfeed or the currently selected player
    let right_panel: Option<IcedContainer<'_>> =
        match (state.selected_player, state.settings.show_chat_and_killfeed) {
            (Some(steamid), _) => Some(widget::Container::new(player::detailed_player_view(
                state, steamid,
            ))),
            (None, true) => Some(widget::Container::new(column![
                widget::Container::new(chat::view(state))
                    .width(Length::Fill)
                    .height(Length::FillPortion(1)),
                Rule::horizontal(1),
                widget::Container::new(killfeed::view(state))
                    .width(Length::Fill)
                    .height(Length::FillPortion(1))
            ])),
            (None, false) => None,
        };

    // Rest of the view
    let mut content = widget::row![widget::column![
        view_select(state),
        Rule::horizontal(1),
        match state.view {
            View::Server => server::view(state),
            View::History => history::view(state),
            View::Records => records::view(state),
            View::Demos => demos::demos_list_view(state),
            View::AnalysedDemo(idx) => demos::analysed_demo_view(state, idx),
            View::Replay => state.replay.view(state),
            View::Settings => settings::view(state),
        }
    ]
    .width(Length::FillPortion(SPLIT[0]))
    .height(Length::Fill)];

    if let Some(right_panel) = right_panel {
        content = content.push(Rule::vertical(1)).push(
            right_panel
                .width(Length::FillPortion(SPLIT[1]))
                .height(Length::Fill),
        );
    }

    content
        .width(Length::Fill)
        .height(Length::Fill)
        .align_items(iced::Alignment::Center)
}

#[must_use]
pub fn view_select(state: &App) -> IcedElement<'_> {
    const VIEWS: &[(&str, View)] = &[
        ("Server", View::Server),
        ("History", View::History),
        ("Records", View::Records),
        ("Demos", View::Demos),
        ("Replay", View::Replay),
        ("Settings", View::Settings),
    ];

    let mut views = row![].spacing(10);
    for &(name, v) in VIEWS {
        let mut button = Button::new(name);
        if state.view != v {
            button = button.on_press(Message::SetView(v));
        }
        views = views.push(button);
    }

    let content = row![
        views,
        widget::horizontal_space(),
        Button::new("Chat and Killfeed").on_press(Message::ToggleChatKillfeed),
    ]
    .spacing(10);

    content.width(Length::Fill).padding(10).into()
}

#[must_use]
pub fn tooltip<'a>(
    element: impl Into<iced::Element<'a, Message, iced::Theme, iced::Renderer>>,
    tooltip: impl Into<iced::Element<'a, Message, iced::Theme, iced::Renderer>>,
) -> Tooltip<'a, Message, iced::Theme, iced::Renderer> {
    Tooltip::new(element, tooltip, iced::widget::tooltip::Position::Bottom)
        .style(theme::Container::Box)
}

#[must_use]
pub fn needs_tf2_dir<'a>() -> IcedElement<'a> {
    widget::Container::new(widget::column![
        widget::text("TF2 directory must be set to use this feature."),
        widget::button("Set TF2 Directory").on_press(Message::BrowseTF2Dir),
    ])
    .center_x()
    .center_y()
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}

#[must_use]
pub fn coming_soon<'a>() -> IcedElement<'a> {
    widget::Container::new(widget::text("Coming soon!"))
        .center_x()
        .center_y()
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

/// e.g. 123 secs = "2:03"
#[must_use]
pub fn format_time(seconds: u32) -> String {
    let secs = seconds % 60;
    let mins = (seconds / 60) % 60;
    let hours = seconds / (60 * 60);
    if hours == 0 {
        format!("{mins}:{secs:02}")
    } else {
        format!("{hours}:{mins:02}:{secs:02}")
    }
}

/// "less than a minute ago"
/// "x minutes ago"
/// "x hours ago"
/// "x days ago"
#[must_use]
pub fn format_time_since(seconds: u64) -> String {
    if seconds < 60 {
        "less than a minute ago".to_string()
    } else if seconds == 60 {
        String::from("1 minute ago")
    } else if seconds < 60 * 60 {
        format!("{} minutes ago", seconds / 60)
    } else if seconds < 60 * 60 * 2 {
        String::from("1 hour ago")
    } else if seconds < 60 * 60 * 24 {
        format!("{} hours ago", seconds / (60 * 60))
    } else if seconds < 60 * 60 * 24 * 2 {
        String::from("1 day ago")
    } else {
        format!("{} days ago", seconds / (60 * 60 * 24))
    }
}
