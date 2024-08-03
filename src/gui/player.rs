use chrono::{DateTime, Datelike, Utc};
use iced::{
    alignment::{Horizontal, Vertical},
    widget::{self, column, Button, Image, Scrollable, Space, TextInput},
    Alignment, Length,
};
use tf2_monitor_core::{
    player::{GameInfo, PlayerState, ProfileVisibility, Team},
    player_records::PlayerRecord,
    steamid_ng::SteamID,
};

use super::{
    copy_button, format_time,
    icons::{self, icon},
    open_profile_button,
    styles::colours,
    tooltip, verdict_picker, COLOR_PALETTE, FONT_SIZE, PFP_FULL_SIZE, PFP_SMALL_SIZE,
};
use crate::{App, IcedElement, Message, ALIAS_KEY, NOTES_KEY};

#[allow(clippy::too_many_lines)]
pub fn view(state: &App, player: SteamID) -> IcedElement<'_> {
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
            let mut tooltip_text = String::new();
            record
                .previous_names()
                .iter()
                .for_each(|n| tooltip_text.push_str(&format!("{n}\n")));

            name = name.push(tooltip(name_text, widget::text(tooltip_text)));
        }
        _ => {
            name = name.push(widget::text(name_text));
        }
    }

    // Alias
    if let Some(alias) =
        maybe_record.and_then(|r| r.custom_data().get(ALIAS_KEY).and_then(|v| v.as_str()))
    {
        name = name.push(widget::horizontal_space());
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
        contents = contents.push(widget::Space::with_height(15));
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
    contents = contents.push(widget::Space::with_height(15));
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
                widget::horizontal_space(),
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

    Scrollable::new(contents.padding(15))
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

#[must_use]
#[allow(clippy::module_name_repetitions)]
pub fn row<'a>(state: &'a App, game_info: &'a GameInfo, player: SteamID) -> IcedElement<'a> {
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
    ]
    .spacing(5)
    .align_items(iced::Alignment::Center)
    .padding(0)
    .width(Length::Fill);

    // Party
    for (i, _) in state
        .mac
        .players
        .parties
        .parties()
        .iter()
        .enumerate()
        .filter(|(_, p)| p.contains(&player))
    {
        contents = contents.push(icon(icons::PARTY).style(COLOR_PALETTE[i % COLOR_PALETTE.len()]));
    }

    contents = contents.push(Space::with_width(Length::Fill));

    // Badges
    contents = contents.push(badges(state, player, Some(game_info)));

    // Time
    let time = format_time(game_info.time);

    contents = contents.push(widget::text(time).size(FONT_SIZE));
    contents = contents.push(widget::Space::with_width(5));

    contents
        .width(Length::Fill)
        .align_items(Alignment::Center)
        .into()
}

#[allow(clippy::too_many_lines)]
#[must_use]
pub fn badges<'a>(
    state: &'a App,
    player: SteamID,
    game_info: Option<&'a GameInfo>,
) -> widget::Row<'a, Message, iced::Theme, iced::Renderer> {
    let mut contents = widget::row![].spacing(15);

    if let Some(game_info) = game_info {
        // Spawning
        if game_info.state == PlayerState::Spawning {
            contents = contents.push(tooltip(icon(icons::JOINING), widget::text("Joining")));
        }

        // Disconnected
        if game_info.state == PlayerState::Disconnected {
            contents = contents.push(tooltip(
                icon(icons::DISCONNECT),
                widget::text("Disconnected"),
            ));
        }
    }

    if let Some(steam) = state.mac.players.steam_info.get(&player) {
        // Private / Friends only profile
        if matches!(
            steam.profile_visibility,
            ProfileVisibility::Private | ProfileVisibility::FriendsOnly
        ) {
            let (col, text) = if steam.profile_visibility == ProfileVisibility::FriendsOnly {
                (colours::yellow(), "Friends only profile")
            } else {
                (colours::red(), "Private profile")
            };

            contents = contents.push(tooltip(icon(icons::HIDDEN).style(col), widget::text(text)));
        }

        // VAC and Game bans
        if let Some(days) = steam.days_since_last_ban {
            let mut tooltip_element = widget::Column::new();

            if steam.vac_bans > 0 {
                tooltip_element =
                    tooltip_element.push(widget::text(format!("{} VAC ban(s)", steam.vac_bans)));
            }
            if steam.game_bans > 0 {
                tooltip_element =
                    tooltip_element.push(widget::text(format!("{} game ban(s)", steam.game_bans)));
            }

            tooltip_element =
                tooltip_element.push(widget::text(format!("Last ban {days} days ago.")));

            contents = contents.push(tooltip(
                icon(icons::SHIELD).style(colours::red()).size(FONT_SIZE),
                tooltip_element,
            ));
        }

        // Young account
        if let Some(created) = steam
            .time_created
            .and_then(|t| DateTime::from_timestamp(t, 0))
        {
            let days = Utc::now().signed_duration_since(created).num_days();

            if days < 100 {
                contents = contents.push(tooltip(
                    widget::text("Y")
                        .style(colours::pink())
                        .width(15)
                        .horizontal_alignment(Horizontal::Center),
                    widget::text(format!("Account only created {days} days ago")),
                ));
            }
        }

        // Old steam info
    } else {
        // No steam info
        contents = contents.push(tooltip(
            icon(icons::BLOCK),
            widget::text("No steam info has been fetched"),
        ));
    }

    // Friend
    if state
        .mac
        .players
        .is_friends_with_user(player)
        .is_some_and(|a| a)
    {
        contents = contents.push(icon(icons::FRIEND).style(colours::green()).size(FONT_SIZE));
    }

    // Notes
    if let Some(notes) = state
        .mac
        .players
        .records
        .get(&player)
        .and_then(|r| r.custom_data().get(NOTES_KEY))
        .and_then(|v| v.as_str())
    {
        contents = contents.push(tooltip(icon(icons::NOTES), widget::text(notes)));
    }

    // Vote
    if let Some(vote) = state.mac.server.vote_history().last() {
        if let Some(vote_cast) = vote
            .votes
            .iter()
            .find(|v| v.steamid.is_some_and(|s| s == player))
        {
            let option = vote.options.get(vote_cast.option as usize);

            if option.is_some_and(|o| o == "Yes") {
                contents = contents.push(tooltip(
                    icon(icons::TICK).style(colours::green()),
                    "Voted Yes",
                ));
            }
            if option.is_some_and(|o| o == "No") {
                contents = contents.push(tooltip(
                    icon(icons::CROSS).style(colours::red()),
                    "Voted No",
                ));
            }
        }
    }

    contents
}
