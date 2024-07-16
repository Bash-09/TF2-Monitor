use std::{
    borrow::Borrow,
    collections::HashMap,
    hash::{DefaultHasher, Hash, Hasher},
};

use serde::{Deserialize, Serialize};
use tf2_monitor_core::{
    bitbuffer::{BitError, BitRead},
    steamid_ng::SteamID,
    tf_demo_parser::{
        demo::{
            data::DemoTick,
            header::Header,
            parser::{
                analyser::Class, gamestateanalyser::GameStateAnalyser, DemoHandler, RawPacketStream,
            },
        },
        Demo, ParseError,
    },
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexedDemo {
    pub hash: u64,
    pub header: Header,
    pub players: HashMap<SteamID, DemoPlayer>,
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
    pub kills: Vec<Death>,
    pub assists: Vec<Death>,
    pub deaths: Vec<Death>,
    pub most_played_classes: Vec<Class>,
    pub highest_killstreak: Option<(u32, Class)>,

    /// Information about a player's experience as each class during a match.
    /// Indexed by `tf_demo_parser::demo::parser::analyser::Class`
    pub class_details: [ClassDetails; 10],
    /// Number of seconds, indexed by `tf_demo_parser::demo::parser::analyser::Team`
    pub time_on_team: [u32; 4],
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
    pub attacked: SteamID,
    pub assiter: Option<SteamID>,
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

impl IndexedDemo {
    pub fn new(demo_bytes: &[u8]) -> Result<IndexedDemo, Error> {
        let mut hasher = DefaultHasher::new();
        demo_bytes.hash(&mut hasher);
        let hash = hasher.finish();

        let demo = Demo::new(demo_bytes);
        let mut stream = demo.get_stream();

        let header = Header::read(&mut stream)?;

        let mut indexed_demo = IndexedDemo {
            hash,
            header,
            players: HashMap::new(),
            events: Vec::new(),
        };

        // Do the gameplay analysis

        let mut handler = DemoHandler::with_analyser(GameStateAnalyser::new());
        let mut packets = RawPacketStream::new(stream);
        let mut last_tick = DemoTick::from(0u32);
        let mut num_ticks_checked = 0u64;
        let mut last_kills_len = 0;
        while let Some(packet) = packets.next(&handler.state_handler)? {
            // Custom packet handling
            // TODO
            // Chat
            // Player join
            // Player leave
            // Killstreak? Can I be bothered?

            handler.handle_packet(packet)?;

            // Game state handling
            if handler.demo_tick == last_tick {
                continue;
            }
            let tick_delta = handler.demo_tick - last_tick;
            last_tick = handler.demo_tick;
            num_ticks_checked += 1;

            // TODO
            // Update player stats
            for (p, info) in handler
                .borrow_output()
                .players
                .iter()
                .filter_map(|p| p.info.as_ref().map(|ui| (p, ui)))
            {
                // Add player if they don't exist
                // Update class and team info
                // Add ping
            }
        }

        // TODO
        // Kills

        // Ping
        indexed_demo
            .players
            .values_mut()
            .for_each(|p| p.average_ping /= num_ticks_checked);

        Ok(indexed_demo)
    }
}
