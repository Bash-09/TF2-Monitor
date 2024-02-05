use client_backend::{
    player::{GameInfo, Team},
    steamid_ng::SteamID,
};
use iced::{
    widget::{column, row, text, Container, Scrollable, Space},
    Color, Length,
};

use super::player;
use crate::{App, IcedContainer};

#[must_use]
pub fn view(state: &App) -> IcedContainer<'_> {
    let mut players: Vec<(SteamID, &GameInfo)> = state
        .mac
        .players
        .connected
        .iter()
        .filter_map(|p| state.mac.players.game_info.get(p).map(|gi| (*p, gi)))
        .collect();
    players.sort_by(|&(_, p1), &(_, p2)| p1.time.cmp(&p2.time));

    let team_red_players: Vec<(SteamID, &GameInfo)> = players
        .iter()
        .filter(|&(_, gi)| gi.team == Team::Red)
        .copied()
        .collect();
    let team_red = team_red_players
        .iter()
        .fold(
            column![
                text(format!("Red ({})", team_red_players.len()))
                    .size(20)
                    .style(Color::new(0.72, 0.22, 0.23, 1.0)),
                Space::with_height(10)
            ],
            |col, &(s, gi)| col.push(player::row(state, gi, s, &state.pfp_cache)),
        )
        .width(Length::Fill)
        .padding(10)
        .spacing(3)
        .align_items(iced::Alignment::Center);

    let team_blu_players: Vec<(SteamID, &GameInfo)> = players
        .iter()
        .filter(|&(_, gi)| gi.team == Team::Blu)
        .copied()
        .collect();
    let team_blu = team_blu_players
        .iter()
        .fold(
            column![
                text(format!("Blu ({})", team_blu_players.len()))
                    .size(20)
                    .style(Color::new(0.34, 0.52, 0.63, 1.0)),
                Space::with_height(10)
            ],
            |col, &(s, gi)| col.push(player::row(state, gi, s, &state.pfp_cache)),
        )
        .width(Length::Fill)
        .padding(10)
        .spacing(3)
        .align_items(iced::Alignment::Center);

    let team_other_players: Vec<(SteamID, &GameInfo)> = players
        .iter()
        .filter(|&(_, gi)| gi.team != Team::Red && gi.team != Team::Blu)
        .copied()
        .collect();
    let team_other = if team_other_players.is_empty() {
        None
    } else {
        Some(
            team_other_players
                .iter()
                .filter(|&&(_, gi)| gi.team != Team::Red && gi.team != Team::Blu)
                .fold(
                    column![
                        text(format!(
                            "Spectators / Unassigned ({})",
                            team_other_players.len()
                        ))
                        .size(20),
                        Space::with_height(10)
                    ],
                    |col, &(s, gi)| col.push(player::row(state, gi, s, &state.pfp_cache)),
                )
                .width(Length::Fill)
                .padding(10)
                .spacing(3)
                .align_items(iced::Alignment::Center),
        )
    };

    let mut contents = column![row![team_red, team_blu]];
    if let Some(others) = team_other {
        contents = contents.push(others);
    }

    Container::new(Scrollable::new(contents)).width(Length::Fill)
}
