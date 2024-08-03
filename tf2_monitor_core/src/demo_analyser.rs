use std::{
    collections::HashMap,
    hash::{DefaultHasher, Hash, Hasher},
    path::Path,
    time::SystemTime,
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
            analyser::Class, gamestateanalyser::GameStateAnalyser, DemoHandler, RawPacketStream,
        },
    },
    Demo, ParseError,
};
use tokio::io::AsyncReadExt;

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

    /// Information about a player's experience as each class during a match.
    /// Indexed by `tf_demo_parser::demo::parser::analyser::Class`
    pub class_details: [ClassDetails; 10],
    /// Number of seconds, indexed by `tf_demo_parser::demo::parser::analyser::Team`
    pub time_on_team: [u32; 4],
    pub time: u32,
    pub average_ping: u64,
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
    /// # Errors
    /// If the demo failed to parse for some reason
    #[allow(clippy::too_many_lines)]
    pub fn new(demo_bytes: &[u8]) -> Result<Self, Error> {
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

        // Do the gameplay analysis

        let mut handler = DemoHandler::with_analyser(GameStateAnalyser::new());

        let mut packets = RawPacketStream::new(stream);
        let mut last_tick = ServerTick::from(0u32);
        let mut num_ticks_checked = 0u64;
        let mut last_kills_len = 0;
        while let Some(packet) = packets.next(&handler.state_handler)? {
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
                                event: GameEvent::PlayerConnectClient(_client_connect),
                                ..
                            }) => {
                                // TODO
                            }
                            _ => {}
                        }
                    }
                }
                _ => {}
            }

            handler.handle_packet(packet)?;

            // Game state handling
            if handler.server_tick == last_tick {
                continue;
            }

            let tick_delta = if last_tick == 0 {
                ServerTick::from(0)
            } else {
                handler.server_tick - last_tick
            };
            last_tick = handler.server_tick;
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

                // Update class and team info
                player.class_details[p.class as usize].time += u32::from(tick_delta);
                player.time_on_team[p.team as usize] += u32::from(tick_delta);
                player.time += u32::from(tick_delta);

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

        // Name
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

            p.name.clone_from(&ui.name);
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
        analysed_demo.players.values_mut().for_each(|p| {
            p.class_details.iter_mut().for_each(|d| {
                d.time = (d.time as f32 * analysed_demo.interval_per_tick) as u32;
            });
            p.time_on_team.iter_mut().for_each(|t| {
                *t = (*t as f32 * analysed_demo.interval_per_tick) as u32;
            });
            p.time = (p.time as f32 * analysed_demo.interval_per_tick) as u32;
        });

        Ok(analysed_demo)
    }
}

/// Takes a hash of the header and created time of a demo file
///
/// # Errors
/// If the created time or header bytes could not be read from the provided file
#[allow(clippy::future_not_send)]
pub async fn hash_demo_file(demo_file: impl AsRef<Path>) -> Result<u64, std::io::Error> {
    let mut demo_file = tokio::fs::File::open(demo_file).await?;
    let demo_meta = demo_file.metadata().await?;
    let created = demo_meta.created()?;
    let mut header_bytes = [0u8; 0x430];
    let _ = demo_file.read_exact(&mut header_bytes).await?;

    Ok(hash_demo(&header_bytes, created))
}

/// Takes a hash of the header and created time of a demo
#[must_use]
pub fn hash_demo(demo_bytes: &[u8], created: SystemTime) -> u64 {
    let mut hasher = DefaultHasher::new();
    created.hash(&mut hasher);
    demo_bytes[..demo_bytes.len().min(0x430)].hash(&mut hasher);
    hasher.finish()
}
