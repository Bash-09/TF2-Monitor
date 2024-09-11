use iced::{
    widget::{self, scrollable::Id, Container, Scrollable},
    Alignment, Length,
};
use tf2_monitor_core::players::game_info::Team;

use crate::{App, IcedElement, Message};

use super::{
    styles::{colours, ButtonColor},
    FONT_SIZE,
};

pub const SCROLLABLE_ID: &str = "Kills";

#[must_use]
pub fn view(state: &App) -> impl Into<IcedElement<'_>> {
    // TODO - Virtualise this by using the on_scroll thing

    let contents = state.mac.server.kill_history().iter().fold(
        widget::Column::new()
            .align_items(Alignment::Start)
            .padding(10)
            .spacing(5),
        |contents, kill| {
            contents.push({
                let mut row = widget::Row::new().align_items(Alignment::Center).spacing(5);

                // Killer name
                let mut killer_name =
                    widget::button(widget::text(&kill.killer_name).size(FONT_SIZE)).padding(2);

                if let Some(steamid) = kill.killer_steamid {
                    killer_name = killer_name.on_press(Message::SelectPlayer(steamid));

                    match state.mac.players.game_info.get(&steamid).map(|gi| gi.team) {
                        Some(Team::Red) => {
                            killer_name = killer_name.style(iced::theme::Button::custom(
                                ButtonColor(colours::team_red_darker()),
                            ));
                        }
                        Some(Team::Blu) => {
                            killer_name = killer_name.style(iced::theme::Button::custom(
                                ButtonColor(colours::team_blu_darker()),
                            ));
                        }
                        _ => {}
                    }
                }

                row = row.push(Container::new(killer_name).width(Length::FillPortion(1)));

                // Weapon
                let mut weapon = widget::text(&kill.weapon).size(FONT_SIZE);
                if kill.crit {
                    weapon = weapon.style(colours::yellow());
                }

                row = row.push(
                    Container::new(weapon).width(Length::FillPortion(1)), // .center_x(),
                );

                // Victim name
                let mut victim_name =
                    widget::button(widget::text(&kill.victim_name).size(FONT_SIZE)).padding(2);

                if let Some(steamid) = kill.victim_steamid {
                    victim_name = victim_name.on_press(Message::SelectPlayer(steamid));

                    match state.mac.players.game_info.get(&steamid).map(|gi| gi.team) {
                        Some(Team::Red) => {
                            victim_name = victim_name.style(iced::theme::Button::custom(
                                ButtonColor(colours::team_red_darker()),
                            ));
                        }
                        Some(Team::Blu) => {
                            victim_name = victim_name.style(iced::theme::Button::custom(
                                ButtonColor(colours::team_blu_darker()),
                            ));
                        }
                        _ => {}
                    }
                }

                let row = row.push(
                    Container::new(victim_name).width(Length::FillPortion(1)), // .align_x(Horizontal::Right),
                );

                row.push(widget::Space::with_width(5.0))
            })
        },
    );

    Scrollable::new(contents)
        .id(Id::new(SCROLLABLE_ID))
        .on_scroll(|v| Message::ScrolledKills(v.relative_offset()))
}
