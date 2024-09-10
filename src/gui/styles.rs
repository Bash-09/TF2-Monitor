use iced::{widget::button, Color};

pub mod picklist;

pub struct ButtonColor(pub iced::Color);

impl button::StyleSheet for ButtonColor {
    fn active(&self, _style: &Self::Style) -> button::Appearance {
        button::Appearance {
            background: Some(iced::Background::Color(self.0)),
            text_color: Color::WHITE,
            ..Default::default()
        }
    }

    type Style = iced::Theme;
    // other methods in Stylesheet have a default impl
}
pub mod colours {
    use iced::Color;

    #[must_use]
    pub const fn red() -> Color {
        Color::from_rgb(1.0, 0.2, 0.2)
    }

    #[must_use]
    pub const fn pink() -> Color {
        Color::from_rgb(1.0, 0.6, 0.6)
    }

    #[must_use]
    pub const fn green() -> Color {
        Color::from_rgb(0.2, 8.0, 0.2)
    }

    #[must_use]
    pub const fn yellow() -> Color {
        Color::from_rgb(1.0, 1.0, 0.4)
    }

    #[must_use]
    pub const fn orange() -> Color {
        Color::from_rgb(1.0, 0.75, 0.25)
    }

    #[must_use]
    pub fn team_red() -> Color {
        Color::from_rgb(184.0 / 255.0, 56.0 / 255.0, 59.0 / 255.0)
    }

    #[must_use]
    pub fn team_blu() -> Color {
        Color::from_rgb(88.0 / 255.0, 133.0 / 255.0, 162.0 / 255.0)
    }
    #[must_use]
    pub fn team_red_darker() -> Color {
        Color::from_rgb(164.0 / 255.0, 36.0 / 255.0, 39.0 / 255.0)
    }

    #[must_use]
    pub fn team_blu_darker() -> Color {
        Color::from_rgb(68.0 / 255.0, 113.0 / 255.0, 162.0 / 255.0)
    }
}
