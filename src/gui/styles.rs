pub mod picklist;

pub mod colours {
    use iced::Color;

    #[must_use]
    pub const fn red() -> Color {
        Color::from_rgb(1.0, 0.3, 0.3)
    }

    #[must_use]
    pub const fn pink() -> Color {
        Color::from_rgb(1.0, 0.6, 0.6)
    }

    #[must_use]
    pub const fn green() -> Color {
        Color::from_rgb(0.2, 1.0, 0.2)
    }

    #[must_use]
    pub const fn yellow() -> Color {
        Color::from_rgb(1.0, 1.0, 0.5)
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
}
