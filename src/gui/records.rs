use client_backend::{
    player_records::{PlayerRecord, Verdict},
    steamid_ng::SteamID,
};
use iced::{
    widget::{self, text, text_input, Button, Container, Scrollable, Space},
    Length,
};

use super::{copy_button, open_profile_button, verdict_picker, FONT_SIZE};
use crate::{App, IcedContainer, Message, ALIAS_KEY};

#[allow(clippy::module_name_repetitions)]
pub fn get_filtered_records(state: &App) -> impl Iterator<Item = (SteamID, &PlayerRecord)> {
    state
        .mac
        .players
        .records
        .iter()
        .map(|(s, r)| (*s, r))
        .filter(|(_, r)| state.record_verdict_whitelist.contains(&r.verdict()))
        .filter(|(s, r)| {
            if state.record_search.is_empty() {
                return true;
            }

            r.previous_names()
                .iter()
                .any(|n| n.contains(&state.record_search))
                || state
                    .record_search
                    .parse::<u64>()
                    .is_ok_and(|_| format!("{}", u64::from(*s)).contains(&state.record_search))
                || state
                    .mac
                    .players
                    .get_name(*s)
                    .is_some_and(|n| n.contains(&state.record_search))
        })
}

#[must_use]
pub fn view(state: &App) -> IcedContainer<'_> {
    let mut records: Vec<(SteamID, &PlayerRecord)> = get_filtered_records(state).collect();
    records.sort_by_key(|(_, r)| r.modified());

    // Pages
    let num_pages = records.len() / state.records_per_page + 1;
    let displaying_start = (state.record_page * state.records_per_page + 1).min(records.len());
    let displaying_end = if state.record_page == num_pages - 1 {
        (num_pages - 1) * state.records_per_page + records.len() % state.records_per_page
    } else {
        (state.record_page + 1) * state.records_per_page
    };

    let button = |contents: &str| {
        widget::button(widget::column![text(contents)].align_items(iced::Alignment::Center))
            .width(30)
            .height(30)
    };

    let header = widget::row![
        widget::horizontal_space(15),
        button("<<").on_press(Message::SetRecordPage(0)),
        button("<").on_press(Message::SetRecordPage(state.record_page.saturating_sub(1))),
        widget::column![text(format!("{}", state.record_page + 1))]
            .align_items(iced::Alignment::Center)
            .width(75),
        button(">").on_press(Message::SetRecordPage(
            state.record_page.saturating_add(1).min(num_pages - 1)
        )),
        button(">>").on_press(Message::SetRecordPage(num_pages - 1)),
        widget::horizontal_space(Length::Fill),
        widget::text(format!(
            "Displaying {displaying_start} - {displaying_end} of {} ({num_pages} {})",
            records.len(),
            if num_pages == 1 { "page" } else { "pages" }
        )),
        widget::horizontal_space(15),
    ]
    .spacing(3)
    .align_items(iced::Alignment::Center);

    let filter_checkbox = |v: Verdict| {
        widget::checkbox(
            format!("{v}"),
            state.record_verdict_whitelist.contains(&v),
            move |_| Message::ToggleVerdictFilter(v),
        )
    };

    let filters = widget::row![
        filter_checkbox(Verdict::Trusted),
        filter_checkbox(Verdict::Player),
        filter_checkbox(Verdict::Suspicious),
        filter_checkbox(Verdict::Cheater),
        filter_checkbox(Verdict::Bot),
        // widget::horizontal_space(Length::Fill),
        text_input("Search", &state.record_search).on_input(Message::SetRecordSearch),
    ]
    .spacing(15)
    .align_items(iced::Alignment::Center)
    .padding(15);

    // Records
    let mut contents = widget::column![].spacing(3).padding(15);
    for (s, r) in records
        .into_iter()
        .skip(state.record_page * state.records_per_page)
        .take(state.records_per_page)
    {
        contents = contents.push(row(state, s, r));
    }

    Container::new(widget::column![
        widget::vertical_space(15),
        header,
        filters,
        Scrollable::new(contents)
    ])
    .width(Length::Fill)
    .height(Length::Fill)
}

#[must_use]
fn row<'a>(state: &'a App, steamid: SteamID, record: &'a PlayerRecord) -> IcedContainer<'a> {
    let mut contents = widget::row![]
        .spacing(5)
        .align_items(iced::Alignment::Center);

    // Verdict picker
    contents = contents.push(verdict_picker(record.verdict(), steamid));

    // SteamID
    contents = contents.push(
        Button::new(text(format!("{}", u64::from(steamid))).size(FONT_SIZE))
            .on_press(crate::Message::SelectPlayer(steamid)),
    );
    contents = contents.push(copy_button(format!("{}", u64::from(steamid))));
    contents = contents.push(open_profile_button("Open", steamid));

    #[allow(clippy::option_if_let_else, clippy::manual_map)]
    let name_text =
        if let Some(alias) = record.custom_data().get(ALIAS_KEY).and_then(|v| v.as_str()) {
            Some(alias.into())
        } else {
            state.mac.players.get_name(steamid)
        };

    if let Some(name_text) = name_text {
        contents = contents.push(Space::with_width(10));
        contents = contents.push(widget::text(name_text));
    }

    Container::new(contents)
        .width(Length::Fill)
        .height(Length::Shrink)
}
