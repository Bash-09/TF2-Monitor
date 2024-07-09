use iced::{
    widget::{self, Scrollable},
    Length,
};
use tf2_monitor_core::{
    events::{InternalPreferences, Preferences},
    settings::FriendsAPIUsage,
};

use crate::{gui::tooltip, App, IcedElement, MACMessage, Message};

#[allow(clippy::too_many_lines)]
#[must_use]
pub fn view(state: &App) -> IcedElement<'_> {
    const HEADING_SIZE: u16 = 25;
    const HEADING_SPACING: u16 = 15;
    const HALF_WIDTH: Length = Length::FillPortion(1);
    const ROW_SPACING: u16 = 15;

    const FRIENDS_API_USAGE_OPTIONS: &[FriendsAPIUsage] = &[
        FriendsAPIUsage::None,
        FriendsAPIUsage::CheatersOnly,
        FriendsAPIUsage::All,
    ];

    let heading = |heading: &str| {
        widget::row![
            widget::horizontal_space(),
            widget::text(heading).size(HEADING_SIZE),
            widget::horizontal_space()
        ]
    };

    Scrollable::new(

            // RCON
        widget::column![
            heading("Rcon"),

            // Rcon password
            widget::row![
                widget::row![
                    tooltip(widget::text("Rcon Password"), widget::text("The password used to connect to TF2 via Rcon. Set by rcon_password in your autoexec file.")),
                ].width(HALF_WIDTH),
                widget::text_input("Rcon password", state.mac.settings.rcon_password()).on_input(
                    |s| Message::MAC(MACMessage::Preferences(Preferences {
                        internal: Some(InternalPreferences {
                            friends_api_usage: None,
                            tf2_directory: None,
                            rcon_password: Some(s),
                            steam_api_key: None,
                            masterbase_key: None,
                            masterbase_host: None,
                            rcon_port: None,
                            dumb_autokick: None,
                        }),
                        external: None
                    }))
                ).width(HALF_WIDTH),
            ].align_items(iced::Alignment::Center)
            .spacing(ROW_SPACING),

            // Rcon port
            widget::row![
                widget::row![
                    tooltip("Rcon port", "The port used to connect to TF2 via Rcon. Defaults to 27015, or set by -port in your launch options."),
                ].width(HALF_WIDTH),
                widget::text_input("Rcon port", &format!("{}", state.mac.settings.rcon_port())).on_input(
                    |s| Message::MAC(MACMessage::Preferences(Preferences {
                        internal: Some(InternalPreferences {
                            friends_api_usage: None,
                            tf2_directory: None,
                            rcon_password: None,
                            steam_api_key: None,
                            masterbase_key: None,
                            masterbase_host: None,
                            rcon_port: s.parse::<u16>().ok(),
                            dumb_autokick: None,
                        }),
                        external: None
                    }))
                ).width(HALF_WIDTH),
            ].align_items(iced::Alignment::Center)
            .spacing(ROW_SPACING),

            // STEAM
            widget::Space::with_height(HEADING_SPACING),
            heading("Steam API"),

            // Steam API key
            widget::row![
                widget::row![
                    tooltip("Steam API key", "Your Steam Web API key is used to lookup player profiles and friend information from the Steam Web API."),
                    widget::horizontal_space(),
                    widget::button("Get yours here").on_press(Message::Open("https://steamcommunity.com/dev/apikey".to_string())),
                ].width(HALF_WIDTH),
                widget::text_input("Steam API key", state.mac.settings.steam_api_key()).on_input(
                    |s| Message::MAC(MACMessage::Preferences(Preferences {
                        internal: Some(InternalPreferences {
                            friends_api_usage: None,
                            tf2_directory: None,
                            rcon_password: None,
                            steam_api_key: Some(s),
                            masterbase_key: None,
                            masterbase_host: None,
                            rcon_port: None,
                            dumb_autokick: None,
                        }),
                        external: None
                    }))
                ).width(HALF_WIDTH),
            ].align_items(iced::Alignment::Center)
            .spacing(ROW_SPACING),

            // Friend lookups
            widget::row![
                widget::row![tooltip("Friend Lookups", "Which accounts will have their friend lists looked up via the Steam Web API. Friend lookups can only be requested on an individual account basis and may use up a larger number of API requests.")].width(HALF_WIDTH),
                widget::row![
                widget::PickList::new(FRIENDS_API_USAGE_OPTIONS, Some(state.mac.settings.friends_api_usage()), |v| {
                    Message::MAC(MACMessage::Preferences(Preferences {
                        internal: Some(InternalPreferences {
                            friends_api_usage: Some(v),
                            tf2_directory: None,
                            rcon_password: None,
                            steam_api_key: None,
                            masterbase_key: None,
                            masterbase_host: None,
                            rcon_port: None,
                            dumb_autokick: None,
                        }),
                        external: None
                    }))
                })].width(HALF_WIDTH).padding(5),
            ].align_items(iced::Alignment::Center).spacing(5),

            // MASTERBASE
            widget::Space::with_height(HEADING_SPACING),
            heading("MAC Integration"),

            // Enable MAC
            widget::row![
                tooltip(widget::checkbox("Enable MAC Integration", state.mac.settings.upload_demos()).on_toggle(Message::ToggleMACEnabled).width(HALF_WIDTH),
                widget::text("Enabled integration with Mega Anti-Cheat, making this useable in place of the official Mega Anti-Cheat client.")),
            ].align_items(iced::Alignment::Center).spacing(5),

            // Masterbase key
            widget::row![
                widget::row![
                    tooltip("Masterbase key", "Your personal key for authenticating with the Masterbase."),
                    widget::horizontal_space(),
                    widget::button("Get yours here").on_press(Message::Open(format!("{}://{}/provision", if state.mac.settings.use_masterbase_http() {"http"} else {"https"}, state.mac.settings.masterbase_host() ))),
                ].width(HALF_WIDTH),
                widget::text_input("Masterbase key", state.mac.settings.masterbase_key()).on_input(
                    |s| Message::MAC(MACMessage::Preferences(Preferences {
                        internal: Some(InternalPreferences {
                            friends_api_usage: None,
                            tf2_directory: None,
                            rcon_password: None,
                            steam_api_key: None,
                            masterbase_key: Some(s),
                            masterbase_host: None,
                            rcon_port: None,
                            dumb_autokick: None,
                        }),
                        external: None
                    }))
                ).width(HALF_WIDTH),
            ].align_items(iced::Alignment::Center)
            .spacing(ROW_SPACING),

            // Masterbase host
            widget::row![
                widget::row![
                    tooltip("Masterbase host", "The address to conteact the remote Masterbase at. You most likely will not need to change this."),
                ].width(HALF_WIDTH),
                widget::text_input("Masterbase host", state.mac.settings.masterbase_host()).on_input(
                    |s| Message::MAC(MACMessage::Preferences(Preferences {
                        internal: Some(InternalPreferences {
                            friends_api_usage: None,
                            tf2_directory: None,
                            rcon_password: None,
                            steam_api_key: None,
                            masterbase_key: None,
                            masterbase_host: Some(s),
                            rcon_port: None,
                            dumb_autokick: None,
                        }),
                        external: None
                    }))
                ).width(HALF_WIDTH),
            ].align_items(iced::Alignment::Center)
            .spacing(ROW_SPACING),

            // OTHER
            widget::Space::with_height(HEADING_SPACING),
            heading("Other"),

            // Autokick bots
            widget::row![
                tooltip(
                    widget::checkbox("Autokick bots", state.mac.settings.autokick_bots()).on_toggle(Message::SetKickBots),
                    widget::text("Attempt to automatically kick bots on your team. This does not account for cooldowns or ongoing votes, so use at your own discretion."),
                )
            ].align_items(iced::Alignment::Center).spacing(5),

            // External section? Probably not
        ]
        .width(Length::Fill)
        .spacing(5)
        .padding(15),
    ).into()
}
