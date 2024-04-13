use std::{collections::HashMap, hash::BuildHasher};

use client_backend::{player::GameInfo, player_records::PlayerRecord, steamid_ng::SteamID};
use iced::{
    theme,
    widget::{self, column, image::Handle, Button, Container, Image, Space, TextInput, Tooltip},
    Length,
};

use super::{
    copy_button, open_profile_button, verdict_picker, FONT_SIZE, PFP_FULL_SIZE, PFP_SMALL_SIZE,
};
use crate::{App, IcedContainer, Message, ALIAS_KEY, NOTES_KEY};

pub fn view<'a, S: BuildHasher>(
    state: &'a App,
    player: SteamID,
    pfp_cache: &'a HashMap<String, Handle, S>,
) -> IcedContainer<'a> {
    let mut contents = column![].spacing(7);

    // pfp and close buttons
    let mut pfp_close = widget::row![];

    if let Some(pfp_handle) = state
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
    let name_text = state.mac.players.get_name(player);

    if let Some(name_text) = name_text {
        name = name.push(widget::text(name_text.to_string()));
    }

    let maybe_record = state.mac.players.records.get(&player);
    if let Some(record) = maybe_record {
        // Alias
        if let Some(alias) = record.custom_data().get(ALIAS_KEY).and_then(|v| v.as_str()) {
            name = name.push(Tooltip::new(
                "☆",
                alias,
                iced::widget::tooltip::Position::Bottom,
            ));
        }

        // Previous names
        if !record.previous_names().is_empty() {
            let mut tooltip = String::new();
            record
                .previous_names()
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
    let steamid_text = format!("{}", u64::from(player));
    let steamid = widget::row![
        open_profile_button(steamid_text.clone(), player),
        copy_button(steamid_text)
    ]
    .align_items(iced::Alignment::Center)
    .spacing(10);

    contents = contents.push(steamid);

    // Verdict
    contents = contents.push(verdict_picker(
        maybe_record.map(PlayerRecord::verdict).unwrap_or_default(),
        player,
    ));

    // Notes
    contents = contents.push(
        TextInput::new(
            "Notes",
            maybe_record
                .and_then(|r| r.custom_data().get(NOTES_KEY).and_then(|v| v.as_str()))
                .unwrap_or(""),
        )
        .size(FONT_SIZE)
        .on_input(move |notes| Message::ChangeNotes(player, notes)),
    );

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
    pfp_cache: &'a HashMap<String, Handle, S>,
) -> IcedContainer<'a> {
    // name
    let mut name = widget::row![];

    // pfp here
    if let Some(steam_info) = &state.mac.players.steam_info.get(&player) {
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
            Button::new(widget::text(&game_info.name).size(FONT_SIZE))
                .on_press(Message::SelectPlayer(player)),
        )
        .align_items(iced::Alignment::Center)
        .spacing(5);

    let mut contents = widget::row![
        verdict_picker(
            state
                .mac
                .players
                .records
                .get(&player)
                .map(PlayerRecord::verdict)
                .unwrap_or_default(),
            player
        ),
        name,
        Space::with_width(Length::Fill),
    ]
    .spacing(5)
    .align_items(iced::Alignment::Center)
    .padding(0)
    .width(Length::Fill);

    if let Some(steam) = state.mac.players.steam_info.get(&player) {
        // Game bans
        if let Some(days) = steam.days_since_last_ban {
            if steam.game_bans > 0 {
                contents = contents.push(
                    Tooltip::new(
                        widget::text("G").size(FONT_SIZE),
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
                        widget::text("V").size(FONT_SIZE),
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
    contents = contents.push(widget::text(game_info.time).size(FONT_SIZE));

    Container::new(contents).width(Length::Fill).center_y()
}
