use iced::Length;
use plotters::{
    element::Rectangle,
    series::{AreaSeries, LineSeries},
    style::{IntoFont, RGBAColor, RGBColor, BLUE, GREEN, RED},
};
use plotters_iced::{Chart, ChartWidget};
use tf2_monitor_core::{
    demo_analyser::{ClassPeriod, Death, TeamPeriod},
    steamid_ng::SteamID,
    tf_demo_parser::demo::parser::analyser::Team,
};

use crate::{
    gui::styles::colours::{team_blu, team_red},
    App, IcedElement, Message,
};

#[derive(Debug, Clone, Default)]
pub struct KDAChart {
    pub kills: Vec<Death>,
    pub k: Vec<usize>,
    pub d: Vec<usize>,
    pub a: Vec<usize>,
    pub col: RGBAColor,
    pub ticks_on_classes: Vec<ClassPeriod>,
    pub ticks_on_teams: Vec<TeamPeriod>,
    pub first_tick: u32,
    pub last_tick: u32,
}

impl KDAChart {
    /// Provided a player who is in the demo, the graph will reflect that player's k/d/a.
    /// If the provided player is not contained in the demo, or no player is provided,
    /// it defaults to tracking the user who recorded the demo.
    pub fn new(state: &App, demo: usize, player: Option<SteamID>) -> Self {
        let mut chart = Self::default();

        let col = state.settings.theme.palette().text;
        chart.col = RGBAColor(
            (col.r * 255.0) as u8,
            (col.g * 255.0) as u8,
            (col.b * 255.0) as u8,
            0.2,
        );

        if let Some(analysed_demo) = state
            .demos
            .demo_files
            .get(demo)
            .map(|d| &d.analysed)
            .and_then(|d| state.demos.analysed_demos.get(d))
            .and_then(|d| d.get_demo())
        {
            let mut player = player.unwrap_or(analysed_demo.user);
            if !analysed_demo.players.contains_key(&player) {
                player = analysed_demo.user;
            }

            let Some(analysed_player) = analysed_demo.players.get(&player) else {
                return chart;
            };

            // chart.player = analysed_demo
            //     .players
            //     .get(&player)
            //     .map(|p| p.name.clone())
            //     .unwrap_or_default();

            chart.kills.clone_from(&analysed_demo.kills);
            chart.k.clone_from(&analysed_player.kills);
            chart.d.clone_from(&analysed_player.deaths);
            chart.a.clone_from(&analysed_player.assists);
            chart
                .ticks_on_teams
                .clone_from(&analysed_player.ticks_on_teams);
            chart
                .ticks_on_classes
                .clone_from(&analysed_player.ticks_on_classes);
            chart.first_tick = analysed_player.first_tick;
            chart.last_tick = analysed_player.last_tick;
        }

        chart
    }
}

impl Chart<Message> for KDAChart {
    type State = ();

    fn build_chart<DB: plotters::prelude::DrawingBackend>(
        &self,
        _state: &Self::State,
        mut chart: plotters::prelude::ChartBuilder<DB>,
    ) {
        const POINT_SIZE: u32 = 2;

        let max_kills = self.k.len().max(self.d.len().max(self.a.len()));

        let mut chart = chart
            .margin(10)
            .x_label_area_size(50)
            .y_label_area_size(20)
            .build_cartesian_2d(self.first_tick..self.last_tick, 0..max_kills)
            .expect("Chart stuff");
        let col_rgb = RGBColor(self.col.0, self.col.1, self.col.2);
        let text_style = ("sans-serif", 13).into_font().color(&col_rgb);

        chart
            .configure_mesh()
            .y_labels(15)
            .x_labels(15)
            .x_label_formatter(&format_fn)
            .x_label_style(text_style.clone())
            .y_label_style(text_style)
            .x_desc("Tick")
            .axis_style(col_rgb)
            .bold_line_style(self.col)
            .draw()
            .expect("Chart stuff");

        // Team backgrounds
        for p in &self.ticks_on_teams {
            let red = team_red();
            let blu = team_blu();
            let team_col = match p.team {
                Team::Other => RGBAColor(0, 0, 0, 0.0),
                Team::Spectator => RGBAColor(128, 128, 128, 0.2),
                Team::Red => RGBAColor(
                    (red.r * 255.0) as u8,
                    (red.g * 255.0) as u8,
                    (red.b * 255.0) as u8,
                    0.2,
                ),
                Team::Blue => RGBAColor(
                    (blu.r * 255.0) as u8,
                    (blu.g * 255.0) as u8,
                    (blu.b * 255.0) as u8,
                    0.2,
                ),
            };

            chart
                .draw_series(AreaSeries::new(
                    [(p.start, max_kills), (p.start + p.duration, max_kills)],
                    0,
                    team_col,
                ))
                .expect("Chart stuff");
        }

        // Kills
        chart
            .draw_series(
                LineSeries::new(
                    self.k
                        .iter()
                        .enumerate()
                        .map(|(i, &k)| (self.kills[k].tick.0, i + 1)),
                    GREEN,
                )
                .point_size(POINT_SIZE),
            )
            .expect("Chart stuff")
            .label("Kills")
            .legend(|(x, y)| Rectangle::new([(x, y + 2), (x + 15, y + 1)], GREEN));

        // Deaths
        chart
            .draw_series(
                LineSeries::new(
                    self.d
                        .iter()
                        .enumerate()
                        .map(|(i, &d)| (self.kills[d].tick.0, i + 1)),
                    RED,
                )
                .point_size(POINT_SIZE),
            )
            .expect("Chart stuff")
            .label("Deaths")
            .legend(|(x, y)| Rectangle::new([(x, y + 2), (x + 15, y + 1)], RED));

        // Assists
        chart
            .draw_series(
                LineSeries::new(
                    self.a
                        .iter()
                        .enumerate()
                        .map(|(i, &a)| (self.kills[a].tick.0, i + 1)),
                    BLUE,
                )
                .point_size(POINT_SIZE),
            )
            .expect("Chart stuff")
            .label("Assists")
            .legend(|(x, y)| Rectangle::new([(x, y + 2), (x + 15, y + 1)], BLUE));

        // Crit kills
        // chart.draw_series(PointSeries::new(
        //             self.a
        //                 .iter()
        //                 .enumerate()
        //                 .map(|(i, &a)| (self.kills[a].tick.0, i + 1)),
        //     POINT_SIZE,
        //     YELLOW
        // )).expect("Chart stuff");

        chart
            .configure_series_labels()
            .position(plotters::chart::SeriesLabelPosition::UpperLeft)
            .margin(10)
            .background_style(self.col)
            .draw()
            .expect("Chart stuff");
    }
}

pub fn view(state: &App) -> IcedElement<'_> {
    ChartWidget::new(&state.demos.chart)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

#[allow(clippy::trivially_copy_pass_by_ref)]
fn format_fn(c: &u32) -> String {
    format!("{}k", c / 1000)
}
