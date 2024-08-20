use std::{collections::HashSet, fmt::Display};

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::gui::{SidePanel, View};

pub const SETTINGS_IDENTIFIER: &str = "MACClientSettings";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
#[allow(clippy::module_name_repetitions)]
pub struct AppSettings {
    pub window_pos: Option<(i32, i32)>,
    pub window_size: Option<(u32, u32)>,
    pub enable_mac_integration: bool,
    pub view: View,
    pub sidepanels: HashSet<SidePanel>,
    pub panel_side: PanelSide,
    #[serde(serialize_with = "serialize_theme")]
    #[serde(deserialize_with = "deserialize_theme")]
    pub theme: iced::Theme,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            window_pos: None,
            window_size: None,
            enable_mac_integration: false,
            view: View::Server,
            sidepanels: HashSet::new(),
            panel_side: PanelSide::Right,
            theme: iced::Theme::CatppuccinMocha,
        }
    }
}

#[derive(Debug, Copy, Clone, Serialize, Deserialize)]
pub enum PanelSide {
    Left,
    Right,
}

impl Display for PanelSide {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

pub const THEMES: &[iced::Theme] = &[
    iced::Theme::Light,
    iced::Theme::Dark,
    iced::Theme::Dracula,
    iced::Theme::Nord,
    iced::Theme::SolarizedLight,
    iced::Theme::SolarizedDark,
    iced::Theme::GruvboxLight,
    iced::Theme::GruvboxDark,
    iced::Theme::CatppuccinLatte,
    iced::Theme::CatppuccinFrappe,
    iced::Theme::CatppuccinMacchiato,
    iced::Theme::CatppuccinMocha,
    iced::Theme::TokyoNight,
    iced::Theme::TokyoNightStorm,
    iced::Theme::TokyoNightLight,
    iced::Theme::KanagawaWave,
    iced::Theme::KanagawaDragon,
    iced::Theme::KanagawaLotus,
    iced::Theme::Moonfly,
    iced::Theme::Nightfly,
    iced::Theme::Oxocarbon,
];

pub const THEME_NAMES: &[&str] = &[
    "Light",
    "Dark",
    "Dracula",
    "Nord",
    "SolarizedLight",
    "SolarizedDark",
    "GruvboxLight",
    "GruvboxDark",
    "CatppuccinLatte",
    "CatppuccinFrappe",
    "CatppuccinMacchiato",
    "CatppuccinMocha",
    "TokyoNight",
    "TokyoNightStorm",
    "TokyoNightLight",
    "KanagawaWave",
    "KanagawaDragon",
    "KanagawaLotus",
    "Moonfly",
    "Nightfly",
    "Oxocarbon",
];

fn serialize_theme<S: Serializer>(theme: &iced::Theme, s: S) -> Result<S::Ok, S::Error> {
    debug_assert_eq!(THEMES.len(), THEME_NAMES.len());
    let Some(i) = THEMES
        .iter()
        .enumerate()
        .find(|(_, t)| *t == theme)
        .map(|(i, _)| i)
    else {
        return s.serialize_none();
    };

    s.serialize_str(THEME_NAMES[i])
}

fn deserialize_theme<'de, D: Deserializer<'de>>(d: D) -> Result<iced::Theme, D::Error> {
    debug_assert_eq!(THEMES.len(), THEME_NAMES.len());

    let s: String = Deserialize::deserialize(d)?;
    if let Some(i) = THEME_NAMES
        .iter()
        .enumerate()
        .find(|(_, theme)| **theme == s)
        .map(|(i, _)| i)
    {
        return Ok(THEMES[i].clone());
    }

    Err(serde::de::Error::custom(format!("Invalid theme \"{s}\"")))
}
