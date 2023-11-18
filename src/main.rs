use clap::Parser;
use client_backend::{
    app::{start_backend, Args},
    state::SharedState,
};
use iced::{
    widget::{column, Container, Text},
    Application, Command, Length, Settings,
};

mod tracing;

type IcedContainer<'a> = Container<'a, Message, iced::Renderer<iced::Theme>>;

pub struct App {
    state: SharedState,
}

#[derive(Debug, Clone)]
pub enum Message {}

#[derive(Debug, Clone)]
pub enum View {}

impl Application for App {
    type Executor = iced::executor::Default;
    type Message = Message;
    type Theme = iced::Theme;
    type Flags = SharedState;

    fn new(flags: Self::Flags) -> (Self, iced::Command<Self::Message>) {
        (App { state: flags }, Command::none())
    }

    fn title(&self) -> String {
        String::from("MAC Client")
    }

    fn theme(&self) -> iced::Theme {
        iced::Theme::Dark
    }

    fn update(&mut self, message: Self::Message) -> iced::Command<Self::Message> {
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

fn main() {
    let _guard = tracing::init_tracing();

    let args = Args::parse();
    let (state, _) = start_backend(args);

    App::run(Settings::with_flags(state)).unwrap();
}
