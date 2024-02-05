use client_backend::{player_records::PlayerRecord, steamid_ng::SteamID};
use iced::{
    widget::{self, text, Button, Container, Scrollable, Space},
    Length,
};

use super::{copy_button, open_profile_button, verdict_picker, FONT_SIZE};
use crate::{App, IcedContainer, ALIAS_KEY};

#[must_use]
pub fn view(state: &App) -> IcedContainer<'_> {
    let mut contents = widget::column![].spacing(3).padding(15);

    let mut records: Vec<(SteamID, &PlayerRecord)> = state
        .mac
        .players
        .records
        .iter()
        .map(|(s, r)| (*s, r))
        .collect();
    records.sort_by_key(|(_, r)| r.modified);

    for (s, r) in records {
        contents = contents.push(row(state, s, r));
    }

    Container::new(Scrollable::new(contents))
        .width(Length::Fill)
        .height(Length::Fill)
    // .padding(15)
}

#[must_use]
fn row<'a>(state: &'a App, steamid: SteamID, record: &'a PlayerRecord) -> IcedContainer<'a> {
    let mut contents = widget::row![]
        .spacing(5)
        .align_items(iced::Alignment::Center);

    // Verdict picker
    contents = contents.push(verdict_picker(record.verdict, steamid));

    // SteamID
    contents = contents.push(
        Button::new(text(format!("{}", u64::from(steamid))).size(FONT_SIZE))
            .on_press(crate::Message::SelectPlayer(steamid)),
    );
    contents = contents.push(copy_button(format!("{}", u64::from(steamid))));
    contents = contents.push(open_profile_button("Open", steamid));

    #[allow(clippy::option_if_let_else, clippy::manual_map)]
    let name_text = if let Some(alias) = record.custom_data.get(ALIAS_KEY).and_then(|v| v.as_str())
    {
        Some(alias.into())
    } else if let Some(game_info) = state.mac.players.game_info.get(&steamid) {
        Some(game_info.name.clone())
    } else if let Some(steam_info) = state.mac.players.steam_info.get(&steamid) {
        Some(steam_info.account_name.clone())
    } else {
        None
    };

    if let Some(name_text) = name_text {
        contents = contents.push(Space::with_width(10));
        contents = contents.push(widget::text(name_text));
    }

    Container::new(contents)
        .width(Length::Fill)
        .height(Length::Shrink)
}
