use std::{
    collections::{HashMap, VecDeque},
    path::{Path, PathBuf},
};

use serde::{Serialize, Serializer};
use steamid_ng::SteamID;

use crate::{
    console::commands::{g15, regexes::StatusLine},
    settings::{AppDetails, ConfigFilesError, Settings},
};

use self::{
    friends::{Friend, FriendInfo},
    game_info::GameInfo,
    parties::Parties,
    records::{default_custom_data, PlayerRecord, Records, Verdict},
    steam_info::SteamInfo,
};

pub mod friends;
pub mod game_info;
#[allow(clippy::module_name_repetitions)]
pub mod new_players;
pub mod parties;
pub mod records;
pub mod steam_info;

pub const STEAM_CACHE_FILE_NAME: &str = "steam_cache.bin";

// const MAX_HISTORY_LEN: usize = 100;

pub struct Players {
    cache_path: Option<PathBuf>,

    pub game_info: HashMap<SteamID, GameInfo>,
    pub steam_info: HashMap<SteamID, SteamInfo>,
    pub friend_info: HashMap<SteamID, FriendInfo>,
    pub records: Records,
    pub parties: Parties,

    pub connected: Vec<SteamID>,
    pub history: VecDeque<SteamID>,

    pub user: Option<SteamID>,

    parties_needs_update: bool,
}

#[allow(dead_code)]
impl Players {
    #[must_use]
    pub fn new(records: Records, user: Option<SteamID>, cache_path: Option<PathBuf>) -> Self {
        let mut players = Self {
            cache_path,

            game_info: HashMap::new(),
            steam_info: HashMap::new(),
            friend_info: HashMap::new(),
            records,
            parties: Parties::new(),

            connected: Vec::new(),
            history: VecDeque::new(),
            user,

            parties_needs_update: false,
        };

        if players.cache_path.is_some() {
            match players.load_steam_info() {
                Ok(()) => tracing::info!(
                    "Loaded steam info cache with {} entries.",
                    players.steam_info.len()
                ),
                Err(ConfigFilesError::IO(e)) if e.kind() == std::io::ErrorKind::NotFound => {
                    tracing::warn!("No steam info cache was found, creating a new one.");
                }
                Err(e) => tracing::error!("Failed to load steam info cache: {e}"),
            }
        }

        players
    }

    /// Attempt to locate a suitable location to store the steam cache
    ///
    /// # Errors
    /// - If no suitable directory could be found to store the steam cache
    pub fn default_steam_cache_path(app_details: AppDetails) -> Result<PathBuf, ConfigFilesError> {
        Ok(Settings::locate_config_directory(app_details)?.join(STEAM_CACHE_FILE_NAME))
    }

    /// Retrieve the local verdict for a player
    #[must_use]
    pub fn verdict(&self, steamid: SteamID) -> Verdict {
        self.records
            .get(&steamid)
            .map_or(Verdict::Player, PlayerRecord::verdict)
    }

    /// Updates friends lists of a user
    /// Propagates to all other friends lists to ensure two-way lookup possible.
    /// Only call if friends list was obtained directly from Steam API (i.e.
    /// friends list is public)
    pub fn update_friends_list(&mut self, steamid: SteamID, friendslist: Vec<Friend>) {
        // Propagate to all other hashmap entries

        for friend in &friendslist {
            self.propagate_friend(steamid, friend);
        }

        let oldfriends: Vec<SteamID> = self.set_friends(steamid, friendslist);

        // If a player's friend has been unfriended, remove player from friend's hashmap
        for oldfriend in oldfriends {
            self.remove_from_friends_list(oldfriend, steamid);
        }
    }

    /// Sets the friends list and friends list visibility, returning any old
    /// friends that have been removed
    fn set_friends(&mut self, steamid: SteamID, friends: Vec<Friend>) -> Vec<SteamID> {
        self.parties_needs_update = true;

        let friend_info = self.friend_info.entry(steamid).or_default();

        friend_info.public = Some(true);

        let mut removed_friends = friends;
        friend_info
            .friends
            .retain(|f1| !removed_friends.iter().any(|f2| f1.steamid == f2.steamid));
        std::mem::swap(&mut removed_friends, &mut friend_info.friends);

        removed_friends.into_iter().map(|f| f.steamid).collect()
    }

    /// Helper function to add a friend to a friends list
    fn propagate_friend(&mut self, steamid: SteamID, friend: &Friend) {
        let friend_info = self.friend_info.entry(friend.steamid).or_default();

        friend_info.friends.push(Friend {
            steamid,
            friend_since: friend.friend_since,
        });
    }

    /// Helper function to remove a friend from a player's friendlist.
    fn remove_from_friends_list(&mut self, steamid: SteamID, friend_to_remove: SteamID) {
        if let Some(friends) = self.friend_info.get_mut(&steamid) {
            friends.friends.retain(|f| f.steamid != friend_to_remove);
            if friends.friends.is_empty() && friends.public.is_none() {
                self.friend_info.remove(&steamid);
            }
        }

        if let Some(friends) = self.friend_info.get_mut(&friend_to_remove) {
            friends.friends.retain(|f| f.steamid != steamid);
            if friends.friends.is_empty() && friends.public.is_none() {
                self.friend_info.remove(&friend_to_remove);
            }
        }
    }

    /// Mark a friends list as being private, trim all now-stale information.
    pub fn mark_friends_list_private(&mut self, steamid: SteamID) {
        let friends = self.friend_info.entry(steamid).or_default();
        let old_vis_state = friends.public;
        if old_vis_state.is_some_and(|public| !public) {
            return;
        }

        friends.public = Some(false);

        let old_friendslist = friends.friends.clone();

        for friend in old_friendslist {
            if let Some(friends_of_friend) = self.friend_info.get(&friend.steamid) {
                // If friend's friendlist is public, that information isn't stale.
                if friends_of_friend.public.is_some_and(|p| p) {
                    continue;
                }

                self.remove_from_friends_list(friend.steamid, steamid);
            }
        }
    }

    /// Check if an account is friends with the user.
    /// Returns None if we don't have enough information to tell.
    #[must_use]
    pub fn is_friends_with_user(&self, friend: SteamID) -> Option<bool> {
        self.user.and_then(|user| self.are_friends(friend, user))
    }

    /// Check if two accounts are friends with each other.
    /// Returns None if we don't have enough information to tell.
    #[must_use]
    pub fn are_friends(&self, friend1: SteamID, friend2: SteamID) -> Option<bool> {
        if let Some(friends) = self.friend_info.get(&friend1) {
            if friends.friends.iter().any(|f| f.steamid == friend2) {
                return Some(true);
            }

            // Friends list is public, so we should be able to see the other party
            // regardless
            if friends.public.is_some_and(|p| p) {
                return Some(false);
            }
        }

        // Other friends list is public, so 2-way lookup should have been possible
        if self
            .friend_info
            .get(&friend2)
            .is_some_and(|f| f.public.is_some_and(|p| p))
        {
            return Some(false);
        }

        // Both are private :(
        None
    }

    /// Moves any old players from the server into history. Any console commands
    /// (status, `g15_dumpplayer`, etc) should be run before calling this
    /// function again to prevent removing all players from the player list.
    pub fn refresh(&mut self) {
        // Get old players
        let unaccounted_players: Vec<SteamID> = self
            .connected
            .iter()
            .filter(|&s| self.game_info.get(s).map_or(true, GameInfo::should_prune))
            .copied()
            .collect();

        if !unaccounted_players.is_empty() {
            self.parties_needs_update = true;
        }

        self.connected.retain(|s| !unaccounted_players.contains(s));

        // Remove any of them from the history as they will be added more recently
        self.history
            .retain(|p| !unaccounted_players.iter().any(|up| up == p));

        // Shrink to not go past max number of players
        // let num_players = self.history.len() + unaccounted_players.len();
        // for _ in MAX_HISTORY_LEN..num_players {
        //     self.history.pop_front();
        // }

        for p in unaccounted_players {
            self.history.push_back(p);
        }

        // Mark all remaining players as unaccounted, they will be marked as accounted
        // again when they show up in status or another console command.
        self.game_info.values_mut().for_each(GameInfo::next_cycle);

        if self.parties_needs_update {
            self.parties
                .find_parties(&self.friend_info, &self.connected);
            self.parties_needs_update = false;
        }
    }

    /// Gets a struct containing all the relevant data on a player in a
    /// serializable format
    pub fn get_serializable_player(&self, steamid: SteamID) -> Player {
        let game_info = self.game_info.get(&steamid);
        let steam_info = self.steam_info.get(&steamid);
        let name = game_info.map_or_else(
            || steam_info.map_or("", |si| &si.account_name),
            |gi| &gi.name,
        );

        let record = self.records.get(&steamid);
        let previous_names = record
            .as_ref()
            .map(|r| r.previous_names().iter().map(AsRef::as_ref).collect())
            .unwrap_or_default();

        let friend_info = self.friend_info.get(&steamid);
        let friends: Vec<&Friend> = friend_info
            .as_ref()
            .map(|fi| fi.friends.iter().collect())
            .unwrap_or_default();

        let local_verdict = record.as_ref().map_or(Verdict::Player, |r| r.verdict());

        Player {
            isSelf: self.user.is_some_and(|user| user == steamid),
            name,
            steamID64: steamid,
            localVerdict: local_verdict,
            steamInfo: steam_info,
            gameInfo: game_info,
            customData: record
                .as_ref()
                .map_or_else(default_custom_data, |r| r.custom_data().clone()),
            convicted: false,
            previous_names,
            friends,
            friendsIsPublic: friend_info.and_then(|fi| fi.public),
        }
    }

    pub fn handle_g15(&mut self, players: Vec<g15::G15Player>) {
        for g15 in players {
            let Some(steamid) = g15.steamid else {
                continue;
            };

            if let Some(r) = self.records.get_mut(&steamid) {
                r.mark_seen();
            }

            // Add to connected players if they aren't already
            if !self.connected.contains(&steamid) {
                self.connected.push(steamid);
                self.parties_needs_update = true;
            }

            // Update game info
            if let Some(game_info) = self.game_info.get_mut(&steamid) {
                if let Some(name) = g15.name.as_ref() {
                    self.records.update_name(steamid, name);
                }
                game_info.update_from_g15(g15);
            } else if let Some(game_info) = GameInfo::new_from_g15(g15) {
                // Update name
                self.records.update_name(steamid, &game_info.name);
                self.game_info.insert(steamid, game_info);
            }
        }
    }

    pub fn handle_status_line(&mut self, status: StatusLine) {
        let steamid = status.steamid;

        if let Some(r) = self.records.get_mut(&steamid) {
            r.mark_seen();
        }

        // Add to connected players if they aren't already
        if !self.connected.contains(&steamid) {
            self.connected.push(steamid);
            self.parties_needs_update = true;
        }

        if let Some(game_info) = self.game_info.get_mut(&steamid) {
            if status.name != game_info.name {
                self.records.update_name(steamid, &status.name);
            }

            game_info.update_from_status(status);
        } else {
            let game_info = GameInfo::new_from_status(status);

            // Update name
            self.records.update_name(steamid, &game_info.name);
            self.game_info.insert(steamid, game_info);
        }
    }

    #[must_use]
    pub fn get_name(&self, steamid: SteamID) -> Option<&str> {
        if let Some(gi) = self.game_info.get(&steamid) {
            return Some(&gi.name);
        } else if let Some(si) = self.steam_info.get(&steamid) {
            return Some(&si.account_name);
        } else if let Some(last_name) = self
            .records
            .get(&steamid)
            .map(|r| r.previous_names().first())
        {
            return last_name.map(String::as_str);
        }

        None
    }

    #[must_use]
    pub fn get_steamid_from_name(&self, name: &str) -> Option<SteamID> {
        self.connected
            .iter()
            .find(|&s| self.game_info.get(s).is_some_and(|gi| gi.name == name))
            .copied()
    }

    #[must_use]
    pub fn get_name_to_steam_ids_map(&self) -> HashMap<String, SteamID> {
        self.connected
            .iter()
            .filter_map(|s| self.game_info.get(s).map(|gi| (gi.name.clone(), *s)))
            .collect()
    }

    /// # Errors
    /// If the file could not be read from disk or the data could not be deserialized
    pub fn load_steam_info(&mut self) -> Result<(), ConfigFilesError> {
        let path = self
            .cache_path
            .as_ref()
            .ok_or(ConfigFilesError::NoConfigSet)?
            .clone();
        self.load_steam_info_from(&path)
    }

    /// # Errors
    /// If the data could not be serialized or the file could not be written back to disk
    pub fn save_steam_info(&self) -> Result<(), ConfigFilesError> {
        let path = self
            .cache_path
            .as_ref()
            .ok_or(ConfigFilesError::NoConfigSet)?;
        self.save_steam_info_to(path)
    }

    pub fn save_steam_info_ok(&self) {
        if let Err(e) = self.save_steam_info() {
            tracing::error!("Failed to save steam info cache: {e}");
        } else {
            tracing::debug!("Saved steam info cache.");
        }
    }

    fn load_steam_info_from(&mut self, path: &Path) -> Result<(), ConfigFilesError> {
        let contents = std::fs::read(path)?;
        let steam_info = pot::from_slice(&contents)?;

        self.steam_info = steam_info;
        Ok(())
    }

    fn save_steam_info_to(&self, path: &Path) -> Result<(), ConfigFilesError> {
        let contents = pot::to_vec(&self.steam_info)?;
        std::fs::write(path, contents)?;
        Ok(())
    }
}

impl Serialize for Players {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let players: Vec<Player> = self
            .connected
            .iter()
            .map(|&s| self.get_serializable_player(s))
            .collect();
        players.serialize(serializer)
    }
}

// Useful

#[allow(clippy::trivially_copy_pass_by_ref, clippy::missing_errors_doc)]
pub fn serialize_steamid_as_string<S: Serializer>(
    steamid: &SteamID,
    s: S,
) -> Result<S::Ok, S::Error> {
    format!("{}", u64::from(*steamid)).serialize(s)
}

#[allow(clippy::trivially_copy_pass_by_ref, clippy::missing_errors_doc)]
pub fn serialize_maybe_steamid_as_string<S: Serializer>(
    steamid: &Option<SteamID>,
    s: S,
) -> Result<S::Ok, S::Error> {
    match steamid {
        Some(steamid) => format!("{}", u64::from(*steamid)).serialize(s),
        None => s.serialize_none(),
    }
}

#[allow(non_snake_case)]
#[derive(Debug, Serialize)]
pub struct Player<'a> {
    pub isSelf: bool,
    pub name: &'a str,
    #[serde(serialize_with = "serialize_steamid_as_string")]
    pub steamID64: SteamID,

    pub steamInfo: Option<&'a SteamInfo>,
    pub gameInfo: Option<&'a GameInfo>,
    pub customData: serde_json::Value,
    pub localVerdict: Verdict,
    pub convicted: bool,
    pub previous_names: Vec<&'a str>,

    pub friends: Vec<&'a Friend>,
    pub friendsIsPublic: Option<bool>,
}
