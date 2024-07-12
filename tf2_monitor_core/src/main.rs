use std::{
    path::{Path, PathBuf},
    str::FromStr,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};

use args::Args;
use clap::Parser;
use event_loop::{define_events, EventLoop};
use events::emit_on_timer;
use launchoptions::LaunchOptions;
use player::Players;
use player_records::PlayerRecords;
use reqwest::StatusCode;
use server::Server;
use settings::{AppDetails, Settings};
use state::MonitorState;
use steamid_ng::SteamID;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::{
    filter::Directive, fmt::writer::MakeWriterExt, layer::SubscriberExt, util::SubscriberInitExt,
    EnvFilter, Layer,
};
use web::{web_main, WebState};

mod args;
mod command_manager;
mod console;
mod demo;
mod events;
mod gamefinder;
mod io;
mod launchoptions;
mod masterbase;
mod new_players;
mod parties;
mod player;
mod player_records;
mod server;
mod settings;
mod sse_events;
mod state;
mod steam_api;
mod web;

use command_manager::{Command, CommandManager, DumbAutoKick};
use console::{ConsoleLog, ConsoleOutput, ConsoleParser, RawConsoleOutput};
use demo::{DemoBytes, DemoManager, DemoMessage, DemoWatcher};
use events::{Preferences, Refresh, UserUpdates};
use new_players::{ExtractNewPlayers, NewPlayers};
use sse_events::SseEventBroadcaster;
use steam_api::{
    FriendLookupResult, LookupFriends, LookupProfiles, ProfileLookupBatchTick,
    ProfileLookupRequest, ProfileLookupResult,
};
use web::{WebAPIHandler, WebRequest};

pub const APP: AppDetails<'static> = AppDetails {
    qualifier: "com.megascatterbomb",
    organization: "MAC",
    application: "MACClient",
};

define_events!(
    MonitorState,
    Message {
        Refresh,

        Command,

        RawConsoleOutput,
        ConsoleOutput,

        NewPlayers,

        ProfileLookupBatchTick,
        ProfileLookupResult,
        FriendLookupResult,
        ProfileLookupRequest,

        Preferences,
        UserUpdates,

        WebRequest,

        DemoBytes,
        DemoMessage,
    },
    Handler {
        CommandManager,
        ConsoleParser,
        ExtractNewPlayers,

        LookupProfiles,
        LookupFriends,

        WebAPIHandler,
        SseEventBroadcaster,

        DemoManager,
        DumbAutoKick,
    },
);

#[allow(clippy::too_many_lines, clippy::cognitive_complexity)]
fn main() {
    let _guard = init_tracing();

    let args = Args::parse();

    let mut settings = Settings::load_or_create(
        Settings::default_file_location(APP).unwrap_or_else(|e| {
            tracing::error!("Failed to find a suitable location to store settings ({e}). Settings will be written to {}", settings::CONFIG_FILE_NAME);
            settings::CONFIG_FILE_NAME.into()
        }
    )).expect("Failed to load settings. Please fix any issues mentioned and try again.");
    settings.save_ok();

    // Resolve steam user
    match args
        .steam_user
        .as_ref()
        .map(|s| SteamID::try_from(s.as_str()))
    {
        Some(Ok(user)) => {
            settings.steam_user = Some(user);
            tracing::info!("Steam user set to {}", u64::from(user));
        }
        Some(Err(e)) => {
            tracing::error!("Failed to parse SteamID ({e})");
            panic!("Please provide a valid SteamID");
        }
        None => match settings.infer_steam_user() {
            Ok(user) => {
                tracing::info!("Identified current steam user as {}", u64::from(user));
                settings.steam_user = Some(user);
                check_launch_options(&settings);
            }
            Err(e) => tracing::error!("Failed to identify current steam user: {e}"),
        },
    }

    // Resolve TF2 directory
    if let Some(tf2_dir) = args.tf2_dir {
        tracing::debug!("Set TF2 directory to {tf2_dir}");
        settings.tf2_directory = Some(tf2_dir.into());
    } else {
        match settings.infer_tf2_directory() {
            Ok(tf2_dir) => {
                tracing::debug!("Identified TF2 directory as {tf2_dir:?}");
                settings.tf2_directory = Some(tf2_dir.into());
            }
            Err(e) => {
                tracing::error!(
                    "Please provide a valid TF2 directory with \"--tf2-dir path_to_tf2_folder\""
                );
                panic!("Failed to locate TF2 directory ({e})");
            }
        }
    }
    let tf2_directory = settings
        .tf2_directory
        .clone()
        .expect("A valid TF2 directory must be set.");

    let mut playerlist = PlayerRecords::load_or_create(PlayerRecords::default_file_location(APP).unwrap_or_else(|e| {
        tracing::error!("Failed to find a suitable location to store player records ({e}). Records will be written to {}", player_records::RECORDS_FILE_NAME);
        player_records::RECORDS_FILE_NAME.into()
    })).expect("Failed to load player records. Please fix any issues mentioned and try again.");
    playerlist.save_ok();

    let players = Players::new(
        playerlist,
        settings.steam_user,
        Players::default_steam_cache_path(APP).ok(),
    );

    let mut state = MonitorState {
        server: Server::new(),
        settings,
        players,
    };

    let web_port = state.settings.webui_port;

    // The juicy part of the program
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("Failed to build async runtime")
        .block_on(async {
            if state.settings.masterbase_key.is_empty() {
                state.settings.upload_demos = false;
                tracing::warn!("No masterbase key is set. If you would like to enable demo uploads, please provision a key at https://megaanticheat.com/provision");
            }

            // Close any previous masterbase sessions that might not have finished up
            // properly.
            if state.settings.upload_demos {
                const TIMEOUT: u64 = 4;
                match tokio::time::timeout(Duration::from_secs(TIMEOUT), async { masterbase::force_close_session(
                    &state.settings.masterbase_host,
                    &state.settings.masterbase_key,
                    state.settings.masterbase_http,
                ).await})
                .await
                {
                    // Successfully closed existing session
                    Ok(Ok(r)) if r.status().is_success() => tracing::warn!(
                        "User was previously in a Masterbase session that has now been closed."
                    ),
                    // Server error
                    Ok(Ok(r)) if r.status().is_server_error() => tracing::error!(
                        "Server error when trying to close previous Masterbase sessions: Status code {}",
                        r.status()
                    ),
                    // Not authorized, invalid key
                    Ok(Ok(r)) if r.status() == StatusCode::UNAUTHORIZED => {
                        tracing::warn!("Your Masterbase key is not valid, demo uploads will be disabled. Please provision a new one at https://megaanticheat.com/provision");
                        state.settings.upload_demos = false;
                    }
                    // Forbidden, no session was open
                    Ok(Ok(r)) if r.status() == StatusCode::FORBIDDEN => {
                        tracing::info!("Successfully authenticated with the Masterbase.");
                    }
                    // Remaining responses will be client failures
                    Ok(Ok(r)) => tracing::info!("Client error when trying to contact masterbase: Status code {}", r.status()),
                    Ok(Err(e)) => tracing::error!("Couldn't reach Masterbase: {e}"),
                    Err(_) => {
                        tracing::error!("Connection to masterbase timed out after {TIMEOUT} seconds");
                    }
                }
            }

            // Exit handler
            let running = Arc::new(AtomicBool::new(true));
            let r = running.clone();
            tokio::task::spawn(async move {
                if let Err(e) = tokio::signal::ctrl_c().await {
                    tracing::error!("Error with Ctrl+C handler: {e}");
                }
                r.store(false, Ordering::SeqCst);
            });

            // Demo watcher and manager
            let demo_path = tf2_directory.join("tf");
            let demo_watcher = if args.dont_parse_demos { None } else { DemoWatcher::new(&demo_path)
                .map_err(|e| {
                    tracing::error!("Could not initialise demo watcher: {e}");
                })
                .ok()};

            // Web API
            let (web_state, web_requests) = WebState::new(&state.settings.web_ui_source);
            tokio::task::spawn(async move {
                web_main(web_state, web_port).await;
            });

            // Autolaunch UI
            if state.settings.autolaunch_ui {
                if let Err(e) = open::that(Path::new(&format!("http://localhost:{web_port}"))) {
                    tracing::error!("Failed to open web browser: {:?}", e);
                }
            }

            // Watch console log
            let log_file_path: PathBuf =
                tf2_directory.join("tf/console.log");
            let console_log = Box::new(ConsoleLog::new(log_file_path).await);

            let mut event_loop: EventLoop<MonitorState, Message, Handler> = EventLoop::new()
                .add_source(console_log)
                .add_source(emit_on_timer(Duration::from_secs(3), || Refresh).await)
                .add_source(emit_on_timer(Duration::from_millis(500), || ProfileLookupBatchTick).await)
                .add_source(Box::new(web_requests))
                .add_handler(DemoManager::new())
                .add_handler(CommandManager::new())
                .add_handler(ConsoleParser::default())
                .add_handler(ExtractNewPlayers)
                .add_handler(LookupProfiles::new())
                .add_handler(LookupFriends::new())
                .add_handler(DumbAutoKick)
                .add_handler(WebAPIHandler::new())
                .add_handler(SseEventBroadcaster::new());

            if args.dont_parse_demos {
                tracing::info!("Demo parsing has been disabled. This also prevents uploading demos to the masterbase.");
            } else if let Some(dw) = demo_watcher {
                event_loop = event_loop.add_source(Box::new(dw));
            }

            loop {
                if !running.load(Ordering::SeqCst) {
                    tracing::info!("Saving and exiting.");
                    state.players.records.save_ok();
                    state.settings.save_ok();
                    state.players.save_steam_info_ok();
                    std::process::exit(0);
                }

                if event_loop.execute_cycle(&mut state).await.is_none() {
                    tokio::time::sleep(Duration::from_millis(50)).await;
                }
            }
        });
}

#[allow(clippy::cognitive_complexity)]
fn check_launch_options(settings: &Settings) {
    // Launch options and overrides
    let launch_opts = match LaunchOptions::new(
        settings
            .steam_user
            .expect("Failed to identify the local steam user (failed to find `loginusers.vdf`)"),
    ) {
        Ok(val) => Some(val),
        Err(why) => {
            tracing::warn!("Couldn't verify app launch options: {:?}", why);
            None
        }
    };

    if let Some(opts) = launch_opts {
        // Warn about missing launch options for TF2
        match opts.check_missing_args() {
            Ok(missing_opts) if !missing_opts.is_empty() => {
                tracing::warn!(
                    "Please add the following launch options to your TF2 to allow the MAC client to interface correctly with TF2."
                );
                tracing::warn!("Missing launch options: \"{}\"", missing_opts.join(" "));
            }

            Ok(_) => {
                tracing::info!("All required launch arguments are present!");
            }

            Err(missing_opts_err) => {
                tracing::error!(
                    "Failed to verify app launch options: {:?} (App may continue to function normally)",
                    missing_opts_err
                );
            }
        }
    }
}

fn init_tracing() -> Option<WorkerGuard> {
    if std::env::var("RUST_LOG").is_err() {
        std::env::set_var("RUST_LOG", "info");
    }

    let suppress_hyper = Directive::from_str("hyper=warn").expect("Bad directive");
    let suppress_demo_parser = Directive::from_str("tf_demo_parser=warn").expect("Bad directive");
    let subscriber = tracing_subscriber::registry().with(
        tracing_subscriber::fmt::layer()
            .with_writer(std::io::stderr)
            .with_filter(
                EnvFilter::from_default_env()
                    .add_directive(suppress_hyper.clone())
                    .add_directive(suppress_demo_parser.clone()),
            ),
    );

    match std::fs::File::create("./macclient.log") {
        Ok(latest_log) => {
            let (file_writer, guard) = tracing_appender::non_blocking(latest_log);
            subscriber
                .with(
                    tracing_subscriber::fmt::layer()
                        .with_ansi(false)
                        .with_writer(file_writer.with_max_level(tracing::Level::TRACE))
                        .with_filter(
                            EnvFilter::builder()
                                .parse("debug")
                                .expect("Bad env")
                                .add_directive(suppress_hyper)
                                .add_directive(suppress_demo_parser),
                        ),
                )
                .init();
            Some(guard)
        }
        Err(e) => {
            subscriber.init();
            tracing::error!(
                "Failed to create log file, continuing without persistent logs: {}",
                e
            );
            None
        }
    }
}
