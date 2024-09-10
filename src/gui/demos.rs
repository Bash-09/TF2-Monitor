use std::time::SystemTime;

use iced::{
    widget::{self, scrollable::Properties, Scrollable},
    Length,
};

use crate::{
    demos::{DemosMessage, MaybeAnalysedDemo, SORT_DIRECTIONS, SORT_OPTIONS},
    App, IcedElement, Message,
};

use super::{
    format_time, format_time_since,
    icons::{self, icon},
    styles::colours,
    tooltip, View, FONT_SIZE, FONT_SIZE_HEADING, PFP_SMALL_SIZE,
};

#[allow(clippy::module_name_repetitions)]
pub fn demos_list_view(state: &App) -> IcedElement<'_> {
    // Pages
    let num_pages = state.demos.demos_to_display.len() / state.demos.demos_per_page + 1;
    let displaying_start =
        (state.demos.page * state.demos.demos_per_page + 1).min(state.demos.demos_to_display.len());
    let displaying_end = if state.demos.page == num_pages - 1 {
        (num_pages - 1) * state.demos.demos_per_page
            + state.demos.demos_to_display.len() % state.demos.demos_per_page
    } else {
        (state.demos.page + 1) * state.demos.demos_per_page
    };

    let arrow_button = |contents: &str| {
        widget::button(
            widget::column![widget::text(contents)]
                .width(25)
                .align_items(iced::Alignment::Center),
        )
    };

    let header = widget::column![
        widget::row![
            arrow_button("<<").on_press(DemosMessage::SetPage(0).into()),
            arrow_button("<")
                .on_press(DemosMessage::SetPage(state.demos.page.saturating_sub(1)).into()),
            widget::column![widget::text(format!("{}", state.demos.page + 1))]
                .align_items(iced::Alignment::Center)
                .width(75),
            arrow_button(">").on_press(
                DemosMessage::SetPage(state.demos.page.saturating_add(1).min(num_pages - 1)).into()
            ),
            arrow_button(">>").on_press(DemosMessage::SetPage(num_pages - 1).into()),
            widget::Space::with_width(Length::FillPortion(1)),
            widget::button(widget::text("Refresh")).on_press(DemosMessage::Refresh.into()),
            widget::Space::with_width(5),
            widget::button(widget::text("Analyse all")).on_press(DemosMessage::AnalyseAll.into()),
            widget::Space::with_width(Length::FillPortion(1)),
            widget::text(format!(
                "Displaying {displaying_start} - {displaying_end} of {} ({num_pages} {})",
                state.demos.demos_to_display.len(),
                if num_pages == 1 { "page" } else { "pages" }
            )),
        ]
        .spacing(5)
        .align_items(iced::Alignment::Center),
        widget::row![
            widget::text("Sort by: "),
            // Sort by
            widget::PickList::new(
                SORT_OPTIONS,
                Some(state.settings.demo_filters.sort_by),
                |s| { DemosMessage::FilterSortBy(s).into() }
            )
            .text_size(FONT_SIZE),
            // Direction
            widget::PickList::new(
                SORT_DIRECTIONS,
                Some(state.settings.demo_filters.direction),
                |s| { DemosMessage::FilterSortDirection(s).into() }
            )
            .text_size(FONT_SIZE),
            widget::horizontal_space(),
            tooltip(
                if state.demos.demos_to_display.len() == state.demos.demo_files.len() {
                    widget::text("All demos visible")
                } else {
                    widget::text(format!(
                        "{} demos hidden",
                        state.demos.demo_files.len() - state.demos.demos_to_display.len(),
                    ))
                },
                "Refresh, check filters, or change sorting method to see more"
            ),
        ]
        .spacing(5)
        .align_items(iced::Alignment::Center)
    ]
    .spacing(15)
    .padding(15);

    // Actual demos
    let mut contents = widget::column![].spacing(3).padding(15);

    for &d in state
        .demos
        .demos_to_display
        .iter()
        .skip(state.demos.page * state.demos.demos_per_page)
        .take(state.demos.demos_per_page)
    {
        contents = contents.push(demo_list_row(state, d));
    }

    widget::column![
        header,
        widget::horizontal_rule(1),
        Scrollable::new(contents)
    ]
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}

#[must_use]
#[allow(clippy::too_many_lines)]
fn demo_list_row(state: &App, demo_index: usize) -> IcedElement<'_> {
    let Some(demo) = state.demos.demo_files.get(demo_index) else {
        return widget::row![widget::text("Invalid demo")].into();
    };

    let recorded_ago = SystemTime::now()
        .duration_since(demo.created)
        .unwrap_or_default();

    let recorded_ago_str = format_time_since(recorded_ago.as_secs());

    let mut contents = widget::row![]
        .align_items(iced::Alignment::Center)
        .height(PFP_SMALL_SIZE)
        .spacing(15);

    // Analysed
    if let Some(analysed) = state
        .demos
        .analysed_demos
        .get(&demo.analysed)
        .and_then(|d| d.get_demo())
    {
        let hostname = if analysed.server_name.len() > 30 {
            let mut host = analysed.server_name.split_at(27).0.to_string();
            host.push_str("...");
            host
        } else {
            analysed.server_name.clone()
        };

        let map = if analysed.header.map.len() > 30 {
            let mut map = analysed.header.map.split_at(27).0.to_string();
            map.push_str("...");
            map
        } else {
            analysed.header.map.clone()
        };

        contents = contents.push(
            widget::row![widget::button(widget::text(hostname).size(FONT_SIZE))
                .on_press(Message::SetView(View::AnalysedDemo(demo_index)))]
            .width(200),
        );
        contents = contents.push(widget::text(recorded_ago_str).width(100));
        contents = contents.push(widget::text(map).width(Length::FillPortion(4)));

        let mut badges = widget::row![]
            .spacing(15)
            .align_items(iced::Alignment::Center)
            .width(220);

        if let Some(player) = analysed.players.get(&analysed.user) {
            badges = badges.push(tooltip(
                widget::row![
                    widget::text(player.kills.len()).style(colours::green()),
                    widget::text("/"),
                    widget::text(player.deaths.len()).style(colours::red()),
                    widget::text("/"),
                    widget::text(player.assists.len()).style(colours::team_blu()),
                ]
                .spacing(5),
                widget::text("Kills/Deaths/Assists"),
            ));
            badges = badges.push(widget::horizontal_space());

            for &c in player.most_played_classes.iter().take(3) {
                let details = &player.class_details[c as usize];
                let time_played = format_time(details.time);

                badges = badges.push(tooltip(
                    icon(icons::CLASS[c as usize]).style(colours::orange()),
                    widget::column![
                        widget::text(format!("{c:?}")),
                        widget::row![widget::text("Time played: "), widget::text(time_played),],
                        widget::row![
                            widget::text("K/D/A: "),
                            widget::text(format!(
                                "{}/{}/{}",
                                details.num_kills, details.num_deaths, details.num_assists
                            )),
                        ],
                    ],
                ));
            }
        } else {
            badges = badges.push(widget::text("Missing player"));
        }

        contents = contents.push(badges);

        // <Player> on <Server> (<map>) for <time>
        #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
        let duration = analysed.header.duration as u32;
        let duration = format_time(duration);

        contents = contents.push(
            widget::column![widget::text(duration)]
                .align_items(iced::Alignment::End)
                .width(70),
        );
    } else {
        let analysing = state.demos.analysed_demos.get(&demo.analysed);
        let not_analysed = analysing.is_none();
        let progress = analysing.and_then(MaybeAnalysedDemo::analysing_progress);

        let analyse_widget: IcedElement<'_> = if not_analysed {
            widget::button(widget::text("Analyse demo").size(FONT_SIZE))
                .on_press(Message::Demos(DemosMessage::AnalyseDemo(demo_index)))
                .into()
        } else if let Some(progress) = progress {
            match progress {
                tf2_monitor_core::demo_analyser::progress::Progress::Queued => {
                    widget::text("Queued...").into()
                }
                tf2_monitor_core::demo_analyser::progress::Progress::InProgress(amount) => {
                    widget::progress_bar(0.0..=1.0, amount).into()
                }
                tf2_monitor_core::demo_analyser::progress::Progress::Finished => {
                    widget::text("Done...").into()
                }
            }
        } else {
            widget::text("Should be analysed?").into()
        };

        contents = contents.push(widget::container(analyse_widget).width(200));
        contents = contents.push(widget::text(&demo.name).width(Length::FillPortion(2)));
        contents = contents.push(widget::text(recorded_ago_str).width(Length::FillPortion(1)));
        contents = contents.push(
            widget::text(format!("{:.2} MB", demo.file_size as f32 / 1_000_000.0))
                .width(Length::FillPortion(1)),
        );
    }

    // widget::column![top_row, bottom_row]
    contents.width(Length::Fill).into()
}

pub fn filters_view(state: &App) -> IcedElement<'_> {
    let mut contents = widget::column![
        widget::text("Filters").size(FONT_SIZE_HEADING),
        widget::checkbox(
            "Show analysed demos",
            state.settings.demo_filters.show_analysed
        )
        .on_toggle(|v| DemosMessage::FilterShowAnalysed(v).into()),
        widget::checkbox(
            "Show non-analysed demos",
            state.settings.demo_filters.show_non_analysed
        )
        .on_toggle(|v| DemosMessage::FilterShowNonAnalysed(v).into()),
        widget::text("Search (Map, Server, IP, File)").size(FONT_SIZE_HEADING),
        widget::text_input(
            "Search (map, server, ip, file)",
            &state.settings.demo_filters.search
        )
        .on_submit(Message::Demos(DemosMessage::ApplyFilters))
        .on_input(|s| DemosMessage::FilterSearchUpdate(s).into()),
        widget::text("Contains Players").size(FONT_SIZE_HEADING),
        widget::row![
            widget::text_input(
                "Player steamid or name",
                state
                    .settings
                    .demo_filters
                    .contains_players
                    .iter()
                    .last()
                    .map_or("", |s| s.as_str())
            )
            .on_submit(Message::Demos(DemosMessage::FilterContainsPlayerAdd))
            .on_input(|s| DemosMessage::FilterContainsPlayerUpdate(s).into()),
            widget::button("Add").on_press(Message::Demos(DemosMessage::FilterContainsPlayerAdd)),
        ]
        .spacing(15),
    ]
    .padding(15)
    .spacing(15);

    for (i, p) in state
        .settings
        .demo_filters
        .contains_players
        .iter()
        .enumerate()
        .rev()
        .skip(1)
    {
        contents = contents.push(
            widget::row![
                widget::button(
                    widget::column![icon(icons::MINUS)]
                        .width(20)
                        .align_items(iced::Alignment::Center),
                )
                .on_press(Message::Demos(DemosMessage::FilterRemovePlayer(i))),
                widget::text(p),
            ]
            .align_items(iced::Alignment::Center)
            .spacing(15),
        );
    }

    contents = contents.push(
        widget::button("Clear All Filters").on_press(Message::Demos(DemosMessage::ClearFilters)),
    );

    widget::Scrollable::new(contents)
        .direction(widget::scrollable::Direction::Vertical(
            Properties::default(),
        ))
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}
