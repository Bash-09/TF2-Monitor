use iced::{alignment::Horizontal, Font};

pub const SHIELD: char = '\u{f132}';
pub const HIDDEN: char = '\u{f21b}';
pub const MUTED: char = '\u{e801}';
pub const NOTES: char = '\u{e802}';
pub const EDIT: char = '\u{e803}';
pub const STAR: char = '\u{e804}';
pub const REFRESH: char = '\u{e805}';
pub const FRIEND: char = '\u{e807}';
pub const PARTY: char = '\u{e800}';
pub const DOWNLOAD: char = '\u{e806}';
pub const BLOCK: char = '\u{e808}';
pub const HOURGLASS: char = '\u{f252}';
pub const DISCONNECT: char = '\u{f1e6}';
pub const JOINING: char = '\u{e809}';
pub const TICK: char = '\u{f14a}';
pub const CROSS: char = '\u{e80a}';

// Generated using https://fontello.com
pub const FONT_FILE: &[u8] = include_bytes!("../../icons.ttf");

#[must_use]
pub fn icon(codepoint: char) -> iced::widget::Text<'static, iced::Theme, iced::Renderer> {
    const ICON_FONT: Font = Font::with_name("icons");

    iced::widget::text(codepoint)
        .font(ICON_FONT)
        .width(15)
        .horizontal_alignment(Horizontal::Center)
}
