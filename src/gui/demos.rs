use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
    io::Read,
    os::unix::fs::MetadataExt,
    path::PathBuf,
    sync::mpsc::Sender,
    time::SystemTime,
};

use iced::{
    widget::{self, scrollable::Properties, Scrollable},
    Length,
};
use tf2_monitor_core::{
    demo_analyser::{self, AnalysedDemo},
    steamid_ng::SteamID,
    tf_demo_parser::demo::parser::analyser::Class,
};
use threadpool::ThreadPool;
use tokio::{io::AsyncReadExt, task::JoinSet};

use crate::{App, IcedElement, Message};

use super::{
    format_time, format_time_since,
    icons::{self, icon},
    styles::colours,
    tooltip, View, FONT_SIZE, PFP_SMALL_SIZE,
};

const CLASSES: [Class; 9] = [
    Class::Scout,
    Class::Sniper,
    Class::Soldier,
    Class::Demoman,
    Class::Medic,
    Class::Heavy,
    Class::Pyro,
    Class::Spy,
    Class::Engineer,
];

pub type AnalysedDemoID = u64;
type TokioReceiver<T> = tokio::sync::mpsc::UnboundedReceiver<T>;
type AnalysedDemoResult = (PathBuf, Option<(AnalysedDemoID, Box<AnalysedDemo>)>);

#[allow(clippy::module_name_repetitions)]
pub struct DemosState {
    demo_files: Vec<Demo>,
    demos_to_display: Vec<usize>,
    analysed_demos: HashMap<u64, AnalysedDemo>,
    /// Demos in progress
    analysing_demos: HashSet<PathBuf>,

    demos_per_page: usize,
    page: usize,

    request_analysis: Sender<PathBuf>,
    #[allow(clippy::pub_underscore_fields, clippy::type_complexity)]
    pub _demo_analysis_output: RefCell<Option<TokioReceiver<AnalysedDemoResult>>>,
}

#[derive(Debug, Clone)]
pub struct Demo {
    name: String,
    path: PathBuf,
    created: SystemTime,
    /// In bytes
    file_size: u64,
    analysed: AnalysedDemoID,
}

#[derive(Debug, Clone)]
#[allow(clippy::module_name_repetitions)]
pub enum DemosMessage {
    Refresh,
    SetDemos(Vec<Demo>),
    SetPage(usize),
    AnalyseDemo(PathBuf),
    AnalyseAll,
    DemoAnalysed(AnalysedDemoResult),
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
            let pool = ThreadPool::new(num_cpus::get());

            while let Ok(demo_path) = request_rx.recv() {
                tracing::debug!("Received request to analyse {demo_path:?}");
                let tx = completed_tx.clone();
                pool.execute(move || {
                    tracing::debug!("Analysing {demo_path:?}");
                    let payload = std::fs::File::open(&demo_path)
                        .map_err(|e| tracing::error!("Failed to read demo file {demo_path:?}: {e}"))
                        .ok()
                        .and_then(|mut f| {
                            let created = f.metadata().and_then(|m| m.created()).ok()?;
                            let mut bytes = Vec::new();
                            let _ = f.read_to_end(&mut bytes).ok()?;
                            let hash = demo_analyser::hash_demo(&bytes, created);
                            let demo = demo_analyser::AnalysedDemo::new(&bytes).ok()?;
                            Some((hash, Box::new(demo)))
                        });

                    tracing::debug!("Finished analysing {demo_path:?}");
                    tx.send((demo_path, payload)).ok();
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

    #[allow(clippy::missing_panics_doc)]
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
            DemosMessage::DemoAnalysed((demo_path, analysed_demo)) => {
                state.demos.analysing_demos.remove(&demo_path);

                match analysed_demo {
                    Some((hash, analysed_demo)) => {
                        state.demos.analysed_demos.insert(hash, *analysed_demo);
                        tracing::debug!("Successfully got analysed demo {demo_path:?}");
                    }
                    None => {
                        tracing::error!("Failed to analyse demo {demo_path:?}");
                    }
                }
            }
            DemosMessage::AnalyseAll => {
                for d in state
                    .demos
                    .demos_to_display
                    .iter()
                    .copied()
                    .filter_map(|i| state.demos.demo_files.get(i))
                {
                    if state.demos.analysed_demos.contains_key(&d.analysed)
                        || state.demos.analysing_demos.contains(&d.path)
                    {
                        continue;
                    }

                    state.demos.analysing_demos.insert(d.path.clone());
                    state
                        .demos
                        .request_analysis
                        .send(d.path.clone())
                        .expect("Couldn't request analysis of demo. Demo analyser thread ded?");
                }
            }
        }

        iced::Command::none()
    }

    /// Update the list of which demos should be displayed in order
    fn update_demos_to_display(&mut self) {
        let mut demos: Vec<_> = self.demo_files.iter().enumerate().collect();
        demos.sort_by_key(|(_, demo)| demo.created);
        demos.reverse();
        self.demos_to_display = demos.iter().map(|(idx, _)| *idx).collect();
    }

    /// Clear the current store of demo files and search the directories for new demo files
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
                            let mut demo_file = tokio::fs::File::open(&file_path).await.ok()?;

                            let mut header_bytes = [0u8; 0x430];
                            demo_file.read_exact(&mut header_bytes).await.ok()?;

                            Some(Demo {
                                name: file_name,
                                path: file_path,
                                created,
                                analysed: demo_analyser::hash_demo(&header_bytes, created),
                                file_size: metadata.size(),
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

#[allow(clippy::module_name_repetitions)]
pub fn demos_list_view(state: &App) -> IcedElement<'_> {
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

    let arrow_button = |contents: &str| {
        widget::button(
            widget::column![widget::text(contents)]
                .width(25)
                .align_items(iced::Alignment::Center),
        )
    };

    let header = widget::row![
        widget::Space::with_width(15),
        arrow_button("<<").on_press(DemosMessage::SetPage(0).into()),
        arrow_button("<")
            .on_press(DemosMessage::SetPage(state.demos.page.saturating_sub(1)).into()),
        widget::column![widget::text(format!("{}", state.demos.page + 1))]
            .align_items(iced::Alignment::Center)
            .width(75),
        arrow_button(">").on_press(
            DemosMessage::SetPage(state.demos.page.saturating_add(1).min(num_pages - 1)).into()
        ),
        arrow_button(">>").on_press(DemosMessage::SetPage(num_pages - 1).into()),
        widget::Space::with_width(Length::FillPortion(1)),
        widget::button(widget::text("Analyse all")).on_press(DemosMessage::AnalyseAll.into()),
        widget::Space::with_width(Length::FillPortion(1)),
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

    for &d in state
        .demos
        .demos_to_display
        .iter()
        .skip(state.demos.page * state.demos.demos_per_page)
        .take(state.demos.demos_per_page)
    {
        contents = contents.push(row(state, d));
    }

    widget::column![
        widget::Space::with_height(15),
        header,
        widget::Space::with_height(15),
        widget::horizontal_rule(1),
        Scrollable::new(contents)
    ]
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}

#[must_use]
#[allow(clippy::too_many_lines)]
fn row(state: &App, demo_index: usize) -> IcedElement<'_> {
    let Some(demo) = state.demos.demo_files.get(demo_index) else {
        return widget::row![widget::text("Invalid demo")].into();
    };

    let recorded_ago = SystemTime::now()
        .duration_since(demo.created)
        .unwrap_or_default();

    let recorded_ago_str = format_time_since(recorded_ago.as_secs());

    let mut contents = widget::row![]
        .align_items(iced::Alignment::Center)
        .height(PFP_SMALL_SIZE)
        .spacing(15);

    // Analysed
    if let Some(analysed) = state.demos.analysed_demos.get(&demo.analysed) {
        let hostname = if analysed.server_name.len() > 30 {
            let mut host = analysed.server_name.split_at(27).0.to_string();
            host.push_str("...");
            host
        } else {
            analysed.server_name.clone()
        };

        let map = if analysed.header.map.len() > 30 {
            let mut map = analysed.header.map.split_at(27).0.to_string();
            map.push_str("...");
            map
        } else {
            analysed.header.map.clone()
        };

        contents = contents.push(
            widget::row![widget::button(widget::text(hostname).size(FONT_SIZE))
                .on_press(Message::SetView(View::AnalysedDemo(demo_index)))]
            .width(200),
        );
        contents = contents.push(widget::text(recorded_ago_str).width(100));
        contents = contents.push(widget::text(map).width(Length::FillPortion(4)));

        let mut badges = widget::row![]
            .spacing(15)
            .align_items(iced::Alignment::Center)
            .width(Length::FillPortion(3));

        if let Some(player) = analysed.players.get(&analysed.user) {
            badges = badges.push(tooltip(
                widget::row![
                    widget::text(player.kills.len()).style(colours::green()),
                    widget::text("/"),
                    widget::text(player.deaths.len()).style(colours::red()),
                    widget::text("/"),
                    widget::text(player.assists.len()).style(colours::team_blu()),
                ]
                .spacing(5),
                widget::text("Kills/Deaths/Assists"),
            ));
            badges = badges.push(widget::horizontal_space());

            for &c in player.most_played_classes.iter().take(3) {
                let details = &player.class_details[c as usize];
                let time_played = format_time(details.time);

                badges = badges.push(tooltip(
                    icon(icons::CLASS[c as usize]).style(colours::orange()),
                    widget::column![
                        widget::text(format!("{c:?}")),
                        widget::row![widget::text("Time played: "), widget::text(time_played),],
                        widget::row![
                            widget::text("K/D/A: "),
                            widget::text(format!(
                                "{}/{}/{}",
                                details.num_kills, details.num_deaths, details.num_assists
                            )),
                        ],
                    ],
                ));
            }
        } else {
            badges = badges.push(widget::text("Missing player"));
        }

        contents = contents.push(badges);

        // <Player> on <Server> (<map>) for <time>
        #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
        let duration = analysed.header.duration as u32;
        let duration = format_time(duration);

        contents = contents.push(
            widget::column![widget::text(duration)]
                .align_items(iced::Alignment::End)
                .width(65),
        );
    } else {
        let analysing = state.demos.analysing_demos.contains(&demo.path);

        let mut analyse_button = widget::button(widget::text("Analyse demo").size(FONT_SIZE));

        if !analysing {
            analyse_button = analyse_button
                .on_press(Message::Demos(DemosMessage::AnalyseDemo(demo.path.clone())));
        }

        contents = contents.push(analyse_button);
        contents = contents.push(widget::text(&demo.name).width(Length::FillPortion(2)));
        contents = contents.push(widget::text(recorded_ago_str).width(Length::FillPortion(1)));
        contents = contents.push(
            widget::text(format!("{:.2} MB", demo.file_size as f32 / 1_000_000.0))
                .width(Length::FillPortion(1)),
        );
    }

    // widget::column![top_row, bottom_row]
    contents.width(Length::Fill).into()
}

#[allow(clippy::too_many_lines)]
pub fn analysed_demo_view(state: &App, demo_index: usize) -> IcedElement<'_> {
    let Some(demo) = state.demos.demo_files.get(demo_index) else {
        return widget::column![
            widget::vertical_space(),
            widget::text("Invalid demo"),
            widget::vertical_space()
        ]
        .width(Length::Fill)
        .align_items(iced::Alignment::Center)
        .into();
    };

    let demo_name_button = tooltip(
        widget::button(widget::text(&demo.name).size(FONT_SIZE)).on_press(
            Message::CopyToClipboard(demo.path.to_string_lossy().to_string()),
        ),
        widget::text("Copy file path"),
    );

    let mut open_folder_button = widget::button("Open folder");
    if let Some(path) = demo.path.parent().and_then(|p| p.to_str()) {
        open_folder_button = open_folder_button.on_press(Message::Open(path.to_string()));
    }

    // Demo name, size, buttons
    let mut contents = widget::column![
        widget::Space::with_height(0),
        widget::row![
            widget::Space::with_width(0),
            demo_name_button,
            widget::text(format!("{:.2} MB", demo.file_size as f32 / 1_000_000.0)),
            widget::horizontal_space(),
            widget::text(format!(
                "Created {}",
                format_time_since(
                    SystemTime::now()
                        .duration_since(demo.created)
                        .unwrap_or_default()
                        .as_secs()
                )
            )),
            open_folder_button,
            widget::button("Create replay").on_press(Message::SetReplay(demo.path.clone())),
            widget::Space::with_width(0),
        ]
        .align_items(iced::Alignment::Center)
        .spacing(15)
    ]
    .width(Length::Fill)
    .spacing(15);

    let Some(analysed) = state.demos.analysed_demos.get(&demo.analysed) else {
        contents = contents.push(widget::text("Demo not analysed"));
        return contents.into();
    };

    // Server name, IP, duration
    contents = contents.push(
        widget::row![
            widget::Space::with_width(0),
            widget::text(&analysed.server_name),
            widget::Space::with_width(10),
            widget::text(format!("({})", analysed.header.server)),
            widget::horizontal_space(),
            widget::text(format_time(analysed.header.duration as u32)),
            widget::Space::with_width(0),
        ]
        .align_items(iced::Alignment::Center)
        .spacing(15),
    );

    // Players heading
    let mut player_classes_heading = widget::row![
        widget::Space::with_width(0),
        widget::text("Player").width(150),
        widget::text("Total").width(Length::FillPortion(1)),
    ]
    .spacing(15)
    .align_items(iced::Alignment::Center);

    for c in CLASSES {
        player_classes_heading = player_classes_heading.push(tooltip(
            icon(icons::CLASS[c as usize])
                .width(Length::FillPortion(1))
                .style(colours::orange()),
            widget::text(format!("{c:?}")),
        ));
    }
    player_classes_heading = player_classes_heading.push(widget::Space::with_width(15));

    contents = contents.push(player_classes_heading);

    // Player list
    let mut player_list = widget::column![].spacing(5);
    player_list = player_list.push(player_row(analysed, analysed.user));
    for s in analysed
        .players
        .keys()
        .copied()
        .filter(|s| *s != analysed.user)
    {
        player_list = player_list.push(player_row(analysed, s));
    }

    contents = contents.push(widget::scrollable(player_list).direction(
        widget::scrollable::Direction::Vertical(Properties::default()),
    ));
    contents = contents.push(widget::Space::with_height(15));

    contents.into()
}

fn player_row(analysed: &AnalysedDemo, steamid: SteamID) -> IcedElement<'_> {
    let Some(player) = analysed.players.get(&steamid) else {
        return widget::row![widget::text("Invalid Player")]
            .height(PFP_SMALL_SIZE)
            .align_items(iced::Alignment::Center)
            .into();
    };

    let format_kda = |kills, deaths, assists| {
        widget::row![
            widget::text(kills).style(colours::green()).size(FONT_SIZE),
            widget::text(" / ").size(FONT_SIZE),
            widget::text(deaths).style(colours::red()).size(FONT_SIZE),
            widget::text(" / ").size(FONT_SIZE),
            widget::text(assists)
                .style(colours::team_blu())
                .size(FONT_SIZE),
        ]
    };

    let mut contents = widget::row![
        widget::Space::with_width(0),
        widget::column![widget::button(widget::text(&player.name).size(FONT_SIZE))
            .on_press(Message::SelectPlayer(steamid))]
        .width(150),
        widget::column![
            widget::text(format_time(player.time)).size(FONT_SIZE),
            format_kda(
                player.kills.len() as u32,
                player.deaths.len() as u32,
                player.assists.len() as u32
            ),
        ]
        .align_items(iced::Alignment::Center)
        .width(Length::FillPortion(1))
    ]
    .spacing(15)
    .align_items(iced::Alignment::Center);

    for c in CLASSES {
        let details = &player.class_details[c as usize];

        if details.time == 0 {
            contents = contents.push(widget::column![].width(Length::FillPortion(1)));
            continue;
        }

        contents = contents.push(
            widget::column![
                widget::text(format_time(details.time)).size(FONT_SIZE),
                format_kda(details.num_kills, details.num_deaths, details.num_assists),
            ]
            .align_items(iced::Alignment::Center)
            .width(Length::FillPortion(1)),
        );
    }
    contents = contents.push(widget::Space::with_width(15));

    contents.width(Length::Fill).into()
}
