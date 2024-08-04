use std::time::SystemTime;

use iced::{
    widget::{self, scrollable::Properties, Scrollable},
    Length,
};
use tf2_monitor_core::{demo_analyser::AnalysedDemo, steamid_ng::SteamID};

use crate::{
    demos::{DemosMessage, CLASSES},
    App, IcedElement, Message,
};

use super::{
    format_time, format_time_since,
    icons::{self, icon},
    styles::colours,
    tooltip, View, FONT_SIZE, PFP_SMALL_SIZE,
};

#[allow(clippy::module_name_repetitions)]
pub fn demos_list_view(state: &App) -> IcedElement<'_> {
    // Pages
    let num_pages = state.demos.demo_files.len() / state.demos.demos_per_page + 1;
    let displaying_start =
        (state.demos.page * state.demos.demos_per_page + 1).min(state.demos.demo_files.len());
    let displaying_end = if state.demos.page == num_pages - 1 {
        (num_pages - 1) * state.demos.demos_per_page
            + state.demos.demo_files.len() % state.demos.demos_per_page
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

    let header = widget::row![
        widget::Space::with_width(15),
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
        widget::button(widget::text("Analyse all")).on_press(DemosMessage::AnalyseAll.into()),
        widget::Space::with_width(Length::FillPortion(1)),
        widget::text(format!(
            "Displaying {displaying_start} - {displaying_end} of {} ({num_pages} {})",
            state.demos.demo_files.len(),
            if num_pages == 1 { "page" } else { "pages" }
        )),
        widget::Space::with_width(15),
    ]
    .spacing(3)
    .align_items(iced::Alignment::Center);

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
        widget::Space::with_height(15),
        header,
        widget::Space::with_height(15),
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
    if let Some(analysed) = state.demos.analysed_demos.get(&demo.analysed) {
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
            .width(Length::FillPortion(3));

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
                .width(65),
        );
    } else {
        let analysing = state.demos.analysing_demos.contains(&demo.path);

        let mut analyse_button = widget::button(widget::text("Analyse demo").size(FONT_SIZE));

        if !analysing {
            analyse_button = analyse_button
                .on_press(Message::Demos(DemosMessage::AnalyseDemo(demo.path.clone())));
        }

        contents = contents.push(analyse_button);
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

#[allow(clippy::too_many_lines)]
pub fn analysed_demo_view(state: &App, demo_index: usize) -> IcedElement<'_> {
    let Some(demo) = state.demos.demo_files.get(demo_index) else {
        return widget::column![
            widget::vertical_space(),
            widget::text("Invalid demo"),
            widget::vertical_space()
        ]
        .width(Length::Fill)
        .align_items(iced::Alignment::Center)
        .into();
    };

    let demo_name_button = tooltip(
        widget::button(widget::text(&demo.name).size(FONT_SIZE)).on_press(
            Message::CopyToClipboard(demo.path.to_string_lossy().to_string()),
        ),
        widget::text("Copy file path"),
    );

    let mut open_folder_button = widget::button("Open folder");
    if let Some(path) = demo.path.parent().and_then(|p| p.to_str()) {
        open_folder_button = open_folder_button.on_press(Message::Open(path.to_string()));
    }

    // Demo name, size, buttons
    let mut contents = widget::column![
        widget::Space::with_height(0),
        widget::row![
            widget::Space::with_width(0),
            demo_name_button,
            widget::text(format!("{:.2} MB", demo.file_size as f32 / 1_000_000.0)),
            widget::horizontal_space(),
            widget::text(format!(
                "Created {}",
                format_time_since(
                    SystemTime::now()
                        .duration_since(demo.created)
                        .unwrap_or_default()
                        .as_secs()
                )
            )),
            open_folder_button,
            widget::button("Create replay").on_press(Message::SetReplay(demo.path.clone())),
            widget::Space::with_width(0),
        ]
        .align_items(iced::Alignment::Center)
        .spacing(15)
    ]
    .width(Length::Fill)
    .spacing(15);

    let Some(analysed) = state.demos.analysed_demos.get(&demo.analysed) else {
        contents = contents.push(widget::text("Demo not analysed"));
        return contents.into();
    };

    // Server name, IP, duration
    contents = contents.push(
        widget::row![
            widget::Space::with_width(0),
            widget::text(&analysed.server_name),
            widget::Space::with_width(10),
            widget::text(format!("({})", analysed.header.server)),
            widget::horizontal_space(),
            widget::text(format_time(analysed.header.duration as u32)),
            widget::Space::with_width(0),
        ]
        .align_items(iced::Alignment::Center)
        .spacing(15),
    );

    // Players heading
    let mut player_classes_heading = widget::row![
        widget::Space::with_width(0),
        widget::text("Player").width(150),
        widget::text("Total").width(Length::FillPortion(1)),
    ]
    .spacing(15)
    .align_items(iced::Alignment::Center);

    for c in CLASSES {
        player_classes_heading = player_classes_heading.push(tooltip(
            icon(icons::CLASS[c as usize])
                .width(Length::FillPortion(1))
                .style(colours::orange()),
            widget::text(format!("{c:?}")),
        ));
    }
    player_classes_heading = player_classes_heading.push(widget::Space::with_width(15));

    contents = contents.push(player_classes_heading);

    // Player list
    let mut player_list = widget::column![].spacing(5);
    player_list = player_list.push(player_row(analysed, analysed.user));
    for s in analysed
        .players
        .keys()
        .copied()
        .filter(|s| *s != analysed.user)
    {
        player_list = player_list.push(player_row(analysed, s));
    }

    contents = contents.push(widget::scrollable(player_list).direction(
        widget::scrollable::Direction::Vertical(Properties::default()),
    ));
    contents = contents.push(widget::Space::with_height(15));

    contents.into()
}

fn player_row(analysed: &AnalysedDemo, steamid: SteamID) -> IcedElement<'_> {
    let Some(player) = analysed.players.get(&steamid) else {
        return widget::row![widget::text("Invalid Player")]
            .height(PFP_SMALL_SIZE)
            .align_items(iced::Alignment::Center)
            .into();
    };

    let format_kda = |kills, deaths, assists| {
        widget::row![
            widget::text(kills).style(colours::green()).size(FONT_SIZE),
            widget::text(" / ").size(FONT_SIZE),
            widget::text(deaths).style(colours::red()).size(FONT_SIZE),
            widget::text(" / ").size(FONT_SIZE),
            widget::text(assists)
                .style(colours::team_blu())
                .size(FONT_SIZE),
        ]
    };

    let mut contents = widget::row![
        widget::Space::with_width(0),
        widget::column![widget::button(widget::text(&player.name).size(FONT_SIZE))
            .on_press(Message::SelectPlayer(steamid))]
        .width(150),
        widget::column![
            widget::text(format_time(player.time)).size(FONT_SIZE),
            format_kda(
                player.kills.len() as u32,
                player.deaths.len() as u32,
                player.assists.len() as u32
            ),
        ]
        .align_items(iced::Alignment::Center)
        .width(Length::FillPortion(1))
    ]
    .spacing(15)
    .align_items(iced::Alignment::Center);

    for c in CLASSES {
        let details = &player.class_details[c as usize];

        if details.time == 0 {
            contents = contents.push(widget::column![].width(Length::FillPortion(1)));
            continue;
        }

        contents = contents.push(
            widget::column![
                widget::text(format_time(details.time)).size(FONT_SIZE),
                format_kda(details.num_kills, details.num_deaths, details.num_assists),
            ]
            .align_items(iced::Alignment::Center)
            .width(Length::FillPortion(1)),
        );
    }
    contents = contents.push(widget::Space::with_width(15));

    contents.width(Length::Fill).into()
}
