use clap::Parser;
use client_backend::{
    app::{start_backend, Args},
    state::SharedState,
};
use iced::{
    event::Event,
    widget::{column, Container, Text},
    Application, Command, Length,
};
use settings::{AppSettings, SETTINGS_IDENTIFIER};

mod settings;
mod tracing;

fn main() {
    let _guard = tracing::init_tracing();

    let args = Args::parse();
    let (state, _) = start_backend(args);

    let app_settings: AppSettings = state
        .settings
        .read()
        .get_external_preferences()
        .get(SETTINGS_IDENTIFIER)
        .map(|v| serde_json::from_value(v.clone()).ok())
        .flatten()
        .unwrap_or(Default::default());

    let mut iced_settings = iced::Settings::with_flags((state, app_settings.clone()));
    if let Some(pos) = app_settings.window_pos {
        iced_settings.window.position = iced::window::Position::Specific(pos.0, pos.1);
    }
    if let Some(size) = app_settings.window_size {
        iced_settings.window.size = size;
    }

    App::run(iced_settings).unwrap();
}

type IcedContainer<'a> = Container<'a, Message, iced::Renderer<iced::Theme>>;

pub struct App {
    client: SharedState,
    settings: AppSettings,
}

#[derive(Debug, Clone)]
pub enum Message {
    EventOccurred(Event),
}

#[derive(Debug, Clone)]
pub enum View {}

impl Application for App {
    type Executor = iced::executor::Default;
    type Message = Message;
    type Theme = iced::Theme;
    type Flags = (SharedState, AppSettings);

    fn new(flags: Self::Flags) -> (Self, iced::Command<Self::Message>) {
        (
            App {
                client: flags.0,
                settings: flags.1,
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
        iced::subscription::events().map(Message::EventOccurred)
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
        let mut settings = self.client.settings.write();
        let mut external_settings = settings.get_external_preferences().clone();
        if !external_settings.is_object() {
            external_settings = serde_json::Value::Object(serde_json::Map::new());
        }
        external_settings[SETTINGS_IDENTIFIER] =
            serde_json::to_value(self.settings.clone()).unwrap();
        settings.update_external_preferences(external_settings);
        settings.save_ok();
    }
}

impl Drop for App {
    fn drop(&mut self) {
        self.save_settings();
    }
}
