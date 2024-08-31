use serde::Serialize;
use steamid_ng::SteamID;
use tf_demo_parser::demo::gameevent_gen::{VoteCastEvent, VoteOptionsEvent};

use crate::{
    console::ConsoleOutput,
    demo::{DemoEvent, DemoMessage},
    io::regexes::{self, ChatMessage, PlayerKill},
    player::Players,
};

// Server

pub struct Server {
    map: Option<String>,
    ip: Option<String>,
    hostname: Option<String>,
    max_players: Option<u32>,
    num_players: Option<u32>,
    gamemode: Option<Gamemode>,
    chat_history: Vec<ChatMessage>,
    kill_history: Vec<PlayerKill>,
    vote_history: Vec<VoteEvent>,
    /// (`vote_idx`, `CastVote`)
    shunted_vote_cast_events: Vec<(u32, CastVote)>,
}

#[derive(Debug, Serialize, Clone)]
pub struct Gamemode {
    pub matchmaking: bool,
    #[serde(rename = "type")]
    pub game_type: String,
    pub vanilla: bool,
}

#[derive(Debug, Clone)]
pub struct VoteEvent {
    pub idx: u32,
    pub options: Vec<String>,
    pub votes: Vec<CastVote>,
}

#[derive(Debug, Clone)]
pub struct CastVote {
    pub steamid: Option<SteamID>,
    pub option: u8,
}

#[allow(dead_code)]
impl Server {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            map: None,
            ip: None,
            hostname: None,
            max_players: None,
            num_players: None,

            gamemode: None,

            chat_history: Vec::new(),
            kill_history: Vec::new(),
            vote_history: Vec::new(),
            shunted_vote_cast_events: Vec::new(),
        }
    }

    // **** Getters / Setters ****

    #[must_use]
    pub fn map(&self) -> Option<&str> {
        self.map.as_deref()
    }

    #[must_use]
    pub fn ip(&self) -> Option<&str> {
        self.ip.as_deref()
    }

    #[must_use]
    pub fn hostname(&self) -> Option<&str> {
        self.hostname.as_deref()
    }

    #[must_use]
    pub const fn max_players(&self) -> Option<u32> {
        self.max_players
    }

    #[must_use]
    pub const fn num_players(&self) -> Option<u32> {
        self.num_players
    }

    #[must_use]
    pub const fn gamemode(&self) -> Option<&Gamemode> {
        self.gamemode.as_ref()
    }

    #[must_use]
    pub fn chat_history(&self) -> &[ChatMessage] {
        &self.chat_history
    }

    #[must_use]
    pub fn kill_history(&self) -> &[PlayerKill] {
        &self.kill_history
    }

    #[must_use]
    pub fn vote_history(&self) -> &[VoteEvent] {
        &self.vote_history
    }
}

impl Default for Server {
    fn default() -> Self {
        Self::new()
    }
}

impl Server {
    // **** Message handling ****

    /// Handles any io output from running commands / reading the console log
    /// file. Returns:
    /// * Some<`SteamID`> of a player if they have been newly added to the
    ///   server.
    pub fn handle_console_output(&mut self, response: ConsoleOutput) {
        use ConsoleOutput::{
            Chat, DemoStop, Hostname, Kill, Map, PlayerCount, ServerIP, Status, G15,
        };
        match response {
            Chat(chat) => self.handle_chat(chat),
            Kill(kill) => self.handle_kill(kill),
            Hostname(regexes::Hostname(hostname)) => {
                self.hostname = Some(hostname);
            }
            ServerIP(regexes::ServerIP(ip)) => {
                self.ip = Some(ip);
            }
            Map(regexes::Map(map)) => {
                self.map = Some(map);
            }
            PlayerCount(playercount) => {
                self.max_players = Some(playercount.max);
                self.num_players = Some(playercount.players);
            }
            G15(_) | Status(_) | DemoStop(_) => {}
        }
    }

    fn handle_chat(&mut self, chat: ChatMessage) {
        tracing::debug!("Chat: {:?}", chat);
        self.chat_history.push(chat);
    }

    fn handle_kill(&mut self, kill: PlayerKill) {
        tracing::debug!("Kill: {:?}", kill);
        self.kill_history.push(kill);
    }

    pub fn handle_demo_message(&mut self, demo_message: DemoMessage, players: &Players) {
        match demo_message.event {
            DemoEvent::VoteOptions(options) => self.handle_vote_options(&options),
            DemoEvent::VoteCast(cast_vote, steamid) => self.handle_vote_cast(&cast_vote, steamid),
            DemoEvent::VoteStarted(_) | DemoEvent::LatestTick => {}
        }
        self.check_shunted_votes(players);
    }

    fn handle_vote_options(&mut self, options: &VoteOptionsEvent) {
        let mut values = Vec::new();
        tracing::info!("Vote options:");
        for i in 0..options.count {
            let opt = match i {
                0 => options.option_1.to_string(),
                1 => options.option_2.to_string(),
                2 => options.option_3.to_string(),
                3 => options.option_4.to_string(),
                4 => options.option_5.to_string(),
                _ => String::new(),
            };

            tracing::info!("\t{}", opt);
            values.push(opt);
        }

        let vote = VoteEvent {
            idx: options.voteidx,
            options: values,
            votes: Vec::new(),
        };

        self.vote_history.push(vote);
    }

    fn handle_vote_cast(&mut self, vote: &VoteCastEvent, caster: Option<SteamID>) {
        let cast_vote = CastVote {
            steamid: caster,
            option: vote.vote_option,
        };

        self.shunted_vote_cast_events
            .push((vote.voteidx, cast_vote));
    }

    fn check_shunted_votes(&mut self, players: &Players) {
        // Replay shunted messages if we have them. This ensures that we don't print VoteCast events for Vote we haven't seen the
        // VoteOptions event for yet. Saves
        if self.shunted_vote_cast_events.is_empty() {
            return;
        }

        // We need to temporarily move the event queue into a local buffer so we can immutably borrow self
        // inside the closure. Once we are done, we move the queue back into self.shunted_vote_cast_messages
        let mut temp = Vec::new();
        std::mem::swap(&mut temp, &mut self.shunted_vote_cast_events);
        temp.retain(|(idx, cast_vote)| {
            // Reverse iterator to check most recent votes first, as there may be earlier votes with the same idx
            let Some(vote) = self.vote_history.iter_mut().rev().find(|v| v.idx == *idx) else {
                return true;
            };

            let name = cast_vote
                .steamid
                .and_then(|s| players.get_name(s))
                .unwrap_or("Unknown player");
            let vote_option: &str = vote
                .options
                .get(cast_vote.option as usize)
                .map_or("Invalid vote option", String::as_str);
            tracing::info!("{name} - {vote_option}");

            vote.votes.push(cast_vote.clone());

            false
        });
        std::mem::swap(&mut temp, &mut self.shunted_vote_cast_events);
    }
}
