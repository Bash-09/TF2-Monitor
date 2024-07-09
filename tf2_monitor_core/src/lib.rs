//! Constructing an event loop with all features enabled would look something like this:
//!
//! ```
//! use event_loop::define_events;
//!
//! use command_manager::{Command, CommandManager, DumbAutoKick};
//! use console::{ConsoleLog, ConsoleOutput, ConsoleParser, RawConsoleOutput};
//! use demo::{DemoBytes, DemoManager, DemoMessage, DemoWatcher, PrintVotes};
//! use events::{Preferences, Refresh, UserUpdates};
//! use new_players::{ExtractNewPlayers, NewPlayers};
//! use sse_events::SseEventBroadcaster;
//! use steam_api::{
//!     FriendLookupResult, LookupFriends, LookupProfiles, ProfileLookupBatchTick,
//!     ProfileLookupRequest, ProfileLookupResult,
//! };
//! use web::{WebAPIHandler, WebRequest};
//!
//! // Among other imports
//!
//! define_events!(
//!     MonitorState,
//!     Message {
//!         Refresh,
//!
//!         Command,
//!
//!         RawConsoleOutput,
//!         ConsoleOutput,
//!
//!         NewPlayers,
//!
//!         ProfileLookupBatchTick,
//!         ProfileLookupResult,
//!         FriendLookupResult,
//!         ProfileLookupRequest,
//!
//!         Preferences,
//!         UserUpdates,
//!
//!         WebRequest,
//!
//!         DemoBytes,
//!         DemoMessage,
//!     },
//!     Handler {
//!         CommandManager,
//!         ConsoleParser,
//!         ExtractNewPlayers,
//!
//!         LookupProfiles,
//!         LookupFriends,
//!
//!         WebAPIHandler,
//!         SseEventBroadcaster,
//!
//!         DemoManager,
//!         PrintVotes,
//!         DumbAutoKick,
//!     },
//! );
//!
//! pub async fn main() {
//!     let args = Args::parse();
//!
//!     let settings = Settings::load_or_create(&args);
//!     let mut playerlist = PlayerRecords::load_or_create(&args);
//!     let players = Players::new(playerlist, settings.steam_user());
//!
//!     let mut state = MonitorState {
//!         server: Server::new(),
//!         settings,
//!         players,
//!     };
//!
//!     // Demo watcher and manager
//!     let demo_path = state.settings.tf2_directory().join("tf");
//!     let demo_watcher = DemoWatcher::new(&demo_path)
//!         .map_err(|e| {
//!             tracing::error!("Could not initialise demo watcher: {e}");
//!         });
//!
//!     // Web API
//!     let (web_state, web_requests) = WebState::new(state.settings.web_ui_source());
//!     tokio::task::spawn(async move {
//!         web_main(web_state, web_port).await;
//!     });
//!
//!     // Watch console log
//!     let log_file_path: PathBuf =
//!         PathBuf::from(state.settings.tf2_directory()).join("tf/console.log");
//!     let console_log = Box::new(ConsoleLog::new(log_file_path).await);
//!
//!     let mut event_loop: EventLoop<MonitorState, Message, Handler> = EventLoop::new()
//!         .add_source(console_log)
//!         .add_source(emit_on_timer(Duration::from_secs(3), || Refresh).await)
//!         .add_source(emit_on_timer(Duration::from_millis(500), || ProfileLookupBatchTick).await)
//!         .add_source(Box::new(web_requests))
//!         .add_handler(DemoManager::new())
//!         .add_handler(CommandManager::new())
//!         .add_handler(ConsoleParser::default())
//!         .add_handler(ExtractNewPlayers)
//!         .add_handler(LookupProfiles::new())
//!         .add_handler(LookupFriends::new())
//!         .add_handler(DumbAutoKick)
//!         .add_handler(PrintVotes::new())
//!         .add_handler(WebAPIHandler::new())
//!         .add_handler(SseEventBroadcaster::new());
//!
//!     if let Ok(dw) = demo_watcher {
//!         event_loop = event_loop.add_source(Box::new(dw));
//!     }
//!
//!     loop {
//!         if event_loop.execute_cycle(&mut state).await.is_none() {
//!             tokio::time::sleep(Duration::from_millis(50)).await;
//!         }
//!     }
//! }
//! ```
//!

pub mod args;
pub mod command_manager;
pub mod console;
pub mod demo;
pub mod events;
pub mod gamefinder;
pub mod io;
pub mod launchoptions;
pub mod masterbase;
pub mod new_players;
pub mod parties;
pub mod player;
pub mod player_records;
pub mod server;
pub mod settings;
pub mod sse_events;
pub mod state;
pub mod steam_api;
pub mod web;

pub use clap;
pub use event_loop;
pub use rcon;
pub use serde_json;
pub use steamid_ng;
