use std::{any::TypeId, collections::HashMap, sync::Arc, time::Duration};

use bytes::Bytes;
use clap::Parser;
use client_backend::{
    args::Args,
    console::ConsoleLog,
    event_loop::{
        self, define_handlers, define_messages, EventLoop, Handled, HandlerStruct, StateUpdater,
    },
    player::Players,
    player_records::{PlayerRecords, Verdict},
    server::Server,
    settings::Settings,
    state::MACState,
    steamid_ng::SteamID,
};
use gui::View;
use iced::{
    event::Event,
    futures::{FutureExt, SinkExt},
    widget::Container,
    Application,
};
use settings::{AppSettings, SETTINGS_IDENTIFIER};

pub mod gui;
pub mod settings;
pub mod style;
mod tracing_setup;

use client_backend::{
    command_manager::{Command, CommandManager},
    console::{ConsoleOutput, ConsoleParser, RawConsoleOutput},
    events::{Preferences, Refresh, UserUpdates},
    new_players::{ExtractNewPlayers, NewPlayers},
    steam_api::{
        FriendLookupResult, LookupFriends, LookupProfiles, ProfileLookupBatchTick,
        ProfileLookupResult,
    },
};

define_messages!(MACMessage<MACState>:
    Refresh,

    Command,

    RawConsoleOutput,
    ConsoleOutput,

    NewPlayers,

    ProfileLookupBatchTick,
    ProfileLookupResult,
    FriendLookupResult,

    Preferences,
    UserUpdates
);

impl Clone for MACMessage {
    fn clone(&self) -> Self {
        tracing::error!("Shouldn't be cloning MACMessages!");
        Self::None
    }
}

define_handlers!(MACHandler<MACState, MACMessage>:
    CommandManager,

    ConsoleParser,

    ExtractNewPlayers,

    LookupProfiles,
    LookupFriends
);

pub struct Client {
    pub mac: MACState,
    pub mac_event_handler: EventLoop<MACState, MACMessage, MACHandler>,
}

type IcedContainer<'a> = Container<'a, Message, iced::Renderer<iced::Theme>>;

pub struct App {
    client: Client,
    settings: AppSettings,

    // UI State
    view: View,
    selected_player: Option<SteamID>,

    pfp_cache: HashMap<Arc<str>, iced::widget::image::Handle>,
}

#[derive(Debug, Clone)]
pub enum Message {
    EventOccurred(Event),
    PfpLookupResponse(Arc<str>, Result<Bytes, ()>),

    SetView(View),
    SelectPlayer(SteamID),
    UnselectPlayer,
    CopyToClipboard(String),
    ChangeVerdict(SteamID, Verdict),
    Open(Arc<str>),
    MAC(MACMessage),
}

impl Application for App {
    type Executor = iced::executor::Default;
    type Message = Message;
    type Theme = iced::Theme;
    type Flags = (Client, AppSettings);

    fn new(flags: Self::Flags) -> (Self, iced::Command<Self::Message>) {
        (
            Self {
                client: flags.0,
                settings: flags.1,

                view: View::Server,
                selected_player: None,

                pfp_cache: HashMap::new(),
            },
            iced::Command::none(),
        )
    }

    fn title(&self) -> String { String::from("MAC Client") }

    fn theme(&self) -> iced::Theme { iced::Theme::Dark }

    fn subscription(&self) -> iced::Subscription<Self::Message> {
        let log_file_path = self
            .client
            .mac
            .settings
            .get_tf2_directory()
            .to_path_buf()
            .join("tf/console.log");

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
            Message::SelectPlayer(steamid) => self.selected_player = Some(steamid),
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
                let mut commands = Vec::new();

                let mut messages = vec![m];
                while let Some(m) = messages.pop() {
                    // Request pfps
                    if let MACMessage::ProfileLookupResult(ProfileLookupResult(Ok(new_info))) = &m {
                        for (_, result) in new_info {
                            if let Ok(si) = result {
                                commands.push(
                                    self.request_pfp_lookup(
                                        si.pfp_hash.clone(),
                                        si.pfp_url.clone(),
                                    ),
                                );
                            }
                        }
                    }

                    for a in self
                        .client
                        .mac_event_handler
                        .handle_message(m, &mut self.client.mac)
                    {
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

                return iced::Command::batch(commands);
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
        let settings = &mut self.client.mac.settings;
        let mut external_settings = settings.get_external_preferences().clone();
        if !external_settings.is_object() {
            external_settings = serde_json::Value::Object(serde_json::Map::new());
        }
        external_settings[SETTINGS_IDENTIFIER] =
            serde_json::to_value(self.settings.clone()).expect("Epic serialization fail.");
        settings.update_external_preferences(external_settings);
        settings.save_ok();
    }

    fn update_verdict(&mut self, steamid: SteamID, verdict: Verdict) {
        let record = self.client.mac.players.records.entry(steamid).or_default();
        record.verdict = verdict;
        self.client.mac.players.records.save_ok();
    }

    fn insert_new_pfp(&mut self, pfp_hash: Arc<str>, bytes: Bytes) {
        let handle = iced::widget::image::Handle::from_memory(bytes);
        self.pfp_cache.insert(pfp_hash, handle);
    }

    fn request_pfp_lookup(&self, pfp_hash: Arc<str>, pfp_url: Arc<str>) -> iced::Command<Message> {
        if self.pfp_cache.contains_key(&pfp_hash) {
            return iced::Command::none();
        }

        iced::Command::perform(
            async move {
                match reqwest::get(&*pfp_url).await {
                    Ok(resp) => (pfp_hash, resp.bytes().await.map_err(|_| ())),
                    Err(_) => (pfp_hash, Err(())),
                }
            },
            |(pfp_hash, resp)| Message::PfpLookupResponse(pfp_hash, resp),
        )
    }
}

impl Drop for App {
    fn drop(&mut self) { self.save_settings(); }
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

    let players = Players::new(playerlist, settings.get_steam_user());

    let mac = MACState {
        server: Server::new(),
        settings,
        players,
    };

    let app_settings: AppSettings = mac
        .settings
        .get_external_preferences()
        .get(SETTINGS_IDENTIFIER)
        .and_then(|v| serde_json::from_value(v.clone()).ok())
        .unwrap_or_default();

    let mac_event_handler = EventLoop::new()
        .add_handler(CommandManager::new())
        .add_handler(ConsoleParser::default())
        .add_handler(ExtractNewPlayers)
        .add_handler(LookupProfiles::new())
        .add_handler(LookupFriends::new());

    let client = Client {
        mac,
        mac_event_handler,
    };
    let mut iced_settings = iced::Settings::with_flags((client, app_settings.clone()));
    if let Some(pos) = app_settings.window_pos {
        iced_settings.window.position = iced::window::Position::Specific(pos.0, pos.1);
    }
    if let Some(size) = app_settings.window_size {
        iced_settings.window.size = size;
    }

    App::run(iced_settings).expect("Failed to run app.");
}
