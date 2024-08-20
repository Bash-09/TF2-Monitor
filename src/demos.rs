use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
    fmt::Display,
    io::{ErrorKind, Read},
    path::PathBuf,
    sync::mpsc::Sender,
    time::SystemTime,
};

use serde::{Deserialize, Serialize};
use tf2_monitor_core::{
    demo_analyser::{self, AnalysedDemo},
    settings::ConfigFilesError,
    steamid_ng::SteamID,
    tf_demo_parser::demo::parser::analyser::Class,
};
use thiserror::Error;
use threadpool::ThreadPool;
use tokio::{io::AsyncReadExt, sync::mpsc::UnboundedReceiver, task::JoinSet};

use crate::{App, Message, APP};

pub const CLASSES: [Class; 9] = [
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

pub const SORT_OPTIONS: &[SortBy] = &[SortBy::FileCreated, SortBy::FileSize, SortBy::FileName];
pub const SORT_DIRECTIONS: &[SortDirection] =
    &[SortDirection::Ascending, SortDirection::Descending];

pub type AnalysedDemoID = tf2_monitor_core::md5::Digest;
type AnalysedDemoResult = (PathBuf, Option<(AnalysedDemoID, Box<AnalysedDemo>)>);

pub struct State {
    pub demo_files: Vec<Demo>,
    pub demos_to_display: Vec<usize>,
    pub analysed_demos: HashMap<AnalysedDemoID, AnalysedDemo>,
    /// Demos in progress
    pub analysing_demos: HashSet<PathBuf>,

    pub demos_per_page: usize,
    pub page: usize,

    pub request_analysis: Sender<PathBuf>,
    #[allow(clippy::pub_underscore_fields, clippy::type_complexity)]
    pub _demo_analysis_output: RefCell<Option<UnboundedReceiver<AnalysedDemoResult>>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Filters {
    pub sort_by: SortBy,
    pub direction: SortDirection,

    pub show_analysed: bool,
    pub show_non_analysed: bool,

    // Steamid (any format), name (case-insensitive, will include previous names if records exist)
    pub contains_players: Vec<String>,

    // Map, server name, IP, file name
    pub search: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default, PartialEq, Eq)]
pub enum SortBy {
    FileName,
    FileSize,
    #[default]
    FileCreated,
    DemoDuration,
    NumKills,
    NumDeaths,
    NumAssists,
    NumPlayers,
    Map,
    ServerName,
}

impl Display for SortBy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let str = match self {
            Self::FileName => "File Name",
            Self::FileSize => "File Size",
            Self::FileCreated => "Created",
            Self::DemoDuration => "Duration",
            Self::NumKills => "Kills",
            Self::NumDeaths => "Deaths",
            Self::NumAssists => "Assists",
            Self::NumPlayers => "Player Count",
            Self::Map => "Map",
            Self::ServerName => "Server Name",
        };
        write!(f, "{str}")
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default, PartialEq, Eq)]
pub enum SortDirection {
    Ascending,
    #[default]
    Descending,
}

impl Display for SortDirection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let str = match self {
            Self::Ascending => "Ascending",
            Self::Descending => "Descending",
        };
        write!(f, "{str}")
    }
}

#[derive(Debug, Clone)]
pub struct Demo {
    pub name: String,
    pub path: PathBuf,
    pub created: SystemTime,
    /// In bytes
    pub file_size: u64,
    pub analysed: AnalysedDemoID,
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

    FilterSortBy(SortBy),
    FilterSortDirection(SortDirection),
    FilterShowAnalysed(bool),
    FilterShowNonAnalysed(bool),
    FilterContainsPlayerUpdate(String),
    FilterContainsPlayerAdd,
    FilterSearchUpdate(String),
    FilterRemovePlayer(usize),
    ApplyFilters,
    ClearFilters,
}

impl From<DemosMessage> for Message {
    fn from(val: DemosMessage) -> Self {
        Self::Demos(val)
    }
}

impl State {
    #[must_use]
    pub fn new() -> Self {
        let (request_tx, completed_rx) = spawn_demo_analyser_thread();

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

    #[allow(
        clippy::missing_panics_doc,
        clippy::too_many_lines,
        clippy::cognitive_complexity
    )]
    pub fn handle_message(state: &mut App, message: DemosMessage) -> iced::Command<Message> {
        match message {
            DemosMessage::Refresh => {
                state.update_demo_list();
                return Self::refresh_demos(state);
            }
            DemosMessage::SetPage(page) => state.demos.page = page,
            DemosMessage::SetDemos(demo_files) => {
                state.demos.demo_files = demo_files;
                state.update_demo_list();

                // Check if the demos have been cached
                let mut commands = Vec::new();
                for h in state
                    .demos
                    .demo_files
                    .iter()
                    .map(|d| d.analysed)
                    .filter(|h| !state.demos.analysed_demos.contains_key(h))
                {
                    commands.push(iced::Command::perform(
                        async move {
                            let r = read_cached_demo(h).await;
                            match &r {
                                Err(CachedDemoError::Io(e)) if e.kind() == ErrorKind::NotFound => {}
                                Err(e) => {
                                    tracing::error!("Failed to load cached demo ({h:x}): {e}");
                                }
                                _ => {}
                            }

                            r.ok()
                        },
                        |r| Message::Demos(DemosMessage::DemoAnalysed((PathBuf::new(), r))),
                    ));
                }
                return iced::Command::batch(commands);
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
                    None if !demo_path.as_os_str().is_empty() => {
                        tracing::error!("Failed to analyse demo {demo_path:?}");
                    }
                    None => {}
                }
            }
            DemosMessage::AnalyseAll => {
                for d in &state.demos.demo_files {
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
            DemosMessage::ApplyFilters => {
                state.update_demo_list();
            }
            DemosMessage::ClearFilters => {
                state.settings.demo_filters = Filters::new();
                state.update_demo_list();
            }
            DemosMessage::FilterSortBy(sort_by) => {
                state.settings.demo_filters.sort_by = sort_by;
                state.update_demo_list();
            }
            DemosMessage::FilterSortDirection(dir) => {
                if dir != state.settings.demo_filters.direction {
                    state.demos.demos_to_display.reverse();
                }
                state.settings.demo_filters.direction = dir;
            }
            DemosMessage::FilterShowAnalysed(show) => {
                state.settings.demo_filters.show_analysed = show;
                state.update_demo_list();
            }
            DemosMessage::FilterShowNonAnalysed(show) => {
                state.settings.demo_filters.show_non_analysed = show;
                state.update_demo_list();
            }
            DemosMessage::FilterContainsPlayerUpdate(player) => {
                if let Some(last) = state
                    .settings
                    .demo_filters
                    .contains_players
                    .iter_mut()
                    .last()
                {
                    *last = player;
                } else {
                    state.settings.demo_filters.contains_players = vec![player];
                }

                // state.update_demo_list();
            }
            DemosMessage::FilterContainsPlayerAdd => {
                if let Some(last) = state.settings.demo_filters.contains_players.iter().last() {
                    if !last.trim().is_empty() {
                        state
                            .settings
                            .demo_filters
                            .contains_players
                            .push(String::new());
                    }
                } else {
                    state.settings.demo_filters.contains_players = vec![];
                }

                state.update_demo_list();
            }
            DemosMessage::FilterSearchUpdate(search) => {
                state.settings.demo_filters.search = search;
                // state.update_demo_list();
            }
            DemosMessage::FilterRemovePlayer(i) => {
                state.settings.demo_filters.contains_players.remove(i);
                state.update_demo_list();
            }
        }

        iced::Command::none()
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
                            // Ensure is demo file
                            let file_type = dir_entry.file_type().await.ok()?;

                            if !file_type.is_file() {
                                return None;
                            }

                            let file_name = dir_entry.file_name().to_string_lossy().to_string();
                            #[allow(clippy::case_sensitive_file_extension_comparisons)]
                            if !file_name.ends_with(".dem") {
                                return None;
                            }

                            // Data
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
                                file_size: metadata.len(),
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

impl Default for State {
    fn default() -> Self {
        Self::new()
    }
}

// Spawn a thread with a thread pool to analyse demos. Requests for demos to be analysed
// can be sent over the channel and their result will eventually come back over the other one.
fn spawn_demo_analyser_thread() -> (Sender<PathBuf>, UnboundedReceiver<AnalysedDemoResult>) {
    let (request_tx, request_rx) = std::sync::mpsc::channel();
    let (completed_tx, completed_rx) = tokio::sync::mpsc::unbounded_channel();

    // Spawn analyser thread
    std::thread::spawn(move || {
        let pool = ThreadPool::new(num_cpus::get().saturating_sub(2).max(1));

        while let Ok(demo_path) = request_rx.recv() {
            tracing::debug!("Received request to analyse {demo_path:?}");
            let tx = completed_tx.clone();
            pool.execute(move || {
                tracing::debug!("Analysing {demo_path:?}");
                // Load and analyse demo
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

                // Cache analysed demo on disk
                let _ = payload.as_ref().and_then(|(hash, demo)| {
                    cache_analysed_demo(hash, demo)
                        .map_err(|e| tracing::error!("Error caching analysed demo: {e}"))
                        .ok()
                });

                tracing::debug!("Finished analysing {demo_path:?}");
                tx.send((demo_path, payload)).ok();
            });
        }
    });

    (request_tx, completed_rx)
}

#[derive(Debug, Error)]
enum CachedDemoError {
    #[error("IO: {0}")]
    Io(#[from] std::io::Error),
    #[error("Config file: {0}")]
    Config(#[from] ConfigFilesError),
    #[error("Rmp: {0}")]
    RmpEnc(#[from] rmp_serde::encode::Error),
    #[error("Rmp: {0}")]
    RmpDec(#[from] rmp_serde::decode::Error),
}

fn cache_analysed_demo(hash: &AnalysedDemoID, demo: &AnalysedDemo) -> Result<(), CachedDemoError> {
    let dir = tf2_monitor_core::settings::Settings::locate_config_directory(APP)?;
    let dir = dir.join("analysed_demos");

    if !dir.try_exists()? {
        std::fs::create_dir_all(&dir)?;
    }

    let bytes = rmp_serde::to_vec(demo)?;

    let file_path = dir.join(format!("{hash:x}.bin"));
    std::fs::write(file_path, bytes)?;

    Ok(())
}

async fn read_cached_demo(
    hash: AnalysedDemoID,
) -> Result<(AnalysedDemoID, Box<AnalysedDemo>), CachedDemoError> {
    let dir = tf2_monitor_core::settings::Settings::locate_config_directory(APP)?;
    let dir = dir.join("analysed_demos");
    let file_path = dir.join(format!("{hash:x}.bin"));

    let bytes = tokio::fs::read(file_path).await?;
    let demo = rmp_serde::from_slice(&bytes)?;

    Ok((hash, Box::new(demo)))
}

impl Filters {
    #[must_use]
    pub fn new() -> Self {
        Self {
            sort_by: SortBy::FileCreated,
            direction: SortDirection::Descending,
            show_analysed: true,
            show_non_analysed: true,
            contains_players: Vec::new(),
            search: String::new(),
        }
    }

    pub fn filter(&self, state: &App) -> Vec<usize> {
        let player_steamids: Vec<Option<SteamID>> = state
            .settings
            .demo_filters
            .contains_players
            .iter()
            .map(|s| SteamID::try_from(s.as_str()).ok())
            .collect();

        let mut demos: Vec<(usize, &Demo)> = state
            .demos
            .demo_files
            .iter()
            .enumerate()
            // Filter analysed / non-analysed
            .filter(|(_, d)| {
                self.show_analysed || !state.demos.analysed_demos.contains_key(&d.analysed)
            })
            .filter(|(_, d)| {
                self.show_non_analysed || state.demos.analysed_demos.contains_key(&d.analysed)
            })
            // Search bar
            .filter(|(_, d)| {
                if self.search.trim().is_empty() {
                    return true;
                }

                let analysed = state.demos.analysed_demos.get(&d.analysed);

                for term in self.search.split_whitespace() {
                    let lower_term = term.to_lowercase();

                    // Map
                    if analysed.is_some_and(|a| a.header.map.to_lowercase().contains(&lower_term)) {
                        continue;
                    }

                    // Server name
                    if analysed.is_some_and(|a| a.server_name.to_lowercase().contains(&lower_term))
                    {
                        continue;
                    }

                    // Server IP
                    if analysed.is_some_and(|a| a.header.server.contains(term)) {
                        continue;
                    }

                    // File name
                    if d.name.to_lowercase().contains(&lower_term) {
                        continue;
                    }

                    return false;
                }

                true
            })
            // Filter players
            .filter(|(_, d)| {
                let players = &state.settings.demo_filters.contains_players;
                if players.is_empty() || (players.len() == 1 && players[0].trim().is_empty()) {
                    return true;
                }

                // Can't check players in demos that aren't analysed
                let Some(analysed) = state.demos.analysed_demos.get(&d.analysed) else {
                    return false;
                };

                'outer: for (i, searched_player) in players.iter().enumerate() {
                    let searched_lower = searched_player.to_lowercase();
                    for (s, p) in &analysed.players {
                        // SteamID - Ensure player_steamids is the same length as players
                        if player_steamids
                            .get(i)
                            .and_then(Option::as_ref)
                            .is_some_and(|s2| s == s2)
                        {
                            continue 'outer;
                        }

                        // Name in demo
                        if p.name.to_lowercase().contains(&searched_lower) {
                            continue 'outer;
                        }

                        // Steam name
                        if state.mac.players.steam_info.get(s).is_some_and(|si| {
                            si.account_name.to_lowercase().contains(&searched_lower)
                        }) {
                            continue 'outer;
                        }

                        // Previous names
                        if state.mac.players.records.get(s).is_some_and(|r| {
                            r.previous_names()
                                .iter()
                                .any(|pn| pn.to_lowercase().contains(&searched_lower))
                        }) {
                            continue 'outer;
                        }
                    }
                    return false;
                }

                true
            })
            .collect();

        state.settings.demo_filters.sort_by.sort(&mut demos, state);
        let mut demos: Vec<usize> = demos.into_iter().map(|(i, _)| i).collect();
        state.settings.demo_filters.direction.sort(&mut demos);

        demos
    }
}

impl Default for Filters {
    fn default() -> Self {
        Self::new()
    }
}

impl SortBy {
    pub fn sort(&self, demos: &mut [(usize, &Demo)], state: &App) {
        match self {
            Self::FileName => {
                demos.sort_by_key(|(_, d)| d.name.as_str());
            }
            Self::FileSize => {
                demos.sort_by_key(|(_, d)| d.file_size);
            }
            Self::FileCreated => {
                demos.sort_by_key(|(_, d)| d.created);
            }
            Self::DemoDuration => todo!(),
            Self::NumKills => todo!(),
            Self::NumDeaths => todo!(),
            Self::NumAssists => todo!(),
            Self::NumPlayers => todo!(),
            Self::Map => todo!(),
            Self::ServerName => todo!(),
        }
    }
}

impl SortDirection {
    pub fn sort(&self, demos: &mut [usize]) {
        if *self == Self::Descending {
            demos.reverse();
        }
    }
}
