use std::{
    collections::HashMap,
    fmt::Display,
    io::{ErrorKind, Write},
    ops::{Deref, DerefMut},
    path::PathBuf,
};

use atomic_write_file::AtomicWriteFile;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Map;
use steamid_ng::SteamID;

use crate::settings::{merge_json_objects, AppDetails, ConfigFilesError, Settings};

pub const RECORDS_FILE_NAME: &str = "playerlist.json";

// PlayerList

#[derive(Serialize, Deserialize, Default)]
pub struct PlayerRecords {
    #[serde(skip)]
    pub path: Option<PathBuf>,
    pub records: HashMap<SteamID, PlayerRecord>,
}

impl PlayerRecords {
    /// # Errors
    /// If the config directory could not be located (usually because no valid
    /// home directory was found)
    pub fn default_file_location(app_details: AppDetails) -> Result<PathBuf, ConfigFilesError> {
        Ok(Settings::locate_config_directory(app_details)?.join(RECORDS_FILE_NAME))
    }

    /// Attempts to load the playerlist from the overriden (if provided in
    /// [Args]) or default location. If it cannot be found, then a new one
    /// is created at the location.
    ///
    /// # Errors
    /// If the playerlist file was provided but could not be parsed, or another
    /// unexpected error occurred
    #[allow(clippy::cognitive_complexity)]
    pub fn load_or_create(playerlist_file_path: PathBuf) -> Result<Self, ConfigFilesError> {
        match Self::load_from(playerlist_file_path.clone()) {
            Ok(records) => Ok(records),
            Err(ConfigFilesError::IO(e)) if e.kind() == ErrorKind::NotFound => {
                tracing::warn!("Could not locate {playerlist_file_path:?}, creating new file.");
                Ok(Self {
                    path: Some(playerlist_file_path),
                    ..Default::default()
                })
            }
            Err(e) => Err(e),
        }
    }

    /// Attempt to load the `PlayerRecords` from the provided file
    ///
    /// # Errors
    /// If the file could not be located, read, or parsed.
    pub fn load_from(path: PathBuf) -> Result<Self, ConfigFilesError> {
        let contents = std::fs::read_to_string(&path)?;
        let mut playerlist: Self = serde_json::from_str(&contents)?;
        playerlist.path = Some(path);

        // Map all of the steamids to the records. They were not included when
        // serializing/deserializing the records to prevent duplication in the
        // resulting file.
        for record in &mut playerlist.records.values_mut() {
            // Some old versions had the custom_data set to `null` by default, but an empty
            // object is preferable so I'm using this to fix it lol. It's really
            // not necessary but at the time the UI wasn't a fan of nulls in the
            // custom_data and this fixes it so whatever. :3
            if record.custom_data.is_null() {
                record.custom_data = serde_json::Value::Object(serde_json::Map::new());
            }
        }

        Ok(playerlist)
    }

    /// Removes all records that don't contain any info worth retaining.
    pub fn prune(&mut self) {
        self.retain(|_, r| !r.is_empty());
    }

    /// Attempt to save the `PlayerRecords` to the file it was loaded from
    ///
    /// # Errors
    /// If it failed to serialize or write back to the file.
    pub fn save(&mut self) -> Result<(), ConfigFilesError> {
        self.prune();

        let path = self.path.as_ref().ok_or(ConfigFilesError::NoConfigSet)?;

        let mut file = AtomicWriteFile::open(path)?;
        let contents = serde_json::to_string(self)?;

        write!(file, "{contents}")?;
        file.commit()?;

        Ok(())
    }

    pub fn save_ok(&mut self) {
        match self.save() {
            Ok(()) => tracing::debug!("Successfully saved player records to {:?}", self.path),
            Err(e) => tracing::error!("Failed to save player records to {:?}: {e}", self.path),
        }
    }

    pub fn update_name(&mut self, steamid: SteamID, name: &str) {
        if let Some(record) = self.records.get_mut(&steamid) {
            record.add_previous_name(name);
        }
    }
}

impl Deref for PlayerRecords {
    type Target = HashMap<SteamID, PlayerRecord>;

    fn deref(&self) -> &Self::Target {
        &self.records
    }
}

impl DerefMut for PlayerRecords {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.records
    }
}

// PlayerRecord

/// A Record of a player stored in the persistent personal playerlist
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(default)]
pub struct PlayerRecord {
    custom_data: serde_json::Value,
    verdict: Verdict,
    previous_names: Vec<String>,
    last_seen: Option<DateTime<Utc>>,
    /// Time of last manual change made by the user.
    modified: DateTime<Utc>,
    created: DateTime<Utc>,
}

impl PlayerRecord {
    /// Returns true if the record does not hold any meaningful information
    #[must_use]
    pub fn is_empty(&self) -> bool {
        fn value_is_empty(v: &serde_json::Value) -> bool {
            v.is_null()
                || v.as_str().is_some_and(str::is_empty)
                || v.as_array().is_some_and(|a| a.iter().all(value_is_empty))
                || v.as_object()
                    .is_some_and(|m| m.values().all(value_is_empty))
        }

        self.verdict == Verdict::Player && value_is_empty(&self.custom_data)
    }
}

impl Default for PlayerRecord {
    fn default() -> Self {
        Self {
            custom_data: default_custom_data(),
            verdict: Verdict::default(),
            previous_names: Vec::new(),
            last_seen: None,
            modified: default_date(),
            created: default_date(),
        }
    }
}

impl PlayerRecord {
    #[must_use]
    pub const fn custom_data(&self) -> &serde_json::Value {
        &self.custom_data
    }
    pub fn clear_custom_data(&mut self) -> &mut Self {
        self.custom_data = serde_json::Value::Object(Map::new());
        self.modified = Utc::now();
        self
    }
    pub fn set_custom_data(&mut self, val: serde_json::Value) -> &mut Self {
        merge_json_objects(&mut self.custom_data, val);
        self.modified = Utc::now();
        self
    }
    #[must_use]
    pub const fn verdict(&self) -> Verdict {
        self.verdict
    }
    pub fn set_verdict(&mut self, verdict: Verdict) -> &mut Self {
        self.verdict = verdict;
        self.modified = Utc::now();
        self
    }
    #[must_use]
    pub fn previous_names(&self) -> &[String] {
        &self.previous_names
    }
    pub fn add_previous_name(&mut self, name: &str) -> &mut Self {
        if self.previous_names.first().is_some_and(|n| n == name) {
            return self;
        }

        self.previous_names.retain(|n| n != name);
        self.previous_names.insert(0, name.to_owned());
        self
    }
    #[must_use]
    pub const fn modified(&self) -> DateTime<Utc> {
        self.modified
    }
    #[must_use]
    pub const fn created(&self) -> DateTime<Utc> {
        self.created
    }

    #[must_use]
    pub const fn last_seen(&self) -> Option<DateTime<Utc>> {
        self.last_seen
    }

    pub fn mark_seen(&mut self) {
        self.last_seen = Some(Utc::now());
    }
}

#[must_use]
pub fn default_custom_data() -> serde_json::Value {
    serde_json::Value::Object(Map::new())
}

#[must_use]
pub fn default_date() -> DateTime<Utc> {
    Utc::now()
}

/// What a player is marked as in the personal playerlist
#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    Player,
    Bot,
    Suspicious,
    Cheater,
    Trusted,
}

impl Display for Verdict {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

impl Default for Verdict {
    fn default() -> Self {
        Self::Player
    }
}
