use std::{
    any::TypeId,
    cell::RefCell,
    time::{Duration, Instant},
};

use clap::Parser;
use client_backend::{
    args::Args,
    io::{IOManager, IOManagerMessage, IOOutput},
    player::SteamInfo,
    player_records::PlayerRecords,
    server::Server,
    settings::Settings,
    steamapi::{SteamAPIManager, SteamAPIMessage},
    steamid_ng::SteamID,
};
use iced::{
    event::Event,
    widget::{column, Container, Text},
    Application, Command, Length,
};
use settings::{AppSettings, SETTINGS_IDENTIFIER};
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};

mod settings;
mod tracing_setup;

pub struct Client {
    pub settings: Settings,
    pub server: Server,
    pub io_send: UnboundedSender<IOManagerMessage>,
    pub api_send: UnboundedSender<SteamAPIMessage>,

    pub _io_management: RefCell<Option<(UnboundedReceiver<Vec<IOOutput>>, IOManager)>>,
    pub _api_management:
        RefCell<Option<(UnboundedReceiver<(SteamID, SteamInfo)>, SteamAPIManager)>>,
}

type IcedContainer<'a> = Container<'a, Message, iced::Renderer<iced::Theme>>;

pub struct App {
    client: Client,
    settings: AppSettings,

    refresh_tick: u64,
}

#[derive(Debug, Clone)]
pub enum Message {
    EventOccurred(Event),
    RefreshTimerTick(Instant),
    ClientIO(Vec<IOOutput>),
    SteamAPIResponse((SteamID, SteamInfo)),
}

#[derive(Debug, Clone)]
pub enum View {}

impl Application for App {
    type Executor = iced::executor::Default;
    type Message = Message;
    type Theme = iced::Theme;
    type Flags = (Client, AppSettings);

    fn new(flags: Self::Flags) -> (Self, iced::Command<Self::Message>) {
        (
            App {
                client: flags.0,
                settings: flags.1,
                refresh_tick: 0,
            },
            Command::none(),
        )
    }

    fn title(&self) -> String {
        String::from("MAC Client")
    }

    fn theme(&self) -> iced::Theme {
        iced::Theme::Dark
    }

    fn subscription(&self) -> iced::Subscription<Self::Message> {
        let io_management = self.client._io_management.replace(None);
        let api_management = self.client._api_management.replace(None);

        iced::Subscription::batch([
            iced::subscription::events().map(Message::EventOccurred),
            iced::time::every(Duration::from_secs(2)).map(Message::RefreshTimerTick),
            iced::subscription::channel(TypeId::of::<IOManager>(), 100, |mut output| async move {
                if let Some((mut io_recv, mut io)) = io_management {
                    tokio::task::spawn(async move {
                        io.io_loop().await;
                    });

                    loop {
                        output
                            .try_send(Message::ClientIO(io_recv.recv().await.unwrap()))
                            .unwrap();
                    }
                } else {
                    panic!("There should have been an IOManager here!")
                }
            }),
            iced::subscription::channel(
                TypeId::of::<SteamAPIManager>(),
                100,
                |mut output| async move {
                    if let Some((mut api_recv, mut api)) = api_management {
                        tokio::task::spawn(async move {
                            api.api_loop().await;
                        });

                        loop {
                            output
                                .try_send(Message::SteamAPIResponse(api_recv.recv().await.unwrap()))
                                .unwrap();
                        }
                    } else {
                        panic!("There should have been a SteamAPIManager here!")
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
            Message::ClientIO(outs) => {
                self.handle_client_outputs(outs);
            }
            Message::SteamAPIResponse((steamid, info)) => {
                self.client.server.insert_steam_info(steamid, info);
            }
            Message::RefreshTimerTick(_) => {
                self.refresh_tick();
            }
        };

        Command::none()
    }

    fn view(&self) -> iced::Element<'_, Self::Message, iced::Renderer<Self::Theme>> {
        let content = column![Text::new("Hello world")];

        Container::new(content)
            .width(Length::Fill)
            .height(Length::Fill)
            .center_x()
            .center_y()
            .padding(50)
            .into()
    }
}

impl App {
    fn save_settings(&mut self) {
        let settings = &mut self.client.settings;
        let mut external_settings = settings.get_external_preferences().clone();
        if !external_settings.is_object() {
            external_settings = serde_json::Value::Object(serde_json::Map::new());
        }
        external_settings[SETTINGS_IDENTIFIER] =
            serde_json::to_value(self.settings.clone()).unwrap();
        settings.update_external_preferences(external_settings);
        settings.save_ok();
    }

    fn handle_client_outputs(&mut self, outs: Vec<IOOutput>) {
        outs.into_iter()
            .flat_map(|out| {
                self.client
                    .server
                    .handle_io_output(out, self.client.settings.get_steam_user())
            })
            .for_each(|new_player| {
                self.client
                    .api_send
                    .send(SteamAPIMessage::Lookup(new_player))
                    .unwrap()
            });
    }

    fn refresh_tick(&mut self) {
        if self.refresh_tick % 2 == 0 {
            self.client.server.refresh();
            self.client
                .io_send
                .send(IOManagerMessage::RunCommand(
                    client_backend::io::Command::Status,
                ))
                .unwrap();
        } else {
            self.client
                .io_send
                .send(IOManagerMessage::RunCommand(
                    client_backend::io::Command::G15,
                ))
                .unwrap();
        }
        self.refresh_tick += 1;
    }
}

impl Drop for App {
    fn drop(&mut self) {
        self.save_settings();
    }
}

fn main() {
    let _guard = tracing_setup::init_tracing();

    // Arg handling
    let args = Args::parse();

    // Load Settings
    let settings = Settings::load_or_create(&args);

    // Playerlist
    let playerlist = PlayerRecords::load_or_create(&args);

    // Server
    let server = Server::new(playerlist);

    // IO and API Manager
    let log_file_path = settings
        .get_tf2_directory()
        .to_path_buf()
        .join("tf/console.log");
    let rcon_password = settings.get_rcon_password();
    let api_key = settings.get_steam_api_key();

    let (io_send, io_recv) = unbounded_channel();
    let io_management = IOManager::new(log_file_path, rcon_password, io_recv);

    let (api_send, api_recv) = unbounded_channel();
    let api_management = SteamAPIManager::new(api_key, api_recv);

    let app_settings: AppSettings = settings
        .get_external_preferences()
        .get(SETTINGS_IDENTIFIER)
        .map(|v| serde_json::from_value(v.clone()).ok())
        .flatten()
        .unwrap_or(Default::default());

    let client = Client {
        settings,
        server,
        io_send,
        api_send,

        _io_management: RefCell::new(Some(io_management)),
        _api_management: RefCell::new(Some(api_management)),
    };
    let mut iced_settings = iced::Settings::with_flags((client, app_settings.clone()));
    if let Some(pos) = app_settings.window_pos {
        iced_settings.window.position = iced::window::Position::Specific(pos.0, pos.1);
    }
    if let Some(size) = app_settings.window_size {
        iced_settings.window.size = size;
    }

    App::run(iced_settings).unwrap();
}
