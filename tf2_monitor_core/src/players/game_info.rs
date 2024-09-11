use serde::{Deserialize, Serialize};

use crate::console::commands::{g15::G15Player, regexes::StatusLine};

#[derive(Debug, Clone, Serialize)]
pub struct GameInfo {
    pub name: String,
    pub userid: String,
    pub team: Team,
    pub time: u32,
    pub ping: u32,
    pub loss: u32,
    pub state: PlayerState,
    pub kills: u32,
    pub deaths: u32,
    pub alive: bool,
    #[serde(skip)]
    /// How many cycles has passed since the player has been seen
    last_seen: u32,
}

impl Default for GameInfo {
    fn default() -> Self {
        Self {
            name: String::new(),
            userid: String::new(),
            team: Team::Unassigned,
            time: 0,
            ping: 0,
            loss: 0,
            state: PlayerState::Active,
            kills: 0,
            deaths: 0,
            last_seen: 0,
            alive: false,
        }
    }
}

impl GameInfo {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn new_from_g15(g15: G15Player) -> Option<Self> {
        g15.userid.as_ref()?;

        let mut game_info = Self::new();
        game_info.update_from_g15(g15);
        Some(game_info)
    }

    pub(crate) fn new_from_status(status: StatusLine) -> Self {
        let mut game_info = Self::new();
        game_info.update_from_status(status);
        game_info
    }

    pub(crate) fn update_from_g15(&mut self, g15: G15Player) {
        if let Some(name) = g15.name {
            self.name = name;
        }
        if let Some(userid) = g15.userid {
            self.userid = userid;
        }
        if let Some(team) = g15.team {
            self.team = team;
        }
        if let Some(ping) = g15.ping {
            self.ping = ping;
        }
        if let Some(kills) = g15.score {
            self.kills = kills;
        }
        if let Some(deaths) = g15.deaths {
            self.deaths = deaths;
        }
        if let Some(alive) = g15.alive {
            self.alive = alive;
        }

        self.acknowledge();
    }

    pub(crate) fn update_from_status(&mut self, status: StatusLine) {
        self.name = status.name;
        self.userid = status.userid;
        self.time = status.time;
        self.ping = status.ping;
        self.loss = status.loss;

        // Attach the spawning flag manually as it can be easily missed by the parsers due to timing.
        if status.time > 0 && status.time < 30 && self.team == Team::Unassigned {
            self.state = PlayerState::Spawning;
        }
        // Make the Spawning flag "sticky" until they either pick a class or join spectator.
        // Makes it easy to spot bots taking up a player slot that can't be kicked.
        else if self.state != PlayerState::Spawning
            || status.state != PlayerState::Active
            || self.alive
            || self.team == Team::Spectators
        {
            self.state = status.state;
        }

        self.acknowledge();
    }

    pub(crate) fn next_cycle(&mut self) {
        const DISCONNECTED_THRESHOLD: u32 = 2;

        self.last_seen += 1;
        if self.last_seen > DISCONNECTED_THRESHOLD {
            self.state = PlayerState::Disconnected;
        }
    }

    pub(crate) const fn should_prune(&self) -> bool {
        const CYCLE_LIMIT: u32 = 6;
        self.last_seen > CYCLE_LIMIT
    }

    fn acknowledge(&mut self) {
        self.last_seen = 0;

        if self.state == PlayerState::Disconnected {
            self.state = PlayerState::Active;
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize, Deserialize)]
#[allow(clippy::module_name_repetitions)]
pub enum PlayerState {
    Active,
    Spawning,
    Disconnected,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum Team {
    Unassigned = 0,
    Spectators = 1,
    Red = 2,
    Blu = 3,
}

impl TryFrom<u32> for Team {
    type Error = &'static str;
    fn try_from(val: u32) -> Result<Self, Self::Error> {
        match val {
            0 => Ok(Self::Unassigned),
            1 => Ok(Self::Spectators),
            2 => Ok(Self::Red),
            3 => Ok(Self::Blu),
            _ => Err("Not a valid team value"),
        }
    }
}

impl Serialize for Team {
    fn serialize<S>(&self, s: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        s.serialize_u32(*self as u32)
    }
}
