use std::{
    any::TypeId,
    collections::{HashMap, HashSet},
    io::Cursor,
    time::Duration,
};

use bytes::Bytes;
use clap::Parser;
use client_backend::{
    args::Args,
    console::ConsoleLog,
    demo::{DemoWatcher, PrintVotes},
    event_loop::{self, define_events, EventLoop, MessageSource},
    masterbase,
    player::Players,
    player_records::{PlayerRecords, Verdict},
    server::Server,
    settings::Settings,
    state::MACState,
    steamid_ng::SteamID,
};
use gui::{chat, killfeed, View, PFP_FULL_SIZE, PFP_SMALL_SIZE};
use iced::{
    event::Event,
    futures::{FutureExt, SinkExt},
    widget::{
        self,
        scrollable::{snap_to, RelativeOffset},
        Container,
    },
    Application,
};
use image::{io::Reader, EncodableLayout, ImageBuffer};
use reqwest::StatusCode;
use serde_json::Map;
use settings::{AppSettings, SETTINGS_IDENTIFIER};

pub const ALIAS_KEY: &str = "alias";
pub const NOTES_KEY: &str = "playerNote";

pub mod gui;
pub mod settings;
mod tracing_setup;

use client_backend::{
    command_manager::{Command, CommandManager, DumbAutoKick},
    console::{ConsoleOutput, ConsoleParser, RawConsoleOutput},
    demo::{DemoBytes, DemoManager, DemoMessage},
    events::{Preferences, Refresh, UserUpdates},
    new_players::{ExtractNewPlayers, NewPlayers},
    steam_api::{
        FriendLookupResult, LookupFriends, LookupProfiles, ProfileLookupBatchTick,
        ProfileLookupRequest, ProfileLookupResult,
    },
};

define_events!(
    MACState,
    MACMessage {
        Refresh,

        Command,

        RawConsoleOutput,
        ConsoleOutput,

        NewPlayers,

        ProfileLookupRequest,
        ProfileLookupBatchTick,
        ProfileLookupResult,
        FriendLookupResult,

        Preferences,
        UserUpdates,

        DemoBytes,
        DemoMessage,
    },
    MACHandler {
        CommandManager,

        ConsoleParser,

        ExtractNewPlayers,

        LookupProfiles,
        LookupFriends,

        DemoManager,
        DumbAutoKick,
        PrintVotes,
    },
);

impl Clone for MACMessage {
    fn clone(&self) -> Self {
        tracing::error!("Shouldn't be cloning MACMessages!");
        Self::None
    }
}

pub struct Client {
    pub mac: MACState,
    pub mac_event_handler: EventLoop<MACState, MACMessage, MACHandler>,
}

type IcedContainer<'a> = Container<'a, Message, iced::Renderer<iced::Theme>>;

pub struct App {
    mac: MACState,
    event_loop: EventLoop<MACState, MACMessage, MACHandler>,
    settings: AppSettings,

    // UI State
    view: View,
    selected_player: Option<SteamID>,

    snap_chat_to_bottom: bool,
    snap_kills_to_bottom: bool,

    // records
    records_to_display: Vec<SteamID>,
    records_per_page: usize,
    record_page: usize,
    record_verdict_whitelist: Vec<Verdict>,
    record_search: String,

    // (High res, Low res)
    pfp_cache: HashMap<String, (iced::widget::image::Handle, iced::widget::image::Handle)>,
    pfp_in_progess: HashSet<String>,
}

#[derive(Debug, Clone)]
pub enum Message {
    None,

    EventOccurred(Event),
    PfpLookupResponse(String, Result<Bytes, ()>),
    ProfileLookupRequest(SteamID),

    SetView(View),
    SelectPlayer(SteamID),
    UnselectPlayer,
    /// Toggle whether the chat and killfeed section on the right should be shown
    ToggleChatKillfeed,

    CopyToClipboard(String),
    ChangeVerdict(SteamID, Verdict),
    ChangeNotes(SteamID, String),
    Open(String),
    MAC(MACMessage),

    /// Which page of records to display
    SetRecordPage(usize),
    ToggleVerdictFilter(Verdict),
    /// Records search bar
    SetRecordSearch(String),

    ScrolledChat(RelativeOffset),
    ScrolledKills(RelativeOffset),

    SetKickBots(bool),
}

impl Application for App {
    type Executor = iced::executor::Default;
    type Message = Message;
    type Theme = iced::Theme;
    type Flags = (
        MACState,
        EventLoop<MACState, MACMessage, MACHandler>,
        AppSettings,
    );

    fn new((mac, event_loop, settings): Self::Flags) -> (Self, iced::Command<Self::Message>) {
        let command = if mac.settings.upload_demos {
            let host = mac.settings.masterbase_host().to_string();
            let key = mac.settings.masterbase_key().to_string();
            let http = mac.settings.use_masterbase_http();
            iced::Command::perform(
                async move {
                    match masterbase::force_close_session(&host, &key, http).await {
                        // Successfully closed existing session
                        Ok(r) if r.status().is_success() => tracing::warn!(
                            "User was previously in a Masterbase session that has now been closed."
                        ),
                        // Server error
                        Ok(r) if r.status().is_server_error() => tracing::error!(
                            "Server error when trying to close previous Masterbase sessions: Status code {}",
                            r.status()
                        ),
                        // Not authorized, invalid key
                        Ok(r) if r.status() == StatusCode::UNAUTHORIZED => {
                            tracing::warn!("Your Masterbase key is not valid. Please provision a new one at https://megaanticheat.com/provision");
                        }
                        // Forbidden, no session was open
                        Ok(r) if r.status() == StatusCode::FORBIDDEN => {
                            tracing::info!("Successfully authenticated with the Masterbase.");
                        }
                        // Remaining responses will be client failures
                        Ok(r) => tracing::info!("Client error when trying to contact masterbase: Status code {}", r.status()),
                        Err(e) => tracing::error!("Couldn't reach Masterbase: {e}"),
                    }
                },
                |()| Message::None,
            )
        } else {
            iced::Command::none()
        };

        (
            Self {
                mac,
                event_loop,
                settings,

                view: View::Server,
                selected_player: None,

                snap_chat_to_bottom: true,
                snap_kills_to_bottom: true,

                records_to_display: Vec::new(),
                records_per_page: 50,
                record_page: 0,
                record_verdict_whitelist: vec![
                    Verdict::Trusted,
                    Verdict::Player,
                    Verdict::Suspicious,
                    Verdict::Cheater,
                    Verdict::Bot,
                ],
                record_search: String::new(),

                pfp_cache: HashMap::new(),
                pfp_in_progess: HashSet::new(),
            },
            command,
        )
    }

    fn title(&self) -> String {
        String::from("Bash Client")
    }

    fn theme(&self) -> iced::Theme {
        iced::Theme::Dark
    }

    fn subscription(&self) -> iced::Subscription<Self::Message> {
        let log_file_path = self.mac.settings.tf2_directory().join("tf/console.log");
        let demo_path = self.mac.settings.tf2_directory().join("tf");

        iced::Subscription::batch([
            iced::subscription::events().map(Message::EventOccurred),
            iced::time::every(Duration::from_secs(2))
                .map(|_| Message::MAC(MACMessage::Refresh(Refresh))),
            iced::time::every(Duration::from_millis(500))
                .map(|_| Message::MAC(MACMessage::ProfileLookupBatchTick(ProfileLookupBatchTick))),
            iced::subscription::channel(TypeId::of::<ConsoleLog>(), 100, |mut output| async move {
                let mut console_log = ConsoleLog::new(log_file_path).await;

                loop {
                    let line = console_log
                        .recv
                        .recv()
                        .await
                        .expect("No more messages coming.");
                    let _ = output
                        .send(Message::MAC(MACMessage::RawConsoleOutput(
                            RawConsoleOutput(line),
                        )))
                        .await;
                }
            }),
            iced::subscription::channel(
                TypeId::of::<DemoWatcher>(),
                100,
                |mut output| async move {
                    match DemoWatcher::new(&demo_path) {
                        Ok(mut watcher) => loop {
                            if let Some(m) = watcher.next_message() {
                                let _ = output.send(Message::MAC(m)).await;
                            }

                            tokio::time::sleep(Duration::from_millis(50)).await;
                        },
                        Err(e) => tracing::error!("Could not start demo watcher: {e:?}"),
                    }

                    loop {
                        tokio::time::sleep(Duration::from_millis(50)).await;
                    }
                },
            ),
        ])
    }

    #[allow(clippy::too_many_lines)]
    fn update(&mut self, message: Self::Message) -> iced::Command<Self::Message> {
        match message {
            Message::None => {}
            Message::EventOccurred(Event::Window(iced::window::Event::Moved { x, y })) => {
                self.settings.window_pos = Some((x, y));
            }
            Message::EventOccurred(Event::Window(iced::window::Event::Resized {
                width,
                height,
            })) => {
                self.settings.window_size = Some((width, height));
            }
            #[allow(clippy::match_same_arms)]
            Message::EventOccurred(_) => {}
            Message::SetView(v) => {
                self.view = v;
                if matches!(self.view, View::Records) {
                    self.update_displayed_records();
                }
            }
            Message::ChangeVerdict(steamid, verdict) => self.update_verdict(steamid, verdict),
            Message::ChangeNotes(steamid, notes) => self.update_notes(steamid, notes),
            Message::SelectPlayer(steamid) => {
                self.selected_player = Some(steamid);

                // Fetch their pfp if we don't have it currently but have the steam info
                if self.mac.players.steam_info.contains_key(&steamid) {
                    return self.request_pfp_lookup_for_existing_player(steamid);
                }

                // Request steam lookup of player if we don't have it currently,
                return self.request_profile_lookup(vec![steamid]);
            }
            Message::UnselectPlayer => {
                return self.unselect_player();
            }
            Message::PfpLookupResponse(pfp_hash, response) => {
                if let Ok(bytes) = response {
                    self.insert_new_pfp(pfp_hash, &bytes);
                }
            }
            Message::CopyToClipboard(contents) => return iced::clipboard::write(contents),
            Message::Open(to_open) => {
                if let Err(e) = open::that(&*to_open) {
                    tracing::error!("Failed to open {}: {:?}", to_open, e);
                }
            }
            Message::MAC(m) => {
                return self.handle_mac_message(m);
            }
            Message::SetRecordPage(p) => self.record_page = p,
            Message::ToggleVerdictFilter(v) => {
                if self.record_verdict_whitelist.contains(&v) {
                    self.record_verdict_whitelist.retain(|&vv| vv != v);
                } else {
                    self.record_verdict_whitelist.push(v);
                }

                self.update_displayed_records();

                let max_page = self.records_to_display.len() / self.records_per_page;
                self.record_page = self.record_page.min(max_page);
            }
            Message::SetRecordSearch(search) => {
                self.record_search = search;
                self.update_displayed_records();
                let max_page = self.records_to_display.len() / self.records_per_page;
                self.record_page = self.record_page.min(max_page);
            }
            Message::SetKickBots(kick) => self.mac.settings.set_autokick_bots(kick),
            Message::ScrolledChat(offset) => {
                self.snap_chat_to_bottom = (offset.y - 1.0).abs() <= f32::EPSILON;
            }
            Message::ScrolledKills(offset) => {
                self.snap_kills_to_bottom = (offset.y - 1.0).abs() <= f32::EPSILON;
            }
            Message::ToggleChatKillfeed => {
                if self.selected_player.is_some() {
                    self.settings.show_chat_and_killfeed = true;
                    return self.unselect_player();
                }

                self.settings.show_chat_and_killfeed = !self.settings.show_chat_and_killfeed;
            }
            Message::ProfileLookupRequest(s) => {
                return self.request_profile_lookup(vec![s]);
            }
        };

        iced::Command::none()
    }

    fn view(&self) -> iced::Element<'_, Self::Message, iced::Renderer<Self::Theme>> {
        gui::main_window(self).into()
    }
}

impl App {
    fn save_settings(&mut self) {
        let settings = &mut self.mac.settings;
        let mut external_settings = settings.external_preferences().clone();
        if !external_settings.is_object() {
            external_settings = serde_json::Value::Object(serde_json::Map::new());
        }
        external_settings[SETTINGS_IDENTIFIER] =
            serde_json::to_value(self.settings.clone()).expect("Epic serialization fail.");
        settings.update_external_preferences(external_settings);
        settings.save_ok();
    }

    fn update_verdict(&mut self, steamid: SteamID, verdict: Verdict) {
        let record = self.mac.players.records.entry(steamid).or_default();
        record.set_verdict(verdict);

        self.mac.players.records.prune();
        self.mac.players.records.save_ok();
    }

    fn update_notes(&mut self, steamid: SteamID, notes: String) {
        let record = self.mac.players.records.entry(steamid).or_default();

        let mut notes_value = Map::new();
        notes_value.insert(NOTES_KEY.to_string(), serde_json::Value::String(notes));
        record.set_custom_data(serde_json::Value::Object(notes_value));

        self.mac.players.records.prune();
        self.mac.players.records.save_ok();
    }

    fn update_displayed_records(&mut self) {
        self.records_to_display = self
            .mac
            .players
            .records
            .iter()
            .map(|(s, r)| (*s, r))
            .filter(|(_, r)| self.record_verdict_whitelist.contains(&r.verdict()))
            .filter(|(s, r)| {
                // Search bar
                if self.record_search.is_empty() {
                    return true;
                }

                // Previous names
                r.previous_names()
                    .iter()
                    .any(|n| n.contains(&self.record_search))

                    // Steamid
                    || self.record_search.parse::<u64>().is_ok_and(|_| {
                        format!("{}", u64::from(*s)).contains(&self.record_search)
                    })

                    // Current name
                    || self
                        .mac
                        .players
                        .get_name(*s)
                        .is_some_and(|n| n.contains(&self.record_search))

                    // Alias
                    || r.custom_data().get(ALIAS_KEY).and_then(|v| v.as_str()).is_some_and(|s| s.contains(&self.record_search))

                    // Notes
                    || r.custom_data().get(NOTES_KEY).and_then(|v| v.as_str()).is_some_and(|s| s.contains(&self.record_search))
                    
            })
            .map(|(s, _)| s)
            .collect();

        self.records_to_display.sort_by_key(|s| {
            self.mac
                .players
                .records
                .get(s)
                .expect("Only existing records should be in this list")
                .modified()
        });
        self.records_to_display.reverse();
    }

    fn handle_mac_message(&mut self, message: MACMessage) -> iced::Command<Message> {
        let mut commands = Vec::new();

        let mut messages = vec![message];
        while let Some(m) = messages.pop() {
            // Get profile pictures
            match &m {
                MACMessage::ProfileLookupResult(ProfileLookupResult(Ok(profiles))) => {
                    for (_, r) in profiles {
                        if let Ok(si) = r {
                            commands.push(self.request_pfp_lookup(&si.pfp_hash, &si.pfp_url));
                        }
                    }
                }
                MACMessage::NewPlayers(NewPlayers(players)) => {
                    for s in players {
                        commands.push(self.request_pfp_lookup_for_existing_player(*s));
                    }
                }
                MACMessage::ConsoleOutput(ConsoleOutput::Chat(_)) if self.snap_chat_to_bottom => {
                    commands.push(snap_to(
                        widget::scrollable::Id::new(chat::SCROLLABLE_ID),
                        RelativeOffset { x: 0.0, y: 1.0 },
                    ));
                }
                MACMessage::ConsoleOutput(ConsoleOutput::Kill(_)) if self.snap_kills_to_bottom => {
                    commands.push(snap_to(
                        widget::scrollable::Id::new(killfeed::SCROLLABLE_ID),
                        RelativeOffset { x: 0.0, y: 1.0 },
                    ));
                }
                _ => {}
            }

            // Handle MAC messages in MAC event loop
            for a in self.event_loop.handle_message(m, &mut self.mac) {
                match a {
                    event_loop::Action::Message(m) => messages.push(m),
                    event_loop::Action::Future(f) => {
                        commands.push(iced::Command::perform(
                            f.map(|m| m.unwrap_or(MACMessage::None)),
                            Message::MAC,
                        ));
                    }
                }
            }
        }

        iced::Command::batch(commands)
    }

    fn insert_new_pfp(&mut self, pfp_hash: String, bytes: &[u8]) {
        fn default_image() -> image::DynamicImage {
            image::DynamicImage::ImageRgb8(ImageBuffer::new(
                u32::from(PFP_FULL_SIZE),
                u32::from(PFP_FULL_SIZE),
            ))
        }

        let full_image = Reader::new(Cursor::new(bytes))
            .with_guessed_format()
            .ok()
            .and_then(|r| r.decode().ok())
            .unwrap_or_else(default_image)
            .resize(
                u32::from(PFP_FULL_SIZE),
                u32::from(PFP_FULL_SIZE),
                image::imageops::FilterType::Triangle,
            );

        let smol_image = full_image.resize(
            u32::from(PFP_SMALL_SIZE),
            u32::from(PFP_SMALL_SIZE),
            image::imageops::FilterType::Triangle,
        );

        let full_handle = iced::widget::image::Handle::from_pixels(
            u32::from(PFP_FULL_SIZE),
            u32::from(PFP_FULL_SIZE),
            Bytes::copy_from_slice(full_image.into_rgba8().as_bytes()),
        );
        let smol_handle = iced::widget::image::Handle::from_pixels(
            u32::from(PFP_SMALL_SIZE),
            u32::from(PFP_SMALL_SIZE),
            Bytes::copy_from_slice(smol_image.into_rgba8().as_bytes()),
        );

        self.pfp_in_progess.remove(&pfp_hash);
        self.pfp_cache.insert(pfp_hash, (full_handle, smol_handle));
    }

    fn request_profile_lookup(&mut self, accounts: Vec<SteamID>) -> iced::Command<Message> {
        let mut commands = Vec::new();
        for a in self.event_loop.handle_message(
            MACMessage::ProfileLookupRequest(ProfileLookupRequest::Multiple(accounts)),
            &mut self.mac,
        ) {
            match a {
                event_loop::Action::Message(_) => {}
                event_loop::Action::Future(f) => {
                    commands.push(iced::Command::perform(
                        f.map(|m| m.unwrap_or(MACMessage::None)),
                        Message::MAC,
                    ));
                }
            }
        }

        iced::Command::batch(commands)
    }

    fn request_pfp_lookup(&mut self, pfp_hash: &str, pfp_url: &str) -> iced::Command<Message> {
        if self.pfp_cache.contains_key(pfp_hash) || self.pfp_in_progess.contains(pfp_hash) {
            return iced::Command::none();
        }

        self.pfp_in_progess.insert(pfp_hash.to_string());
        let pfp_hash = pfp_hash.to_string();
        let pfp_url = pfp_url.to_string();
        iced::Command::perform(
            async move {
                match reqwest::get(&pfp_url).await {
                    Ok(resp) => (pfp_hash, resp.bytes().await.map_err(|_| ())),
                    Err(_) => (pfp_hash, Err(())),
                }
            },
            |(pfp_hash, resp)| Message::PfpLookupResponse(pfp_hash, resp),
        )
    }

    fn request_pfp_lookup_for_existing_player(
        &mut self,
        player: SteamID,
    ) -> iced::Command<Message> {
        let Some(si) = self.mac.players.steam_info.get(&player) else {
            return iced::Command::none();
        };

        let pfp_hash = &si.pfp_hash;
        let pfp_url = &si.pfp_url;

        if self.pfp_cache.contains_key(pfp_hash) || self.pfp_in_progess.contains(pfp_hash) {
            return iced::Command::none();
        }

        self.pfp_in_progess.insert(pfp_hash.to_string());
        let pfp_hash = pfp_hash.to_string();
        let pfp_url = pfp_url.to_string();
        iced::Command::perform(
            async move {
                match reqwest::get(&pfp_url).await {
                    Ok(resp) => (pfp_hash, resp.bytes().await.map_err(|_| ())),
                    Err(_) => (pfp_hash, Err(())),
                }
            },
            |(pfp_hash, resp)| Message::PfpLookupResponse(pfp_hash, resp),
        )
    }

    fn unselect_player(&mut self) -> iced::Command<Message> {
        self.selected_player = None;

        if self.settings.show_chat_and_killfeed {
            return iced::Command::batch([
                snap_to(widget::scrollable::Id::new(chat::SCROLLABLE_ID), RelativeOffset { x: 0.0, y: 1.0 }),
                snap_to(widget::scrollable::Id::new(killfeed::SCROLLABLE_ID), RelativeOffset { x: 0.0, y: 1.0 }),
            ]);
        }

        iced::Command::none()
    }
}

impl Drop for App {
    fn drop(&mut self) {
        self.save_settings();
        self.mac.players.records.save_ok();
        self.mac.players.save_steam_info_ok();
    }
}

fn main() {
    let _guard = tracing_setup::init_tracing();

    // Arg handling
    let args = Args::parse();

    // Load Settings
    let settings = Settings::load_or_create(&args);
    settings.save_ok();

    // Playerlist
    let mut playerlist = PlayerRecords::load_or_create(&args);
    playerlist.save_ok();

    let players = Players::new(playerlist, settings.steam_user());

    let mac = MACState {
        server: Server::new(),
        settings,
        players,
    };

    let app_settings: AppSettings = mac
        .settings
        .external_preferences()
        .get(SETTINGS_IDENTIFIER)
        .and_then(|v| serde_json::from_value(v.clone()).ok())
        .unwrap_or_default();

    let event_loop = EventLoop::new()
        .add_handler(CommandManager::new())
        .add_handler(ConsoleParser::default())
        .add_handler(ExtractNewPlayers)
        .add_handler(LookupProfiles::new())
        .add_handler(DemoManager::new())
        .add_handler(PrintVotes::new())
        .add_handler(LookupFriends::new());

    let mut iced_settings = iced::Settings::with_flags((mac, event_loop, app_settings.clone()));
    iced_settings.window.min_size = Some((600, 400));
    if let Some(pos) = app_settings.window_pos {
        iced_settings.window.position = iced::window::Position::Specific(pos.0, pos.1);
    }
    if let Some(size) = app_settings.window_size {
        iced_settings.window.size = size;
    }

    App::run(iced_settings).expect("Failed to run app.");
}

impl std::fmt::Debug for MACMessage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "MACMessage")
    }
}
