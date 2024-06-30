use iced::{
    widget::{self, scrollable::Id, Container, Scrollable},
    Alignment, Color, Length,
};

use crate::{App, IcedContainer, Message};

use super::FONT_SIZE;

pub const SCROLLABLE_ID: &str = "Kills";

#[must_use]
pub fn view(state: &App) -> IcedContainer<'_> {
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
                }

                row = row.push(Container::new(killer_name).width(Length::FillPortion(1)));

                // Weapon
                let mut weapon = widget::text(&kill.weapon).size(FONT_SIZE);
                if kill.crit {
                    weapon = weapon.style(Color::from_rgb(1.0, 0.0, 0.0));
                }

                row = row.push(
                    Container::new(weapon).width(Length::FillPortion(1)), // .center_x(),
                );

                // Victim name
                let mut victim_name =
                    widget::button(widget::text(&kill.victim_name).size(FONT_SIZE)).padding(2);

                if let Some(steamid) = kill.victim_steamid {
                    victim_name = victim_name.on_press(Message::SelectPlayer(steamid));
                }

                let row = row.push(
                    Container::new(victim_name).width(Length::FillPortion(1)), // .align_x(Horizontal::Right),
                );

                row.push(widget::horizontal_space(5.0))
            })
        },
    );

    Container::new(
        Scrollable::new(contents)
            .id(Id::new(SCROLLABLE_ID))
            .on_scroll(|v| Message::ScrolledKills(v.relative_offset())),
    )
}
