use iced::{widget, Length};
use plotters::{
    element::{PathElement, Rectangle},
    series::LineSeries,
    style::{
        Color, IntoFont, IntoTextStyle, RGBAColor, RGBColor, ShapeStyle, BLACK, BLUE, GREEN, RED,
        WHITE,
    },
};
use plotters_iced::{sample::lttb::LttbSource, Chart, ChartWidget};
use tf2_monitor_core::{demo_analyser::Death, steamid_ng::SteamID};

use crate::{App, IcedElement, Message};

#[derive(Debug, Clone, Default)]
pub struct KDAChart {
    pub kills: Vec<Death>,
    pub k: Vec<usize>,
    pub d: Vec<usize>,
    pub a: Vec<usize>,
    pub col: RGBAColor,
    pub player: String,
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
        {
            let mut player = player.unwrap_or(analysed_demo.user);
            if !analysed_demo.players.contains_key(&player) {
                player = analysed_demo.user;
            }

            chart.player = analysed_demo
                .players
                .get(&player)
                .map(|p| p.name.clone())
                .unwrap_or_default();

            chart.kills = analysed_demo.kills.clone();
            chart.k = analysed_demo
                .players
                .get(&player)
                .map(|p| p.kills.clone())
                .unwrap_or_default();
            chart.d = analysed_demo
                .players
                .get(&player)
                .map(|p| p.deaths.clone())
                .unwrap_or_default();
            chart.a = analysed_demo
                .players
                .get(&player)
                .map(|p| p.assists.clone())
                .unwrap_or_default();
        }

        chart
    }
}

impl Chart<Message> for KDAChart {
    type State = ();

    fn build_chart<DB: plotters::prelude::DrawingBackend>(
        &self,
        state: &Self::State,
        mut chart: plotters::prelude::ChartBuilder<DB>,
    ) {
        let last_tick = self.kills.last().map(|k| k.tick.0).unwrap_or(0);
        let num_kills = self.k.len().max(self.d.len().max(self.a.len()));
        let mut chart = chart
            .margin(10)
            .x_label_area_size(50)
            .y_label_area_size(20)
            .build_cartesian_2d(0..last_tick, 0..num_kills)
            .unwrap();
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
            .unwrap();

        const POINT_SIZE: u32 = 2;

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
            .unwrap()
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
            .unwrap()
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
            .unwrap()
            .label("Assists")
            .legend(|(x, y)| Rectangle::new([(x, y + 2), (x + 15, y + 1)], BLUE));

        chart
            .configure_series_labels()
            .position(plotters::chart::SeriesLabelPosition::UpperLeft)
            .margin(10)
            .background_style(self.col)
            .draw()
            .unwrap();
    }
}

pub fn view(state: &App) -> IcedElement<'_> {
    ChartWidget::new(&state.demos.chart)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

fn format_fn(c: &u32) -> String {
    format!("{}k", c / 1000)
}
