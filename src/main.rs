use std::{
    any::TypeId,
    collections::{HashMap, HashSet},
    time::Duration,
};

use bytes::Bytes;
use clap::Parser;
use client_backend::{
    args::Args,
    console::ConsoleLog,
    demo::DemoWatcher,
    event_loop::{self, define_events, EventLoop, MessageSource},
    player::Players,
    player_records::{PlayerRecords, Verdict},
    server::Server,
    settings::Settings,
    state::MACState,
    steamid_ng::SteamID,
};
use gui::{records::get_filtered_records, View};
use iced::{
    event::Event,
    futures::{FutureExt, SinkExt},
    widget::Container,
    Application,
};
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
        ProfileLookupResult,
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

    // records
    records_per_page: usize,
    record_page: usize,
    record_verdict_whitelist: Vec<Verdict>,
    record_search: String,

    pfp_cache: HashMap<String, iced::widget::image::Handle>,
    pfp_in_progess: HashSet<String>,
}

#[derive(Debug, Clone)]
pub enum Message {
    EventOccurred(Event),
    PfpLookupResponse(String, Result<Bytes, ()>),

    SetView(View),
    SelectPlayer(SteamID),
    UnselectPlayer,
    CopyToClipboard(String),
    ChangeVerdict(SteamID, Verdict),
    ChangeNotes(SteamID, String),
    Open(String),
    MAC(MACMessage),

    SetRecordPage(usize),
    ToggleVerdictFilter(Verdict),
    SetRecordSearch(String),

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

    fn new(flags: Self::Flags) -> (Self, iced::Command<Self::Message>) {
        (
            Self {
                mac: flags.0,
                event_loop: flags.1,
                settings: flags.2,

                view: View::Server,
                selected_player: None,

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
            iced::Command::none(),
        )
    }

    fn title(&self) -> String {
        String::from("MAC Client")
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

    fn update(&mut self, message: Self::Message) -> iced::Command<Self::Message> {
        match message {
            Message::EventOccurred(Event::Window(iced::window::Event::Moved { x, y })) => {
                self.settings.window_pos = Some((x, y));
            }
            Message::EventOccurred(Event::Window(iced::window::Event::Resized {
                width,
                height,
            })) => {
                self.settings.window_size = Some((width, height));
            }
            Message::EventOccurred(_) => {}
            Message::SetView(v) => self.view = v,
            Message::ChangeVerdict(steamid, verdict) => self.update_verdict(steamid, verdict),
            Message::ChangeNotes(steamid, notes) => self.update_notes(steamid, notes),
            Message::SelectPlayer(steamid) => {
                self.selected_player = Some(steamid);

                let mut commands = Vec::new();

                // Fetch their profile if we don't have it currently but have the steam info
                if let Some(si) = self.mac.players.steam_info.get(&steamid) {
                    // Request pfps
                    commands.push(self.request_pfp_lookup(si.pfp_hash.clone(), si.pfp_url.clone()));
                } else {
                    // Request steam lookup of player if we don't have it currently,
                    for a in self.event_loop.handle_message(
                        MACMessage::NewPlayers(NewPlayers(vec![steamid])),
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
                }

                return iced::Command::batch(commands);
            }
            Message::UnselectPlayer => self.selected_player = None,
            Message::PfpLookupResponse(pfp_hash, response) => {
                if let Ok(bytes) = response {
                    self.insert_new_pfp(pfp_hash, bytes);
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

                let max_page = get_filtered_records(self).count() / self.records_per_page;
                self.record_page = self.record_page.min(max_page);
            }
            Message::SetRecordSearch(search) => {
                self.record_search = search;
                let max_page = get_filtered_records(self).count() / self.records_per_page;
                self.record_page = self.record_page.min(max_page);
            }
            Message::SetKickBots(kick) => self.mac.settings.set_autokick_bots(kick),
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

    fn handle_mac_message(&mut self, message: MACMessage) -> iced::Command<Message> {
        let mut commands = Vec::new();

        let mut messages = vec![message];
        while let Some(m) = messages.pop() {
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

    fn insert_new_pfp(&mut self, pfp_hash: String, bytes: Bytes) {
        let handle = iced::widget::image::Handle::from_memory(bytes);
        self.pfp_in_progess.remove(&pfp_hash);
        self.pfp_cache.insert(pfp_hash, handle);
    }

    fn request_pfp_lookup(&mut self, pfp_hash: String, pfp_url: String) -> iced::Command<Message> {
        if self.pfp_cache.contains_key(&pfp_hash) || self.pfp_in_progess.contains(&pfp_hash) {
            return iced::Command::none();
        }

        self.pfp_in_progess.insert(pfp_hash.clone());
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
    let playerlist = PlayerRecords::load_or_create(&args);
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
        .add_handler(LookupFriends::new());

    let mut iced_settings = iced::Settings::with_flags((mac, event_loop, app_settings.clone()));
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
