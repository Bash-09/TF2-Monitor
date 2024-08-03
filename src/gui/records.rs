use iced::{
    widget::{self, text, text_input, Button, Scrollable, Space},
    Length,
};
use tf2_monitor_core::{player_records::Verdict, steamid_ng::SteamID};

use super::{copy_button, open_profile_button, verdict_picker, FONT_SIZE, PFP_SMALL_SIZE};
use crate::{App, IcedElement, Message, ALIAS_KEY};

#[must_use]
pub fn view(state: &App) -> IcedElement<'_> {
    // Pages
    let num_pages = state.records_to_display.len() / state.records_per_page + 1;
    let displaying_start =
        (state.record_page * state.records_per_page + 1).min(state.records_to_display.len());
    let displaying_end = if state.record_page == num_pages - 1 {
        (num_pages - 1) * state.records_per_page
            + state.records_to_display.len() % state.records_per_page
    } else {
        (state.record_page + 1) * state.records_per_page
    };

    let button = |contents: &str| {
        widget::button(
            widget::column![widget::text(contents)]
                .width(25)
                .align_items(iced::Alignment::Center),
        )
    };

    let header = widget::row![
        widget::Space::with_width(15),
        button("<<").on_press(Message::SetRecordPage(0)),
        button("<").on_press(Message::SetRecordPage(state.record_page.saturating_sub(1))),
        widget::column![text(format!("{}", state.record_page + 1))]
            .align_items(iced::Alignment::Center)
            .width(75),
        button(">").on_press(Message::SetRecordPage(
            state.record_page.saturating_add(1).min(num_pages - 1)
        )),
        button(">>").on_press(Message::SetRecordPage(num_pages - 1)),
        widget::horizontal_space(),
        widget::text(format!(
            "Displaying {displaying_start} - {displaying_end} of {} ({num_pages} {})",
            state.records_to_display.len(),
            if num_pages == 1 { "page" } else { "pages" }
        )),
        widget::Space::with_width(15),
    ]
    .spacing(3)
    .align_items(iced::Alignment::Center);

    let filter_checkbox = |v: Verdict| {
        widget::checkbox(format!("{v}"), state.record_verdict_whitelist.contains(&v))
            .on_toggle(move |_| Message::ToggleVerdictFilter(v))
    };

    let filters = widget::row![
        widget::Space::with_width(0),
        filter_checkbox(Verdict::Trusted),
        filter_checkbox(Verdict::Player),
        filter_checkbox(Verdict::Suspicious),
        filter_checkbox(Verdict::Cheater),
        filter_checkbox(Verdict::Bot),
        text_input("Search", &state.record_search).on_input(Message::SetRecordSearch),
        widget::Space::with_width(0),
    ]
    .spacing(15)
    .align_items(iced::Alignment::Center);

    // Records
    let mut contents = widget::column![].spacing(3).padding(15);
    for &s in state
        .records_to_display
        .iter()
        .skip(state.record_page * state.records_per_page)
        .take(state.records_per_page)
    {
        contents = contents.push(row(state, s));
    }

    widget::column![
        widget::Space::with_height(15),
        header,
        widget::Space::with_height(15),
        filters,
        widget::Space::with_height(15),
        widget::horizontal_rule(1),
        Scrollable::new(contents)
    ]
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}

#[must_use]
fn row(state: &App, steamid: SteamID) -> IcedElement<'_> {
    let record = state.mac.players.records.get(&steamid);

    let mut contents = widget::row![]
        .spacing(5)
        .align_items(iced::Alignment::Center);

    // Verdict picker
    contents = contents.push(verdict_picker(state.mac.players.verdict(steamid), steamid));

    // SteamID
    contents = contents.push(
        Button::new(text(format!("{}", u64::from(steamid))).size(FONT_SIZE))
            .on_press(crate::Message::SelectPlayer(steamid)),
    );
    contents = contents.push(copy_button(format!("{}", u64::from(steamid))));
    contents = contents.push(open_profile_button("Open", steamid));

    // Pfp
    if let Some((_, pfp)) = state
        .mac
        .players
        .steam_info
        .get(&steamid)
        .map(|si| &si.pfp_hash)
        .and_then(|pfp_hash| state.pfp_cache.get(pfp_hash))
    {
        contents = contents.push(
            widget::image(pfp.clone())
                .width(PFP_SMALL_SIZE)
                .height(PFP_SMALL_SIZE),
        );
    }

    #[allow(clippy::option_if_let_else, clippy::manual_map)]
    let name_text = if let Some(alias) =
        record.and_then(|r| r.custom_data().get(ALIAS_KEY).and_then(|v| v.as_str()))
    {
        Some(alias)
    } else {
        state.mac.players.get_name(steamid)
    };

    if let Some(name_text) = name_text {
        contents = contents.push(Space::with_width(10));
        contents = contents.push(widget::text(name_text));
    }

    contents = contents.push(widget::horizontal_space());
    contents = contents.push(super::player::badges(state, steamid, None));
    contents = contents.push(widget::Space::with_width(5));

    contents
        .align_items(iced::Alignment::Center)
        .height(PFP_SMALL_SIZE)
        .width(Length::Fill)
        .into()
}
