use chrono::{DateTime, Datelike, Utc};
use client_backend::{
    player::{GameInfo, ProfileVisibility, Team},
    player_records::PlayerRecord,
    steamid_ng::SteamID,
};
use iced::{
    alignment::{Horizontal, Vertical},
    theme,
    widget::{self, column, Button, Container, Image, Scrollable, Space, TextInput, Tooltip},
    Alignment, Length,
};

use super::{
    copy_button, open_profile_button, styles::colours, verdict_picker, FONT_SIZE, PFP_FULL_SIZE,
    PFP_SMALL_SIZE,
};
use crate::{App, IcedContainer, Message, ALIAS_KEY, NOTES_KEY};

#[allow(clippy::too_many_lines)]
pub fn view(state: &App, player: SteamID) -> IcedContainer<'_> {
    let mut contents = column![].spacing(7);

    // pfp and close buttons
    let mut pfp_close = widget::row![];

    if let Some((pfp, _)) = state
        .mac
        .players
        .steam_info
        .get(&player)
        .as_ref()
        .and_then(|s| state.pfp_cache.get(&s.pfp_hash))
    {
        pfp_close = pfp_close.push(
            Image::new(pfp.clone())
                .width(PFP_FULL_SIZE)
                .height(PFP_FULL_SIZE),
        );
    } else {
        pfp_close = pfp_close.push(Space::with_height(184));
    }

    pfp_close = pfp_close.push(widget::row![
        Space::with_width(Length::Fill),
        Button::new("Close").on_press(Message::UnselectPlayer),
    ]);

    contents = contents.push(pfp_close).push(Space::with_height(10));

    // Name and stuff
    let mut name = widget::row![]
        .align_items(iced::Alignment::Center)
        .spacing(10);

    let name_text = state.mac.players.get_name(player).unwrap_or("    ");

    let maybe_record = state.mac.players.records.get(&player);

    // Name and previous names
    match maybe_record {
        Some(record) if !record.previous_names().is_empty() => {
            let mut tooltip = String::new();
            record
                .previous_names()
                .iter()
                .for_each(|n| tooltip.push_str(&format!("{n}\n")));

            name = name.push(
                Tooltip::new(name_text, tooltip, iced::widget::tooltip::Position::Bottom)
                    .style(theme::Container::Box),
            );
        }
        _ => {
            name = name.push(widget::text(name_text));
        }
    }

    // Alias
    if let Some(alias) =
        maybe_record.and_then(|r| r.custom_data().get(ALIAS_KEY).and_then(|v| v.as_str()))
    {
        name = name.push(widget::horizontal_space(Length::Fill));
        name = name.push(widget::text(format!("({alias})")));
    }

    contents = contents.push(name);

    // Verdict and SteamID
    let steamid_text = format!("{}", u64::from(player));
    let steamid = widget::row![
        verdict_picker(
            maybe_record.map(PlayerRecord::verdict).unwrap_or_default(),
            player
        ),
        open_profile_button(steamid_text.clone(), player),
        copy_button(steamid_text)
    ]
    .align_items(iced::Alignment::Center)
    .spacing(10);

    contents = contents.push(steamid);

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

    // Game info
    if let Some(gi) = state.mac.players.game_info.get(&player) {
        contents = contents.push(widget::vertical_space(15));
        contents = contents.push(
            widget::text("Game Info")
                .width(Length::Fill)
                .horizontal_alignment(Horizontal::Center),
        );

        // Team
        let mut team = widget::text(format!("{:?}", gi.team)).width(Length::FillPortion(1));

        if matches!(gi.team, Team::Red) {
            team = team.style(colours::team_red());
        } else if matches!(gi.team, Team::Blu) {
            team = team.style(colours::team_blu());
        }

        contents = contents.push(widget::row![
            widget::text("Team").width(Length::FillPortion(1)),
            team
        ]);

        // Kills / Deaths
        contents = contents.push(widget::row![
            widget::text("Kills / Deaths").width(Length::FillPortion(1)),
            widget::text(format!("{} / {}", gi.kills, gi.deaths)).width(Length::FillPortion(1)),
        ]);

        // Ping
        contents = contents.push(widget::row![
            widget::text("Ping").width(Length::FillPortion(1)),
            widget::text(format!("{}ms", gi.ping)).width(Length::FillPortion(1)),
        ]);
    }

    // Account info
    contents = contents.push(widget::vertical_space(15));
    if let Some(si) = state.mac.players.steam_info.get(&player) {
        let age = Utc::now().signed_duration_since(si.fetched);

        contents = contents.push(
            widget::text("Account Info")
                .width(Length::Fill)
                .horizontal_alignment(Horizontal::Center),
        );

        // Profile visibility
        contents = contents.push(widget::row![
            widget::text("Profile Visibility").width(Length::FillPortion(1)),
            widget::text(format!("{:?}", si.profile_visibility))
                .width(Length::FillPortion(1))
                .style(match si.profile_visibility {
                    ProfileVisibility::Private => colours::red(),
                    ProfileVisibility::FriendsOnly => colours::yellow(),
                    ProfileVisibility::Public => colours::green(),
                })
        ]);

        // Date created
        if let Some(created) = si.time_created.and_then(|t| DateTime::from_timestamp(t, 0)) {
            contents = contents.push(widget::row![
                widget::text("Created").width(Length::FillPortion(1)),
                widget::text(format!(
                    "{}/{}/{}",
                    created.day(),
                    created.month(),
                    created.year()
                ))
                .width(Length::FillPortion(1))
            ]);
        }

        // Country
        if let Some(country) = si.country_code.as_ref() {
            contents = contents.push(widget::row![
                widget::text("Country").width(Length::FillPortion(1)),
                widget::text(country).width(Length::FillPortion(1)),
            ]);
        }

        // Bans
        if si.vac_bans > 0 || si.game_bans > 0 {
            let mut bans = widget::column![];

            // VAC Bans
            if si.vac_bans > 0 {
                bans = bans.push(
                    widget::text(format!(
                        "{} VAC {}",
                        si.vac_bans,
                        if si.vac_bans == 1 { "ban" } else { "bans" },
                    ))
                    .style(colours::red()),
                );
            }

            // Game Bans
            if si.game_bans > 0 {
                bans = bans.push(
                    widget::text(format!(
                        "{} Game {}",
                        si.game_bans,
                        if si.game_bans == 1 { "ban" } else { "bans" },
                    ))
                    .style(colours::red()),
                );
            }

            // Days since last ban
            let mut since_last_ban = widget::column![];

            // Days since last ban will be from when the info was last fetched, so we need to
            // add the days since fetched as well.
            if let Some(days_since_last_ban) = si.days_since_last_ban.map(|d| d + age.num_days()) {
                since_last_ban = since_last_ban.push(
                    widget::text(format!("{days_since_last_ban} days since last ban."))
                        .vertical_alignment(Vertical::Center),
                );
            }

            contents = contents.push(
                widget::row![
                    bans.width(Length::FillPortion(1)),
                    since_last_ban.width(Length::FillPortion(1)),
                ]
                .align_items(Alignment::Center),
            );
        }

        // Last refreshed
        contents = contents.push(
            widget::row![
                widget::button(widget::text("Refresh account info").size(FONT_SIZE))
                    .on_press(Message::ProfileLookupRequest(player)),
                widget::horizontal_space(Length::Fill),
                widget::text(format!(
                    "(Last refreshed {})",
                    if age.num_days() > 2 {
                        format!("{} days ago", age.num_days())
                    } else if age.num_hours() > 1 {
                        format!("{} hours ago", age.num_hours())
                    } else if age.num_hours() == 1 {
                        "1 hour ago".to_string()
                    } else if age.num_minutes() > 1 {
                        format!("{} minutes ago", age.num_minutes())
                    } else if age.num_minutes() == 1 {
                        "1 minute ago".to_string()
                    } else {
                        "less than a minute ago".to_string()
                    }
                ))
                .size(FONT_SIZE),
            ]
            .align_items(Alignment::Center),
        );
    } else {
        contents = contents.push(
            widget::button(widget::text("Refresh account info").size(FONT_SIZE))
                .on_press(Message::ProfileLookupRequest(player)),
        );
    }

    Container::new(Scrollable::new(contents.padding(15)))
        .width(Length::Fill)
        .height(Length::Fill)
}

#[must_use]
#[allow(clippy::module_name_repetitions)]
pub fn row<'a>(state: &'a App, game_info: &'a GameInfo, player: SteamID) -> IcedContainer<'a> {
    // pfp + name
    let mut name = widget::row![];

    // pfp here
    if let Some(steam_info) = &state.mac.players.steam_info.get(&player) {
        if let Some((_, pfp_handle)) = state.pfp_cache.get(&steam_info.pfp_hash) {
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
    let hours = game_info.time / (60 * 60);
    let minutes = game_info.time % (60 * 60) / 60;
    let seconds = game_info.time % 60;

    let time = if hours == 0 {
        format!("{minutes:02}:{seconds:02}")
    } else {
        format!("{hours}:{minutes:02}:{seconds:02}")
    };

    contents = contents.push(widget::text(time).size(FONT_SIZE));
    contents = contents.push(widget::horizontal_space(5));

    Container::new(contents).width(Length::Fill).center_y()
}
