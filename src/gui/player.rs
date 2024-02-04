use std::{collections::HashMap, hash::BuildHasher, sync::Arc};

use client_backend::{player::GameInfo, player_records::Verdict, steamid_ng::SteamID};
use iced::{
    theme,
    widget::{
        self, column, image::Handle, text, Button, Container, Image, PickList, Space, Tooltip,
    },
    Length,
};

use crate::{App, IcedContainer, Message};

const FONT_SIZE: u16 = 13;
const PFP_FULL_SIZE: u16 = 184;
const PFP_SMALL_SIZE: u16 = 28;

const VERDICT_OPTIONS: &[Verdict] = &[
    Verdict::Trusted,
    Verdict::Player,
    Verdict::Suspicious,
    Verdict::Cheater,
    Verdict::Bot,
];

pub fn view<'a, S: BuildHasher>(
    state: &'a App,
    player: SteamID,
    pfp_cache: &'a HashMap<Arc<str>, Handle, S>,
) -> IcedContainer<'a> {
    let mut contents = column![].spacing(5);

    // pfp and close button
    let mut pfp_close = widget::row![];

    if let Some(pfp_handle) = state
        .client
        .mac
        .players
        .steam_info
        .get(&player)
        .as_ref()
        .and_then(|s| pfp_cache.get(&s.pfp_hash))
    {
        pfp_close = pfp_close.push(
            Image::new((*pfp_handle).clone())
                .width(PFP_FULL_SIZE)
                .height(PFP_FULL_SIZE),
        );
    } else {
        pfp_close = pfp_close.push(Space::with_height(184));
    }

    pfp_close = pfp_close.push(widget::row![
        Space::with_width(Length::Fill),
        Button::new("Close").on_press(Message::UnselectPlayer)
    ]);

    contents = contents.push(pfp_close).push(Space::with_height(10));

    // Name and stuff
    let mut name = widget::row![]
        .align_items(iced::Alignment::Center)
        .spacing(10);

    #[allow(clippy::option_if_let_else, clippy::manual_map)]
    let name_text = if let Some(game_info) = state.client.mac.players.game_info.get(&player) {
        Some(game_info.name.clone())
    } else if let Some(steam_info) = state.client.mac.players.steam_info.get(&player) {
        Some(steam_info.account_name.clone())
    } else {
        None
    };

    if let Some(name_text) = name_text {
        name = name.push(
            Button::new(text("Copy").size(FONT_SIZE))
                .on_press(Message::CopyToClipboard(name_text.to_string())),
        );
        name = name.push(text(&name_text).size(FONT_SIZE));
    }

    if let Some(record) = state.client.mac.players.records.get(&player) {
        // Alias
        if let Some(alias) = record.custom_data.get("alias").and_then(|v| v.as_str()) {
            name = name.push(Tooltip::new(
                "☆",
                alias,
                iced::widget::tooltip::Position::Bottom,
            ));
        }

        // Previous names
        if !record.previous_names.is_empty() {
            let mut tooltip = String::new();
            record
                .previous_names
                .iter()
                .for_each(|n| tooltip.push_str(&format!("{n}\n")));

            name = name.push(
                Tooltip::new("P", tooltip, iced::widget::tooltip::Position::Bottom)
                    .style(theme::Container::Box),
            );
        }
    }

    contents = contents.push(name);

    // SteamID
    let steamid_text: Arc<str> = format!("{}", u64::from(player)).into();
    let steamid = widget::row![
        Button::new(text("Copy").size(FONT_SIZE))
            .on_press(Message::CopyToClipboard(steamid_text.to_string())),
        Tooltip::new(
            Button::new(text(&steamid_text).size(FONT_SIZE)).on_press(Message::Open(
                format!("https://steamcommunity.com/profiles/{steamid_text}").into()
            )),
            "Open Profile",
            iced::widget::tooltip::Position::Bottom,
        )
        .style(theme::Container::Box)
    ]
    .align_items(iced::Alignment::Center)
    .spacing(10);

    contents = contents.push(steamid);

    Container::new(contents)
        .width(Length::Fill)
        .height(Length::Fill)
        .padding(15)
}

#[must_use]
#[allow(clippy::module_name_repetitions)]
pub fn row<'a, S: BuildHasher>(
    state: &'a App,
    game_info: &'a GameInfo,
    player: SteamID,
    pfp_cache: &'a HashMap<Arc<str>, Handle, S>,
) -> IcedContainer<'a> {
    // name
    let mut name = widget::row![];

    // pfp here
    if let Some(steam_info) = &state.client.mac.players.steam_info.get(&player) {
        if let Some(pfp_handle) = pfp_cache.get(&steam_info.pfp_hash) {
            name = name.push(
                Image::new(pfp_handle.clone())
                    .width(PFP_SMALL_SIZE)
                    .height(PFP_SMALL_SIZE),
            );
        }
    }

    name = name
        .push(
            Button::new(text(&game_info.name).size(FONT_SIZE))
                .on_press(Message::SelectPlayer(player)),
        )
        .align_items(iced::Alignment::Center)
        .spacing(5);

    let mut contents = widget::row![
        PickList::new(
            VERDICT_OPTIONS,
            Some(
                state
                    .client
                    .mac
                    .players
                    .records
                    .get(&player)
                    .map(|r| r.verdict)
                    .unwrap_or_default()
            ),
            move |v| { Message::ChangeVerdict(player, v) }
        )
        .width(100)
        .text_size(FONT_SIZE),
        name,
        Space::with_width(Length::Fill),
    ]
    .spacing(5)
    .align_items(iced::Alignment::Center)
    .padding(0)
    .width(Length::Fill);

    if let Some(steam) = state.client.mac.players.steam_info.get(&player) {
        // Game bans
        if let Some(days) = steam.days_since_last_ban {
            if steam.game_bans > 0 {
                contents = contents.push(
                    Tooltip::new(
                        text("G").size(FONT_SIZE),
                        format!(
                            "{} game ban(s).\nLast ban {} days ago.",
                            steam.game_bans, days
                        ),
                        iced::widget::tooltip::Position::Bottom,
                    )
                    .style(theme::Container::Box),
                );
            }
        }

        // Vac bans
        if let Some(days) = steam.days_since_last_ban {
            if steam.vac_bans > 0 {
                contents = contents.push(
                    Tooltip::new(
                        text("V").size(FONT_SIZE),
                        format!(
                            "{} VAC ban(s).\nLast ban {} days ago.",
                            steam.vac_bans, days
                        ),
                        iced::widget::tooltip::Position::Bottom,
                    )
                    .style(theme::Container::Box),
                );
            }
        }

        // Young account

        // Friend
    }

    // Time
    contents = contents.push(text(game_info.time).size(FONT_SIZE));

    Container::new(contents).width(Length::Fill).center_y()
}
