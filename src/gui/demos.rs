use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
    path::PathBuf,
    sync::mpsc::Sender,
    time::SystemTime,
};

use iced::{
    widget::{self, Scrollable},
    Length,
};
use tf2_monitor_core::demo_analyser::{self, AnalysedDemo};
use threadpool::ThreadPool;
use tokio::task::JoinSet;

use crate::{App, IcedElement, Message};

use super::PFP_SMALL_SIZE;

type TokioReceiver<T> = tokio::sync::mpsc::UnboundedReceiver<T>;

#[allow(clippy::module_name_repetitions)]
pub struct DemosState {
    demo_files: Vec<Demo>,
    demos_to_display: Vec<usize>,
    analysed_demos: HashMap<PathBuf, AnalysedDemo>,
    /// Demos in progress
    analysing_demos: HashSet<PathBuf>,

    demos_per_page: usize,
    page: usize,

    request_analysis: Sender<PathBuf>,
    pub _demo_analysis_output: RefCell<Option<TokioReceiver<(PathBuf, Option<AnalysedDemo>)>>>,
}

#[derive(Debug, Clone)]
pub struct Demo {
    name: String,
    path: PathBuf,
    created: SystemTime,
}

#[derive(Debug, Clone)]
#[allow(clippy::module_name_repetitions)]
pub enum DemosMessage {
    Refresh,
    SetDemos(Vec<Demo>),
    SetPage(usize),
    AnalyseDemo(PathBuf),
    DemoAnalysed(PathBuf, Option<AnalysedDemo>),
}

impl From<DemosMessage> for Message {
    fn from(val: DemosMessage) -> Self {
        Self::Demos(val)
    }
}

impl DemosState {
    #[must_use]
    pub fn new() -> Self {
        let (request_tx, request_rx) = std::sync::mpsc::channel();
        let (completed_tx, completed_rx) = tokio::sync::mpsc::unbounded_channel();

        // Spawn analyser thread
        std::thread::spawn(move || {
            let pool = ThreadPool::new(4);

            while let Ok(demo_path) = request_rx.recv() {
                let tx = completed_tx.clone();
                pool.execute(move || {
                    let demo = std::fs::read(&demo_path)
                        .map_err(|e| tracing::error!("Failed to read demo file {demo_path:?}: {e}"))
                        .ok()
                        .and_then(|demo_bytes| demo_analyser::AnalysedDemo::new(&demo_bytes).ok());
                    tx.send((demo_path, demo))
                        .expect("Failed to send analysed demo to main thread.");
                });
            }
        });

        Self {
            demo_files: Vec::new(),
            demos_to_display: Vec::new(),
            analysed_demos: HashMap::new(),
            analysing_demos: HashSet::new(),

            demos_per_page: 50,
            page: 0,

            request_analysis: request_tx,
            _demo_analysis_output: RefCell::new(Some(completed_rx)),
        }
    }

    pub fn handle_message(state: &mut App, message: DemosMessage) -> iced::Command<Message> {
        match message {
            DemosMessage::Refresh => return Self::refresh_demos(state),
            DemosMessage::SetPage(page) => state.demos.page = page,
            DemosMessage::SetDemos(demo_files) => {
                state.demos.demo_files = demo_files;
                state.demos.update_demos_to_display();
            }
            DemosMessage::AnalyseDemo(demo_path) => {
                if state.demos.analysing_demos.contains(&demo_path) {
                    return iced::Command::none();
                }

                state.demos.analysing_demos.insert(demo_path.clone());
                state
                    .demos
                    .request_analysis
                    .send(demo_path)
                    .expect("Couldn't request analysis of demo. Demo analyser thread ded?");
            }
            DemosMessage::DemoAnalysed(demo_path, analysed_demo) => {
                state.demos.analysing_demos.remove(&demo_path);

                match analysed_demo {
                    Some(analysed_demo) => {
                        state.demos.analysed_demos.insert(demo_path, analysed_demo);
                    }
                    None => {
                        tracing::error!("Failed to analyse demo {demo_path:?}");
                    }
                }
            }
        }

        iced::Command::none()
    }

    fn update_demos_to_display(&mut self) {
        let mut demos: Vec<_> = self.demo_files.iter().enumerate().collect();
        demos.sort_by_key(|(_, demo)| demo.created);
        demos.reverse();
        self.demos_to_display = demos.iter().map(|(idx, _)| *idx).collect();
    }

    pub fn refresh_demos(state: &App) -> iced::Command<Message> {
        let mut dirs_to_search = Vec::new();
        if let Some(tf2_dir) = &state.mac.settings.tf2_directory {
            dirs_to_search.push(tf2_dir.join("tf/demos"));
        }

        iced::Command::perform(
            async move {
                let mut demos = Vec::new();

                // Directories
                for dir in dirs_to_search {
                    tracing::debug!("Searching for demos in {dir:?}");

                    let Ok(mut dir_entries) = tokio::fs::read_dir(&dir).await.map_err(|e| {
                        tracing::error!(
                            "Coudldn't read directory while looking for demos in {dir:?}: {e}"
                        );
                    }) else {
                        continue;
                    };

                    // Files in each directory
                    let mut join_handles: JoinSet<Option<Demo>> = JoinSet::new();
                    while let Ok(Some(dir_entry)) = dir_entries.next_entry().await {
                        join_handles.spawn(async move {
                            let file_type = dir_entry.file_type().await.ok()?;

                            if !file_type.is_file() {
                                return None;
                            }

                            let file_name = dir_entry.file_name().to_string_lossy().to_string();
                            #[allow(clippy::case_sensitive_file_extension_comparisons)]
                            if !file_name.ends_with(".dem") {
                                return None;
                            }

                            let metadata = dir_entry.metadata().await.ok()?;
                            let created = metadata.created().ok()?;
                            let file_path = dir_entry.path();

                            Some(Demo {
                                name: file_name,
                                path: file_path,
                                created,
                            })
                        });
                    }

                    while let Some(result) = join_handles.join_next().await {
                        let Ok(Some(demo)) = result else {
                            continue;
                        };

                        tracing::debug!("Added demo {}", demo.name);
                        demos.push(demo);
                    }
                }
                demos
            },
            |demos| Message::Demos(DemosMessage::SetDemos(demos)),
        )
    }
}

impl Default for DemosState {
    fn default() -> Self {
        Self::new()
    }
}

pub fn view(state: &App) -> IcedElement<'_> {
    // Pages
    let num_pages = state.demos.demo_files.len() / state.demos.demos_per_page + 1;
    let displaying_start =
        (state.demos.page * state.demos.demos_per_page + 1).min(state.demos.demo_files.len());
    let displaying_end = if state.demos.page == num_pages - 1 {
        (num_pages - 1) * state.demos.demos_per_page
            + state.demos.demo_files.len() % state.demos.demos_per_page
    } else {
        (state.demos.page + 1) * state.demos.demos_per_page
    };

    let button = |contents: &str| {
        widget::button(
            widget::column![widget::text(contents)]
                .width(25)
                .align_items(iced::Alignment::Center),
        )
    };

    let header = widget::row![
        widget::Space::with_width(15),
        button("<<").on_press(DemosMessage::SetPage(0).into()),
        button("<").on_press(DemosMessage::SetPage(state.demos.page.saturating_sub(1)).into()),
        widget::column![widget::text(format!("{}", state.demos.page + 1))]
            .align_items(iced::Alignment::Center)
            .width(75),
        button(">").on_press(
            DemosMessage::SetPage(state.demos.page.saturating_add(1).min(num_pages - 1)).into()
        ),
        button(">>").on_press(DemosMessage::SetPage(num_pages - 1).into()),
        widget::horizontal_space(),
        widget::text(format!(
            "Displaying {displaying_start} - {displaying_end} of {} ({num_pages} {})",
            state.demos.demo_files.len(),
            if num_pages == 1 { "page" } else { "pages" }
        )),
        widget::Space::with_width(15),
    ]
    .spacing(3)
    .align_items(iced::Alignment::Center);

    // Actual demos
    let mut contents = widget::column![].spacing(3).padding(15);

    for d in state
        .demos
        .demos_to_display
        .iter()
        .skip(state.demos.page * state.demos.demos_per_page)
        .take(state.demos.demos_per_page)
        .filter_map(|idx| state.demos.demo_files.get(*idx))
    {
        contents = contents.push(row(state, d));
    }

    widget::column![
        widget::Space::with_height(15),
        header,
        widget::Space::with_height(15),
        Scrollable::new(contents)
    ]
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}

#[must_use]
fn row<'a>(state: &'a App, demo: &'a Demo) -> IcedElement<'a> {
    widget::row![widget::text(&demo.name)]
        .width(Length::Fill)
        .height(PFP_SMALL_SIZE) // Just because it's consistent with the records
        .into()
}
