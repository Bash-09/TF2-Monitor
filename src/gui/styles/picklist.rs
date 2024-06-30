use client_backend::player_records::Verdict;
use iced::{widget::pick_list, Color};

use super::colours;

pub struct VerdictPickList(pub Verdict);

impl iced::overlay::menu::StyleSheet for VerdictPickList {
    type Style = iced::Theme;

    fn appearance(&self, style: &Self::Style) -> iced::overlay::menu::Appearance {
        let palette = style.extended_palette();

        iced::overlay::menu::Appearance {
            text_color: palette.background.weak.text,
            background: palette.background.weak.color.into(),
            border_width: 1.0,
            border_radius: 0.0.into(),
            border_color: palette.background.strong.color,
            selected_text_color: palette.primary.strong.text,
            selected_background: palette.primary.strong.color.into(),
        }
    }
}

impl pick_list::StyleSheet for VerdictPickList {
    type Style = iced::Theme;

    fn active(&self, style: &Self::Style) -> pick_list::Appearance {
        let palette = style.extended_palette();

        let verdict_col = match self.0 {
            Verdict::Player => palette.background.weak.text,
            Verdict::Bot => colours::red(),
            Verdict::Suspicious => colours::pink(),
            Verdict::Cheater => colours::orange(),
            Verdict::Trusted => colours::green(),
        };

        pick_list::Appearance {
            text_color: verdict_col,
            background: palette.background.weak.color.into(),
            placeholder_color: palette.background.strong.color,
            handle_color: palette.background.weak.text,
            border_radius: 2.0.into(),
            border_width: 1.0,
            border_color: verdict_col,
        }
    }

    fn hovered(&self, style: &Self::Style) -> pick_list::Appearance {
        let palette = style.extended_palette();

        pick_list::Appearance {
            text_color: palette.background.weak.text,
            background: palette.background.weak.color.into(),
            placeholder_color: palette.background.strong.color,
            handle_color: palette.background.weak.text,
            border_radius: 2.0.into(),
            border_width: 1.0,
            border_color: palette.primary.strong.color,
        }
    }
}
