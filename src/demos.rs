use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
    io::{ErrorKind, Read},
    path::PathBuf,
    sync::mpsc::Sender,
    time::SystemTime,
};

use tf2_monitor_core::{
    demo_analyser::{self, AnalysedDemo},
    settings::ConfigFilesError,
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

pub type AnalysedDemoID = tf2_monitor_core::md5::Digest;
type AnalysedDemoResult = (PathBuf, Option<(AnalysedDemoID, Box<AnalysedDemo>)>);

#[allow(clippy::module_name_repetitions)]
pub struct DemosState {
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
}

impl From<DemosMessage> for Message {
    fn from(val: DemosMessage) -> Self {
        Self::Demos(val)
    }
}

impl DemosState {
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

    #[allow(clippy::missing_panics_doc)]
    pub fn handle_message(state: &mut App, message: DemosMessage) -> iced::Command<Message> {
        match message {
            DemosMessage::Refresh => return Self::refresh_demos(state),
            DemosMessage::SetPage(page) => state.demos.page = page,
            DemosMessage::SetDemos(demo_files) => {
                state.demos.demo_files = demo_files;
                state.demos.update_demos_to_display();

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

impl Default for DemosState {
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
