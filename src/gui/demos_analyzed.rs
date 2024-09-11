use std::time::SystemTime;

use iced::{
    widget::{
        self,
        scrollable::{Id, Properties},
    },
    Length,
};
use plotters_iced::ChartWidget;
use tf2_monitor_core::{
    demos::analyser::AnalysedDemo, steamid_ng::SteamID,
    tf_demo_parser::demo::parser::analyser::Class,
};

use crate::{
    demos::{AnalysedDemoView, CLASSES},
    App, IcedElement, Message,
};

use super::{
    coming_soon, format_time, format_time_since,
    icons::{self, icon},
    invalid_view,
    styles::colours,
    tooltip, FONT_SIZE, PFP_SMALL_SIZE,
};

pub const KDA_SCROLLABLE_ID: &str = "kda_table";

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

    let Some(analysed) = state
        .demos
        .analysed_demos
        .get(&demo.analysed)
        .and_then(|d| d.get_demo())
    else {
        contents = contents.push(widget::text("Demo not analysed"));
        return contents.into();
    };

    // Server name, IP, duration
    contents = contents.push(
        widget::row![
            widget::Space::with_width(0),
            widget::text(&analysed.header.map),
            widget::text("on").size(FONT_SIZE),
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

    // Tab selection
    contents = contents.push(view_select(state));
    contents = contents.push(widget::horizontal_rule(1));

    match state.settings.analysed_demo_view {
        AnalysedDemoView::Players => {
            if state
                .selected_player
                .is_some_and(|p| analysed.players.contains_key(&p))
            {
                contents = contents.push(widget::row![
                    kda_table(analysed, false).width(300),
                    widget::vertical_rule(1),
                    detailed_player_view(state, analysed),
                ]);
            } else {
                contents = contents.push(kda_table(analysed, true));
            }
        }
        AnalysedDemoView::Events => contents = contents.push(coming_soon()),
    }

    contents.into()
}

fn view_select(state: &App) -> IcedElement<'_> {
    const VIEWS: &[(&str, AnalysedDemoView)] = &[
        ("Players", AnalysedDemoView::Players),
        ("Events", AnalysedDemoView::Events),
    ];

    let mut views = widget::row![widget::Space::with_width(0)].spacing(10);
    for &(name, v) in VIEWS {
        let mut button = widget::Button::new(name);
        if state.settings.analysed_demo_view != v {
            button = button.on_press(Message::Demos(
                crate::demos::DemosMessage::SetAnalysedDemoView(v),
            ));
        }
        views = views.push(button);
    }

    views.width(Length::Fill).into()
}

fn detailed_player_view<'a>(state: &'a App, analysed: &AnalysedDemo) -> IcedElement<'a> {
    let Some(p) = state.selected_player.and_then(|p| analysed.players.get(&p)) else {
        return invalid_view(state);
    };

    let chart_width = 800.0;
    let chart_margin = 30.0;
    let scale = (chart_width - chart_margin)
        / (state
            .demos
            .chart
            .last_tick
            .saturating_sub(state.demos.chart.first_tick)
            .max(1)) as f32;

    let mut classes_timeline = widget::row![widget::Space::with_width(chart_margin)]
        .width(chart_width)
        .height(PFP_SMALL_SIZE);

    // let total_ticks = (state.demos.chart.last_tick - state.demos.chart.first_tick) as f32;
    let mut last = state.demos.chart.first_tick;
    for period in &state.demos.chart.ticks_on_classes {
        if period.class == Class::Other {
            continue;
        }

        let space = ((period.start.saturating_sub(last)) as f32 * scale) as u16;
        let width = (period.duration as f32 * scale) as u16;

        classes_timeline = classes_timeline.push(widget::vertical_rule(1));

        if period.start.saturating_sub(last) > 1000 {
            classes_timeline =
                classes_timeline.push(widget::Space::with_width(Length::FillPortion(space)));
            classes_timeline = classes_timeline.push(widget::vertical_rule(1));
        }

        classes_timeline = classes_timeline.push(tooltip(
            icon(icons::CLASS[period.class as usize])
                .style(colours::orange())
                .width(Length::FillPortion(width))
                .vertical_alignment(iced::alignment::Vertical::Center),
            widget::text(format!("{}", period.class)),
        ));
        last = period.start + period.duration;
    }
    classes_timeline = classes_timeline.push(widget::vertical_rule(1));

    widget::column![
        widget::row![
            widget::text(&p.name),
            format_kda(
                p.kills.len() as u32,
                p.deaths.len() as u32,
                p.assists.len() as u32
            ),
            widget::text(format_time(p.time)),
        ]
        .align_items(iced::Alignment::Center)
        .spacing(50),
        widget::scrollable(widget::row![
            widget::column![
                classes_timeline,
                ChartWidget::new(&state.demos.chart).height(Length::Fixed(400.0)),
            ]
            .width(Length::Fixed(chart_width)),
            widget::Space::with_width(5)
        ])
        .width(Length::Fill)
        .direction(widget::scrollable::Direction::Vertical(
            Properties::default()
        )),
    ]
    .width(Length::Fill)
    .align_items(iced::Alignment::Center)
    .into()
}

fn kda_table(
    analysed: &AnalysedDemo,
    show_classes: bool,
) -> widget::Column<'_, Message, iced::Theme, iced::Renderer> {
    // Players heading
    let mut player_classes_heading = widget::row![
        widget::Space::with_width(0),
        widget::text("Player").width(150),
        widget::text("Total")
            .width(80)
            .horizontal_alignment(iced::alignment::Horizontal::Center),
    ]
    .spacing(15)
    .align_items(iced::Alignment::Center);

    if show_classes {
        for c in CLASSES {
            player_classes_heading = player_classes_heading.push(tooltip(
                icon(icons::CLASS[c as usize])
                    .width(Length::FillPortion(1))
                    .style(colours::orange()),
                widget::text(format!("{c:?}")),
            ));
        }
        player_classes_heading = player_classes_heading.push(widget::Space::with_width(15));
    }

    // Player list
    let mut player_list = widget::column![].spacing(2);
    player_list = player_list.push(player_table_row(analysed, analysed.user, show_classes));
    for s in analysed
        .players
        .keys()
        .copied()
        .filter(|s| *s != analysed.user)
    {
        player_list = player_list.push(widget::horizontal_rule(1));
        player_list = player_list.push(player_table_row(analysed, s, show_classes));
    }
    player_list = player_list.push(widget::Space::with_height(15));

    let kda_table = widget::column![
        player_classes_heading,
        widget::row![
            widget::Space::with_width(15),
            widget::scrollable(player_list)
                .id(Id::new(KDA_SCROLLABLE_ID))
                .direction(widget::scrollable::Direction::Vertical(
                    Properties::default()
                ),)
        ],
    ]
    .spacing(15);
    // .width(Length::Fill);
    kda_table
}

fn player_table_row(
    analysed: &AnalysedDemo,
    steamid: SteamID,
    show_classes: bool,
) -> IcedElement<'_> {
    let Some(player) = analysed.players.get(&steamid) else {
        return widget::row![widget::text("Invalid Player")]
            .height(PFP_SMALL_SIZE)
            .align_items(iced::Alignment::Center)
            .into();
    };

    let mut contents = widget::row![
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
        .width(80)
    ]
    .spacing(15)
    .align_items(iced::Alignment::Center);

    if show_classes {
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
    }
    contents = contents.push(widget::Space::with_width(15));

    // contents.width(Length::Fill).into()
    contents.into()
}

fn format_kda<'a>(k: u32, d: u32, a: u32) -> IcedElement<'a> {
    widget::row![
        widget::text(k).style(colours::green()).size(FONT_SIZE),
        widget::text(" / ").size(FONT_SIZE),
        widget::text(d).style(colours::red()).size(FONT_SIZE),
        widget::text(" / ").size(FONT_SIZE),
        widget::text(a).style(colours::team_blu()).size(FONT_SIZE),
    ]
    .into()
}
