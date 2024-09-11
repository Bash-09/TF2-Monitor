#![allow(clippy::cast_precision_loss)]
#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::cast_possible_wrap)]
#![allow(clippy::cast_sign_loss)]
#![allow(clippy::redundant_pub_crate)]

use std::{
    any::TypeId, cell::RefCell, collections::{HashMap, HashSet}, io::Cursor, path::PathBuf, time::Duration
};
use bytes::Bytes;
use demos::DemosMessage;
use graph::KDAChart;
use replay::{ReplayMessage, ReplayState};
use gui::{chat, icons::FONT_FILE, killfeed, records, SidePanel, View, PFP_FULL_SIZE, PFP_SMALL_SIZE};
use iced::{
    event::Event,
    futures::{FutureExt, SinkExt},
    widget::{
        self,
        scrollable::{snap_to, RelativeOffset},
    },
    Application,
};
use image::{io::Reader, EncodableLayout, ImageBuffer};
use reqwest::StatusCode;
use serde_json::Map;
use settings::{AppSettings, PanelSide, SETTINGS_IDENTIFIER};
use tokio::sync::broadcast::{Receiver, Sender};

use tf2_monitor_core::{
    console::{commands::{Command, CommandManager, DumbAutoKick}, ConsoleLog, ConsoleOutput, ConsoleParser, RawConsoleOutput}, demos::{analyser::AnalysedDemo, DemoBytes, DemoManager, DemoMessage, DemoWatcher}, event_loop::{self, define_events, EventLoop, MessageSource}, events::{Preferences, Refresh, UserUpdates}, masterbase, players::{new_players::{ExtractNewPlayers, NewPlayers}, records::{Records, Verdict}, Players}, server::Server, settings::{AppDetails, Settings}, steam::{self, api::{
        FriendLookupResult, LookupFriends, LookupProfiles, ProfileLookupBatchTick,
        ProfileLookupRequest, ProfileLookupResult,
    }}, steamid_ng::SteamID, MonitorState
};

pub mod gui;
pub mod settings;
pub mod replay;
pub mod demos;
pub mod graph;
mod tracing_setup;

/// Changing this will change where config files are stored,
/// so I'm just leaving it as-is for compatibility's sake
pub const APP: AppDetails<'static> = AppDetails {
    qualifier: "com.megascatterbomb",
    organization: "MAC",
    application: "MACClient",
};

pub const ALIAS_KEY: &str = "alias";
pub const NOTES_KEY: &str = "playerNote";

define_events!(
    MonitorState,
    MonitorMessage {
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
    MonitorHandler {
        CommandManager,

        ConsoleParser,

        ExtractNewPlayers,

        LookupProfiles,
        LookupFriends,

        DemoManager,
        DumbAutoKick,
    },
);

impl Clone for MonitorMessage {
    fn clone(&self) -> Self {
        tracing::error!("Shouldn't be cloning MACMessages!");
        Self::None
    }
}

pub struct Client {
    pub mac: MonitorState,
    pub mac_event_handler: EventLoop<MonitorState, MonitorMessage, MonitorHandler>,
}

type IcedElement<'a> = iced::Element<'a, Message, iced::Theme, iced::Renderer>;
type IcedContainer<'a> = iced::widget::Container<'a, Message, iced::Theme, iced::Renderer>;

pub struct App {
    mac: MonitorState,
    event_loop: EventLoop<MonitorState, MonitorMessage, MonitorHandler>,
    settings: AppSettings,

    // UI State
    selected_player: Option<SteamID>,

    snap_chat_to_bottom: bool,
    snap_kills_to_bottom: bool,

    // records
    records: records::State,

    // (High res, Low res)
    pfp_cache: HashMap<String, (iced::widget::image::Handle, iced::widget::image::Handle)>,
    pfp_in_progess: HashSet<String>,

    // Replay
    replay: ReplayState,

    // Demos
    demos: demos::State,

    // Change TF2 directory
    change_tf2_dir: Sender<PathBuf>,
    _tf2_dir_changed: RefCell<Option<Receiver<PathBuf>>>,
}

#[derive(Debug, Clone)]
pub enum Message {
    None,

    EventOccurred(Event),
    PfpLookupResponse(String, Result<Bytes, ()>),
    ProfileLookupRequest(SteamID),

    SetTheme(iced::Theme),
    SetView(View),
    SelectPlayer(SteamID),
    UnselectPlayer,
    SetReplay(PathBuf),
    /// Toggle whether a particular sidepanel is visible 
    ToggleSidePanel(&'static [SidePanel], SidePanel),
    SetPanelSide(PanelSide),

    CopyToClipboard(String),
    ChangeVerdict(SteamID, Verdict),
    ChangeNotes(SteamID, String),
    Open(String),
    MAC(MonitorMessage),
    ToggleMACEnabled(bool),
    BrowseTF2Dir,

    AddDemoDir,
    RemoveDemoDir(usize),

    /// Which page of records to display
    SetRecordPage(usize),
    ToggleVerdictFilter(Verdict),
    /// Records search bar
    SetRecordSearch(String),

    Demos(DemosMessage),

    ScrolledChat(RelativeOffset),
    ScrolledKills(RelativeOffset),

    SetKickBots(bool),

    Replay(ReplayMessage),
}

impl Application for App {
    type Executor = iced::executor::Default;
    type Message = Message;
    type Theme = iced::Theme;
    type Flags = (
        MonitorState,
        EventLoop<MonitorState, MonitorMessage, MonitorHandler>,
        AppSettings,
    );

    fn new((mut mac, event_loop, settings): Self::Flags) -> (Self, iced::Command<Self::Message>) {

        mac.settings.upload_demos = settings.enable_mac_integration;
        let mut commands = Vec::new();
        if settings.enable_mac_integration {
            commands.push(verify_masterbase_connection(&mac.settings));
        };

        let (tf2_dir_tx, tf2_dir_rx) = tokio::sync::broadcast::channel(1);
        let mut app = Self {
            mac,
            event_loop,
            settings,

            selected_player: None,

            snap_chat_to_bottom: true,
            snap_kills_to_bottom: true,

            records: records::State::new(),

            pfp_cache: HashMap::new(),
            pfp_in_progess: HashSet::new(),

            replay: ReplayState::new(),

            demos: demos::State::new(),

            change_tf2_dir: tf2_dir_tx,
            _tf2_dir_changed: RefCell::new(Some(tf2_dir_rx)),
        };

        app.update_displayed_records();

        commands.push(demos::State::refresh_demos(&app));

        (app, iced::Command::batch(commands))
    }

    fn title(&self) -> String {
        String::from("Bash's TF2 Monitor")
    }

    fn theme(&self) -> iced::Theme {
        self.settings.theme.clone()
    }

    fn subscription(&self) -> iced::Subscription<Self::Message> {
        let mut tf2_dir_changed_log = self.change_tf2_dir.subscribe();
        let mut tf2_dir_changed_con = self.change_tf2_dir.subscribe();

        #[allow(clippy::used_underscore_binding)]
        let _ = self._tf2_dir_changed.replace(None);
        
        let log_file_path = self.mac.settings.tf2_directory.clone().map(|path| path.join("tf/console.log"));
        let demo_path = self.mac.settings.tf2_directory.clone().map(|path| path.join("tf"));

        #[allow(clippy::used_underscore_binding)]
        let analysed_demo_rx = self.demos._demo_analysis_output.replace(None);

        iced::Subscription::batch([
            iced::event::listen().map(Message::EventOccurred),
            iced::time::every(Duration::from_secs(2))
                .map(|_| Message::MAC(MonitorMessage::Refresh(Refresh))),
            iced::time::every(Duration::from_millis(500))
                .map(|_| Message::MAC(MonitorMessage::ProfileLookupBatchTick(ProfileLookupBatchTick))),
            iced::subscription::channel(TypeId::of::<ConsoleLog>(), 100, |mut output| async move {
                let mut console_log = if let Some(path) = log_file_path {
                    ConsoleLog::new(path)
                } else {
                    ConsoleLog::new(tf2_dir_changed_log.recv().await.expect("Couldn't receive new TF2 dir"))
                }.await;

                loop {
                    tokio::select! {
                        Some(line) = console_log.recv.recv() => {
                            output
                                .send(Message::MAC(MonitorMessage::RawConsoleOutput(
                                    RawConsoleOutput(line),
                                )))
                                .await.ok();
                        },
                        Ok(new_tf2_dir) = tf2_dir_changed_log.recv() => {
                            console_log = ConsoleLog::new(new_tf2_dir).await;
                        }
                        else => {
                            panic!("Console watcher should have either received a new line or new TF2 dir :(");
                        }
                    };

                }
            }),
            iced::subscription::channel(
                TypeId::of::<DemoWatcher>(),
                100,
                |mut output| async move {
                    let mut demo_watcher = demo_path.and_then(|path| DemoWatcher::new(&path).map_err(|e| {
                        tracing::error!("Couldn't start demo watcher: {e}");
                    }).ok());

                    loop {
                        if let Some (m) = demo_watcher.as_mut().and_then(MessageSource::next_message) {
                            output.send(Message::MAC(m)).await.ok();
                        }

                        if let Ok(Ok(new_tf2_dir)) = tokio::time::timeout(Duration::from_millis(50), tf2_dir_changed_con.recv()).await {
                            demo_watcher = DemoWatcher::new(&new_tf2_dir).map_err(|e| {
                                tracing::error!("Couldn't start demo watcher: {e}");
                            }).ok();
                        }

                        tokio::time::sleep(Duration::from_millis(50)).await;
                    }
                    
                },
            ),
            iced::subscription::channel(
                TypeId::of::<AnalysedDemo>(), 
                50, 
                |mut output| async move {
                    let mut analysed_demo_rx = analysed_demo_rx.expect("Should have been a valid receiver.");
                    loop {
                        let demo = analysed_demo_rx.recv().await.expect("Couldn't receive any more analysed demos.");
                        tracing::debug!("Received analysed demo {:?}", demo.0);
                        output.send(Message::Demos(DemosMessage::DemoAnalysed(demo))).await.expect("Couldn't forward analysed demo.");
                    }
                }
            ),
        ])
    }

    #[allow(clippy::too_many_lines)]
    fn update(&mut self, message: Self::Message) -> iced::Command<Self::Message> {
        match message {
            Message::None => {}
            Message::EventOccurred(Event::Window(_, iced::window::Event::Moved { x, y })) => {
                self.settings.window_pos = Some((x, y));
            }
            Message::EventOccurred(Event::Window(_, iced::window::Event::Resized {
                width,
                height,
            })) => {
                self.settings.window_size = Some((width, height));
            }
            #[allow(clippy::match_same_arms)]
            Message::EventOccurred(_) => {}
            Message::SetView(v) => {
                self.settings.view = v;
                if matches!(self.settings.view, View::Records) {
                    self.update_displayed_records();
                } 
                if matches!(self.settings.view, View::Demos) {
                    self.update_demo_list();
                } 
                if let View::AnalysedDemo(id) = self.settings.view {
                    self.demos.chart = KDAChart::new(self, id, self.selected_player);
                }
            }
            Message::ChangeVerdict(steamid, verdict) => self.update_verdict(steamid, verdict),
            Message::ChangeNotes(steamid, notes) => self.update_notes(steamid, notes),
            Message::SelectPlayer(steamid) => {
                self.selected_player = Some(steamid);

                if let View::AnalysedDemo(demo) = self.settings.view {
                    self.demos.chart = KDAChart::new(self, demo, Some(steamid)); 
                }

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
            Message::SetRecordPage(p) => self.records.current_page = p,
            Message::ToggleVerdictFilter(v) => {
                if self.records.verdict_whitelist.contains(&v) {
                    self.records.verdict_whitelist.retain(|&vv| vv != v);
                } else {
                    self.records.verdict_whitelist.push(v);
                }

                self.update_displayed_records();

                let max_page = self.records.to_display.len() / self.records.num_per_page;
                self.records.current_page = self.records.current_page.min(max_page);
            }
            Message::SetRecordSearch(search) => {
                self.records.search = search;
                self.update_displayed_records();
                let max_page = self.records.to_display.len() / self.records.num_per_page;
                self.records.current_page = self.records.current_page.min(max_page);
            }
            Message::SetKickBots(kick) => self.mac.settings.autokick_bots = kick,
            Message::ScrolledChat(offset) => {
                self.snap_chat_to_bottom = (offset.y - 1.0).abs() <= f32::EPSILON;
            }
            Message::ScrolledKills(offset) => {
                self.snap_kills_to_bottom = (offset.y - 1.0).abs() <= f32::EPSILON;
            }
            Message::ProfileLookupRequest(s) => {
                return self.request_profile_lookup(vec![s]);
            }
            Message::ToggleMACEnabled(enabled) => {
                self.settings.enable_mac_integration = enabled;
                self.mac.settings.upload_demos = enabled;
                if enabled {
                    return verify_masterbase_connection(&self.mac.settings);
                }
            },
            Message::Replay(m) => {
                return self.replay.handle_message(m, &self.mac);
            },
            Message::BrowseTF2Dir => {
                let Some(new_tf2_dir) = rfd::FileDialog::new().pick_folder() else {
                    return iced::Command::none();
                };
                self.mac.settings.tf2_directory = Some(new_tf2_dir.clone());
                self.change_tf2_dir.send(new_tf2_dir).map_err(|e| tracing::error!("TF2 Directory could not be update for console and demo watchers: {e}")).ok();
            },
            Message::Demos(msg) => {
                return demos::State::handle_message(self, msg);
            },
            Message::SetReplay(path) => {
                self.settings.view = View::Replay;
                return self.replay.handle_message(ReplayMessage::SetDemoPath(path), &self.mac);
            }
            Message::SetTheme(theme) => {
                self.settings.theme = theme;
            },
            Message::ToggleSidePanel(available_panels, panel) => {
                if self.selected_player.is_some() || !self.settings.sidepanels.contains(&panel) {
                    for p in available_panels { self.settings.sidepanels.remove(p); }
                    self.settings.sidepanels.insert(panel);
                    return self.unselect_player();
                }

                for p in available_panels { self.settings.sidepanels.remove(p); }
            }
            Message::SetPanelSide(side) => self.settings.panel_side = side,
            Message::AddDemoDir => {
                let Some(new_demo_dir) = rfd::FileDialog::new().pick_folder() else {
                    return iced::Command::none();
                };
                self.settings.demo_directories.push(new_demo_dir);
                return snap_to(
                    widget::scrollable::Id::new(gui::settings::SCROLLABLE_ID),
                    RelativeOffset { x: 0.0, y: 1.0 },
                );
            },
            Message::RemoveDemoDir(idx) => {
                self.settings.demo_directories.remove(idx);
            },
        };

        iced::Command::none()
    }

    fn view(&self) -> iced::Element<'_, Self::Message, Self::Theme, iced::Renderer> {
        gui::main_window(self).into()
    }
}

impl App {
    fn save_settings(&mut self) {
        let settings = &mut self.mac.settings;
        let mut external_settings = settings.external.clone();
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
        let steamid = SteamID::try_from(self.records.search.as_str()).ok();

        self.records.to_display = self
            .mac
            .players
            .records
            .iter()
            .map(|(s, r)| (*s, r))
            .filter(|(_, r)| self.records.verdict_whitelist.contains(&r.verdict()))
            .filter(|(s, r)| {
                // Search bar
                if self.records.search.is_empty() {
                    return true;
                }

                // Previous names
                r.previous_names()
                    .iter()
                    .any(|n| n.contains(&self.records.search))

                    // Steamid
                    || steamid.is_some_and(|_| {
                        format!("{}", u64::from(*s)).contains(&self.records.search)
                    })

                    // Current name
                    || self
                        .mac
                        .players
                        .get_name(*s)
                        .is_some_and(|n| n.contains(&self.records.search))

                    // Alias
                    || r.custom_data().get(ALIAS_KEY).and_then(|v| v.as_str()).is_some_and(|s| s.contains(&self.records.search))

                    // Notes
                    || r.custom_data().get(NOTES_KEY).and_then(|v| v.as_str()).is_some_and(|s| s.contains(&self.records.search))
                    
            })
            .map(|(s, _)| s)
            .collect();

        self.records.to_display.sort_by_key(|s| {
            self.mac
                .players
                .records
                .get(s)
                .expect("Only existing records should be in this list")
                .modified()
        });

        // If exact steamid, put it at the top of the list (even if there isn't a record for it)
        if let Some(steamid) = steamid {
            #[allow(clippy::unreadable_literal)]
            if u64::from(steamid) >= 76561197960265728 {
                if let Some(i) = self.records.to_display.iter().position(|s| *s == steamid) {
                    self.records.to_display.remove(i);
                }

                self.records.to_display.push(steamid);
            }
        }
        
        self.records.to_display.reverse();
    }

    /// Updates the list of demos that is being displayed
    pub fn update_demo_list(&mut self) {
        self.demos.demos_to_display = self.settings.demo_filters.filter(self);
        self.demos.page = self.demos.page.min(self.demos.demos_to_display.len() / self.demos.demos_per_page);
    }

    fn handle_mac_message(&mut self, message: MonitorMessage) -> iced::Command<Message> {
        let mut commands = Vec::new();

        let mut messages = vec![message];
        while let Some(m) = messages.pop() {
            // Get profile pictures
            match &m {
                MonitorMessage::ProfileLookupResult(ProfileLookupResult(Ok(profiles))) => {
                    for (_, r) in profiles {
                        if let Ok(si) = r {
                            commands.push(self.request_pfp_lookup(&si.pfp_hash, &si.pfp_url));
                        }
                    }
                }
                MonitorMessage::NewPlayers(NewPlayers(players)) => {
                    for s in players {
                        commands.push(self.request_pfp_lookup_for_existing_player(*s));
                    }
                }
                MonitorMessage::ConsoleOutput(ConsoleOutput::Chat(_)) if self.snap_chat_to_bottom => {
                    commands.push(snap_to(
                        widget::scrollable::Id::new(chat::SCROLLABLE_ID),
                        RelativeOffset { x: 0.0, y: 1.0 },
                    ));
                }
                MonitorMessage::ConsoleOutput(ConsoleOutput::Kill(_)) if self.snap_kills_to_bottom => {
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
                            f.map(|m| m.unwrap_or(MonitorMessage::None)),
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
            MonitorMessage::ProfileLookupRequest(ProfileLookupRequest::Multiple(accounts)),
            &mut self.mac,
        ) {
            match a {
                event_loop::Action::Message(_) => {}
                event_loop::Action::Future(f) => {
                    commands.push(iced::Command::perform(
                        f.map(|m| m.unwrap_or(MonitorMessage::None)),
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

        if self.settings.sidepanels.contains(&SidePanel::ChatKills) {
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
        if let View::AnalysedDemo(_) = self.settings.view {
            self.settings.view = View::Demos;
        }
        self.save_settings();
        self.mac.players.records.save_ok();
        self.mac.players.save_steam_info_ok();
    }
}

fn main() {
    let _guard = tracing_setup::init_tracing();

    // Load Settings
    let mut settings = Settings::load_or_create(
        Settings::default_file_location(APP).unwrap_or_else(|e| {
            tracing::error!("Failed to find a suitable location to store settings ({e}). Settings will be written to {}", tf2_monitor_core::settings::CONFIG_FILE_NAME);
            tf2_monitor_core::settings::CONFIG_FILE_NAME.into()
        }
    )).expect("Failed to load settings. Please fix any issues mentioned and try again.");
    settings.save_ok();

    if let Err(e) = settings.infer_steam_user() {
        tracing::error!("Failed to infer steam user: {e}");
    }

    if let Err(e) = settings.infer_tf2_directory() {
        tracing::error!("Failed to locate TF2 directory: {e}");
    }

    // Playerlist
    let mut playerlist = Records::load_or_create(Records::default_file_location(APP).unwrap_or_else(|e| {
        tracing::error!("Failed to find a suitable location to store player records ({e}). Records will be written to {}", tf2_monitor_core::players::records::RECORDS_FILE_NAME);
        tf2_monitor_core::players::records::RECORDS_FILE_NAME.into()
    })).expect("Failed to load player records. Please fix any issues mentioned and try again.");
    playerlist.save_ok();

    let mut players = Players::new(
        playerlist,
        settings.steam_user,
        Players::default_steam_cache_path(APP).ok(),
    );

    // Local friends
    if let Some(user) = settings.steam_user {
        match steam::find_steam_user_friends(user) {
            Ok(friends) => players.update_friends_list(user, friends),
            Err(e) => tracing::error!("Failed to check local player's friends: {e}"),
        }
    }

    let core = MonitorState {
        server: Server::new(),
        settings,
        players,
    };

    let app_settings: AppSettings = core
        .settings
        .external
        .get(SETTINGS_IDENTIFIER)
        .and_then(|v| serde_json::from_value(v.clone()).map_err(|e| {
            tracing::error!("Failed to deserialize app settings: {e}");
        }).ok())
        .unwrap_or_default();

    let event_loop = EventLoop::new()
        .add_handler(CommandManager::new())
        .add_handler(ConsoleParser::default())
        .add_handler(ExtractNewPlayers)
        .add_handler(LookupProfiles::new())
        .add_handler(DemoManager::new())
        .add_handler(LookupFriends::new());

    let mut iced_settings = iced::Settings::with_flags((core, event_loop, app_settings.clone()));
    iced_settings.window.min_size = Some(iced::Size::new(800.0, 450.0));
    iced_settings.fonts.push(FONT_FILE.into());
    // iced_settings.fonts.push(&FONT_FILE);
    if let Some((x, y)) = app_settings.window_pos {
        iced_settings.window.position = iced::window::Position::Specific(iced::Point::new(x as f32, y as f32));
    }
    if let Some((width, height)) = app_settings.window_size {
        iced_settings.window.size = iced::Size::new(width as f32, height as f32);
    }

    App::run(iced_settings).expect("Failed to run app.");
}

impl std::fmt::Debug for MonitorMessage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "MACMessage")
    }
}

fn verify_masterbase_connection(settings: &Settings) -> iced::Command<Message> {
    let host = settings.masterbase_host.to_string();
    let key = settings.masterbase_key.to_string();
    let http = settings.masterbase_http;
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
}
