use std::{
    collections::HashMap,
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};

use bitbuffer::{BitError, BitRead};
use serde::{Deserialize, Serialize};
use steamid_ng::SteamID;
use tf_demo_parser::{
    demo::{
        data::{DemoTick, ServerTick},
        gamevent::GameEvent,
        header::Header,
        message::{gameevent::GameEventMessage, Message},
        packet::{message::MessagePacket, Packet},
        parser::{
            analyser::{Class, Team},
            gamestateanalyser::GameStateAnalyser,
            DemoHandler, RawPacketStream,
        },
    },
    Demo, ParseError,
};
use tokio::io::AsyncReadExt;

pub mod progress;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalysedDemo {
    pub user: SteamID,
    pub header: Header,
    pub server_name: String,
    pub demo_version: u16,
    pub interval_per_tick: f32,
    pub players: HashMap<SteamID, DemoPlayer>,
    pub kills: Vec<Death>,
    pub events: Vec<(DemoTick, Event)>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Event {
    Death(Death),
    Chat(ChatMessage),
    PlayerJoin(SteamID),
    PlayerLeave(SteamID),
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DemoPlayer {
    pub name: String,
    pub kills: Vec<usize>,
    pub assists: Vec<usize>,
    pub deaths: Vec<usize>,
    pub most_played_classes: Vec<Class>,
    pub highest_killstreak: Option<(u32, Class)>,
    /// Sequence of which classes the player was playing as, and for how many ticks.
    /// To find what class they were at a given tick, iterate and sum the number of
    /// ticks until it is greater than the tick being checked, and that will be the class.
    pub ticks_on_classes: Vec<ClassPeriod>,

    /// Information about a player's experience as each class during a match.
    /// Indexed by `tf_demo_parser::demo::parser::analyser::Class`
    pub class_details: [ClassDetails; 10],
    /// Number of seconds, indexed by `tf_demo_parser::demo::parser::analyser::Team`
    pub time_on_team: [u32; 4],
    /// Sequence of which team the player was on, and for how many ticks.
    /// To find what team they were on at a given tick, iterate and sum the number of
    /// ticks until it is greater than the tick being checked, and that will be the team.
    pub ticks_on_teams: Vec<TeamPeriod>,
    pub time: u32,
    pub average_ping: u64,
    pub first_tick: u32,
    pub last_tick: u32,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct TeamPeriod {
    pub team: Team,
    /// Starting tick
    pub start: u32,
    /// How many ticks
    pub duration: u32,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct ClassPeriod {
    pub class: Class,
    /// Starting tick
    pub start: u32,
    /// How many ticks
    pub duration: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ClassDetails {
    /// Number of seconds spent on this class
    pub time: u32,
    pub num_kills: u32,
    pub num_assists: u32,
    pub num_deaths: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Death {
    pub tick: DemoTick,
    pub attacker: Option<SteamID>,
    pub assister: Option<SteamID>,
    pub victim: SteamID,
    pub weapon: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub tick: DemoTick,
    pub from: SteamID,
    pub text: String,
    pub team_only: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("BitError({0})")]
    BitError(#[from] BitError),
    #[error("ParseError({0})")]
    ParseError(#[from] ParseError),
}

impl DemoPlayer {
    #[must_use]
    pub fn class_during_tick(&self, tick: u32) -> Option<Class> {
        for p in &self.ticks_on_classes {
            if tick >= p.start && tick - p.start <= p.duration {
                return Some(p.class);
            }
        }
        None
    }

    #[must_use]
    pub fn team_during_tick(&self, tick: u32) -> Option<Team> {
        for p in &self.ticks_on_teams {
            if tick >= p.start && tick - p.start <= p.duration {
                return Some(p.team);
            }
        }
        None
    }
}

impl AnalysedDemo {
    /// Takes in a slice of bytes making up a demo and attempts to extract some useful information from it.
    /// Extracted information includes:
    /// * Demo header
    /// * Players
    ///   * `SteamID`
    ///   * Kills / Assists / Deaths
    ///   * Most played classes
    ///   * Amount of kills / assists / deaths and time spent on each class
    ///   * Average ping
    ///
    /// A `progress` field is only for if you would like to be able to check on the progress of
    /// demo analysis, and can safely be given `None` otherwise.
    ///
    /// # Errors
    /// If the demo failed to parse for some reason
    #[allow(clippy::too_many_lines)]
    pub fn new(demo_bytes: &[u8], mut progress: Option<progress::Updater>) -> Result<Self, Error> {
        let demo = Demo::new(demo_bytes);
        let mut stream = demo.get_stream();

        let header = Header::read(&mut stream)?;

        let mut analysed_demo = Self {
            user: SteamID::from(0u64),
            header,
            server_name: String::new(),
            demo_version: 0,
            interval_per_tick: 0.0,
            players: HashMap::new(),
            kills: Vec::new(),
            events: Vec::new(),
        };

        // Total number of bits in the demo
        #[allow(clippy::cast_precision_loss)]
        let progress_total = (demo_bytes.len() * 8) as f32;
        // Number of bits processed at the time of the last progress update
        let mut last_progress_update = 0;
        // Number of bits to process between progress updates
        #[allow(clippy::items_after_statements)]
        const PROGRESS_INTERVAL: usize = 100_000;

        // Do the gameplay analysis

        let mut handler = DemoHandler::with_analyser(GameStateAnalyser::new());

        let mut packets = RawPacketStream::new(stream);
        let mut initial_server_tick = ServerTick::from(0u32);
        let mut last_tick = ServerTick::from(0u32);
        let mut num_ticks_checked = 0u64;
        let mut last_kills_len = 0;
        while let Some(packet) = packets.next(&handler.state_handler)? {
            let mut newly_connected: Option<(String, u16)> = None;

            // Custom packet handling
            // TODO
            // Chat
            // Player join
            // Player leave
            // Killstreak? Can I be bothered?
            #[allow(clippy::single_match)]
            match &packet {
                Packet::Signon(MessagePacket { messages, .. }) => {
                    for m in messages {
                        match m {
                            Message::ServerInfo(server_info) => {
                                analysed_demo
                                    .server_name
                                    .clone_from(&server_info.server_name);
                            }
                            _ => {}
                        }
                    }
                }
                Packet::Message(MessagePacket { messages, .. }) => {
                    for m in messages {
                        match m {
                            // Player join
                            Message::GameEvent(GameEventMessage {
                                event: GameEvent::PlayerConnectClient(client_connect),
                                ..
                            }) => {
                                newly_connected =
                                    Some((client_connect.name.to_string(), client_connect.user_id));
                            }
                            _ => {}
                        }
                    }
                }
                _ => {}
            }

            handler.handle_packet(packet)?;

            if let Some((name, userid)) = newly_connected {
                if let Some(info) = handler
                    .borrow_output()
                    .players
                    .iter()
                    .filter_map(|p| p.info.as_ref())
                    .find(|i| i.user_id == userid)
                {
                    if let Some(player) = SteamID::try_from(info.steam_id.as_str())
                        .ok()
                        .map(|s| analysed_demo.players.entry(s).or_default())
                    {
                        player.name = name;
                    }
                }
            }

            // Game state handling
            if handler.server_tick == last_tick {
                continue;
            }

            // Update progress
            let current_progress_bytes = packets.pos();
            if current_progress_bytes - last_progress_update >= PROGRESS_INTERVAL {
                last_progress_update = current_progress_bytes;
                if let Some(updater) = &mut progress {
                    #[allow(clippy::cast_precision_loss)]
                    updater.update_progress(progress::Progress::InProgress(
                        last_progress_update as f32 / progress_total,
                    ));
                }
            }

            let tick_delta = if last_tick == 0 {
                initial_server_tick = handler.server_tick;
                ServerTick::from(0)
            } else {
                handler.server_tick - last_tick
            };
            last_tick = handler.server_tick;
            let current_tick = last_tick - initial_server_tick;
            num_ticks_checked += 1;

            let game_state = handler.borrow_output();

            let get_player_from_userid = |userid: u16| {
                game_state
                    .players
                    .iter()
                    .filter_map(|p| p.info.as_ref().map(|ui| (p, ui)))
                    .find(|(_, ui)| ui.user_id == userid)
                    .and_then(|(p, ui)| {
                        SteamID::try_from(ui.steam_id.as_str()).ok().map(|s| (p, s))
                    })
            };

            // Get player names
            for (s, ui) in handler
                .borrow_output()
                .players
                .iter()
                .filter_map(|p| p.info.as_ref())
                .filter_map(|ui| {
                    SteamID::try_from(ui.steam_id.as_str())
                        .ok()
                        .map(|s| (s, ui))
                })
            {
                let Some(p) = analysed_demo.players.get_mut(&s) else {
                    continue;
                };

                if !p.name.is_empty() {
                    continue;
                }
                p.name.clone_from(&ui.name);
            }

            // Update player stats
            for (p, info) in game_state
                .players
                .iter()
                .filter_map(|p| p.info.as_ref().map(|ui| (p, ui)))
            {
                let Ok(steamid) = SteamID::try_from(info.steam_id.as_str()) else {
                    continue;
                };

                // Add player if they don't exist
                let player = analysed_demo.players.entry(steamid).or_default();

                if player.first_tick == 0 {
                    player.first_tick = u32::from(current_tick);
                }
                player.last_tick = u32::from(current_tick);

                // Update class and team info
                player.class_details[p.class as usize].time += u32::from(tick_delta);
                player.time_on_team[p.team as usize] += u32::from(tick_delta);
                player.time += u32::from(tick_delta);

                match player.ticks_on_teams.last_mut() {
                    Some(period) if period.team == p.team => {
                        period.duration += u32::from(tick_delta);
                    }
                    _ => {
                        player.ticks_on_teams.push(TeamPeriod {
                            team: p.team,
                            start: u32::from(current_tick),
                            duration: 0,
                        });
                    }
                }
                match player.ticks_on_classes.last_mut() {
                    Some(period) if period.class == p.class => {
                        period.duration += u32::from(tick_delta);
                    }
                    _ => {
                        player.ticks_on_classes.push(ClassPeriod {
                            class: p.class,
                            start: u32::from(current_tick),
                            duration: 0,
                        });
                    }
                }

                // Add ping
                player.average_ping += u64::from(p.ping);
            }

            // Kills
            if last_kills_len < game_state.kills.len() {
                for k in game_state.kills.iter().skip(last_kills_len) {
                    let Some((victim, victim_steamid)) = get_player_from_userid(k.victim_id) else {
                        continue;
                    };

                    let attacker = get_player_from_userid(k.attacker_id);
                    let assister = get_player_from_userid(k.assister_id);

                    let death = Death {
                        tick: k.tick,
                        attacker: attacker.as_ref().map(|(_, s)| *s),
                        assister: assister.as_ref().map(|(_, s)| *s),
                        victim: victim_steamid,
                        weapon: k.weapon.clone(),
                    };
                    let death_idx = analysed_demo.kills.len();
                    analysed_demo.kills.push(death);

                    // Victim
                    let victim_entry = analysed_demo.players.entry(victim_steamid).or_default();
                    victim_entry.deaths.push(death_idx);
                    victim_entry.class_details[victim.class as usize].num_deaths += 1;

                    // Attacker
                    if let Some((attacker, attacker_steamid)) = attacker {
                        let attacker_entry =
                            analysed_demo.players.entry(attacker_steamid).or_default();
                        attacker_entry.kills.push(death_idx);
                        attacker_entry.class_details[attacker.class as usize].num_kills += 1;
                    }

                    // Assister
                    if let Some((assister, assister_steamid)) = assister {
                        let assister_entry =
                            analysed_demo.players.entry(assister_steamid).or_default();
                        assister_entry.assists.push(death_idx);
                        assister_entry.class_details[assister.class as usize].num_assists += 1;
                    }
                }

                last_kills_len = game_state.kills.len();
            }
        }

        // Most played classes
        for p in analysed_demo.players.values_mut() {
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

            let mut most_played_classes: Vec<_> = CLASSES
                .iter()
                .map(|c| (c, &p.class_details[*c as usize]))
                .filter(|(_, d)| d.time > 0)
                .collect();
            most_played_classes.sort_by_key(|(_, d)| d.time);
            most_played_classes.reverse();

            p.most_played_classes = most_played_classes.iter().map(|(&c, _)| c).collect();
        }

        // Ping
        analysed_demo
            .players
            .values_mut()
            .for_each(|p| p.average_ping /= num_ticks_checked);

        // User
        if let Some(steamid) = handler
            .borrow_output()
            .players
            .iter()
            .filter_map(|p| p.info.as_ref())
            .find(|ui| ui.name == analysed_demo.header.nick)
            .and_then(|ui| SteamID::try_from(ui.steam_id.as_str()).ok())
        {
            analysed_demo.user = steamid;
        }

        // Metadata
        let meta = &handler.get_parser_state().demo_meta;
        analysed_demo.demo_version = meta.version;
        analysed_demo.interval_per_tick = meta.interval_per_tick;

        // Scale time
        #[allow(
            clippy::cast_sign_loss,
            clippy::cast_possible_truncation,
            clippy::cast_precision_loss
        )]
        analysed_demo.players.values_mut().for_each(|p| {
            p.class_details.iter_mut().for_each(|d| {
                d.time = (d.time as f32 * analysed_demo.interval_per_tick) as u32;
            });
            p.time_on_team.iter_mut().for_each(|t| {
                *t = (*t as f32 * analysed_demo.interval_per_tick) as u32;
            });
            p.time = (p.time as f32 * analysed_demo.interval_per_tick) as u32;
        });

        // Update progress
        if let Some(updater) = &mut progress {
            updater.update_progress(progress::Progress::Finished);
        }

        Ok(analysed_demo)
    }
}

/// Takes a hash of the header and created time of a demo file
///
/// # Errors
/// If the created time or header bytes could not be read from the provided file
#[allow(clippy::future_not_send)]
pub async fn hash_demo_file(demo_file: impl AsRef<Path>) -> Result<md5::Digest, std::io::Error> {
    let mut demo_file = tokio::fs::File::open(demo_file).await?;
    let demo_meta = demo_file.metadata().await?;
    let created = demo_meta.created()?;
    let mut header_bytes = [0u8; 0x430];
    let _ = demo_file.read_exact(&mut header_bytes).await?;

    Ok(hash_demo(&header_bytes, created))
}

/// Takes a hash of the header and created time of a demo
#[must_use]
pub fn hash_demo(demo_bytes: &[u8], created: SystemTime) -> md5::Digest {
    let time = created
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
        .to_le_bytes();

    let mut ctx = md5::Context::new();
    ctx.consume(&demo_bytes[0..demo_bytes.len().min(0x430)]);
    ctx.consume(time);
    ctx.compute()
}
