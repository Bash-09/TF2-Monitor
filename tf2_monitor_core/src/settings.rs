use std::{
    fmt::Display,
    io::{self, ErrorKind, Write},
    path::{Path, PathBuf},
};

use atomic_write_file::AtomicWriteFile;
use directories_next::ProjectDirs;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use steamid_ng::SteamID;
use thiserror::Error;

use crate::{gamefinder, player_records::Verdict, web::UISource};

pub const CONFIG_FILE_NAME: &str = "config.yaml";

#[derive(Debug, Clone, Copy)]
pub struct AppDetails<'a> {
    pub qualifier: &'a str,
    pub organization: &'a str,
    pub application: &'a str,
}

#[derive(Debug, Error)]
pub enum ConfigFilesError {
    #[error("No valid home directory found")]
    NoValidHome,
    #[error("IO({0})")]
    IO(#[from] io::Error),
    #[error("Yaml{0}")]
    Yaml(#[from] serde_yaml::Error),
    #[error("Json{0}")]
    Json(#[from] serde_json::Error),
    #[error("Pot({0})")]
    Pot(#[from] pot::Error),
    #[error("Failed to located game/user information: {0}")]
    GameFinder(#[from] gamefinder::Error),
    #[error("No config file path is set")]
    NoConfigSet,
}
#[derive(Debug, Serialize, Deserialize, Copy, Clone, PartialEq, Eq)]
pub enum FriendsAPIUsage {
    None,
    CheatersOnly,
    All,
}

impl Display for FriendsAPIUsage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

impl FriendsAPIUsage {
    #[must_use]
    pub fn lookup(self, verdict: Verdict) -> bool {
        match verdict {
            Verdict::Player | Verdict::Suspicious | Verdict::Trusted => self == Self::All,
            Verdict::Bot | Verdict::Cheater => self != Self::None,
        }
    }
}

#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    #[serde(skip)]
    pub config_path: Option<PathBuf>,
    #[serde(skip)]
    pub steam_user: Option<SteamID>,
    #[serde(skip)]
    pub tf2_directory: Option<PathBuf>,

    pub rcon_password: String,
    pub steam_api_key: String,
    pub friends_api_usage: FriendsAPIUsage,
    pub request_playtime: bool,
    pub rcon_port: u16,
    pub external: serde_json::Value,
    pub autokick_bots: bool,

    pub minimal_demo_parsing: bool,

    pub masterbase_key: String,
    pub masterbase_host: String,
    #[serde(skip)]
    pub upload_demos: bool,
    #[serde(skip)]
    pub masterbase_http: bool,

    pub webui_port: u16,
    pub autolaunch_ui: bool,
    #[serde(skip)]
    pub web_ui_source: UISource,
}

#[allow(dead_code)]
impl Settings {
    /// Attempts to set the TF2 directory by locating and reading steam config files
    ///
    /// # Errors
    /// - If the steam install location could not be found
    /// - Config files failed to be read or parsed
    /// - Necessary information was missing
    #[allow(clippy::missing_panics_doc)]
    pub fn infer_tf2_directory(&mut self) -> Result<&Path, ConfigFilesError> {
        let tf2_directory = gamefinder::locate_tf2_folder()?;
        self.tf2_directory = Some(tf2_directory);

        Ok(self
            .tf2_directory
            .as_deref()
            .expect("Just set TF2 directory"))
    }

    /// Attempts to set the steam user by locating and reading steam config files
    ///
    /// # Errors
    /// - If the steam install location could not be found
    /// - Config files failed to be read or parsed
    /// - Necessary information was missing
    /// - No viable steam user could be identified
    pub fn infer_steam_user(&mut self) -> Result<SteamID, ConfigFilesError> {
        let steam_user = gamefinder::find_current_steam_user()?;
        self.steam_user = Some(steam_user);

        Ok(steam_user)
    }

    /// Attempts to locate the default file location for the settings config file
    ///
    /// # Errors
    /// If an appropriate location could not be found
    pub fn default_file_location(app_details: AppDetails) -> Result<PathBuf, ConfigFilesError> {
        Ok(Self::locate_config_directory(app_details)?.join(CONFIG_FILE_NAME))
    }

    /// Attempts to load the [Settings] at the specified location.
    /// If it cannot be found, new [Settings] will be
    /// created at that location.
    ///
    /// # Errors
    /// * `IO` - If the file could not be loaded from some reason
    /// * `Yaml` - If the contents of the file were not valid
    #[allow(clippy::cognitive_complexity)]
    pub fn load_or_create(config_file_path: PathBuf) -> Result<Self, ConfigFilesError> {
        match Self::load_from(config_file_path.clone()) {
            Ok(settings) => Ok(settings),
            Err(ConfigFilesError::IO(e)) if e.kind() == ErrorKind::NotFound => {
                tracing::warn!("Could not locate {config_file_path:?}, creating new file.");
                Ok(Self {
                    config_path: Some(config_file_path),
                    ..Default::default()
                })
            }
            Err(e) => Err(e),
        }
    }

    /// Attempt to load settings from a provided configuration file, or just use
    /// default config
    ///
    /// # Errors
    /// If the config file could not be located (usually because no valid home
    /// directory could be found)
    pub fn load_from(config_file_path: PathBuf) -> Result<Self, ConfigFilesError> {
        // Read config.yaml file if it exists, otherwise try to create a default file.
        let contents = std::fs::read_to_string(&config_file_path)?;
        let mut settings = serde_yaml::from_str::<Self>(&contents)?;
        tracing::debug!("Successfully loaded {config_file_path:?}");
        settings.config_path = Some(config_file_path);
        Ok(settings)
    }

    /// Attempt to save the settings back to the loaded configuration file
    ///
    /// # Errors
    /// If the settings could not be serialized or written back to disk
    pub fn save(&self) -> Result<(), ConfigFilesError> {
        let config_path = self
            .config_path
            .as_ref()
            .ok_or(ConfigFilesError::NoConfigSet)?;

        let mut file = AtomicWriteFile::open(config_path)?;
        write!(&mut file, "{}", serde_yaml::to_string(self)?)?;
        file.commit()?;

        Ok(())
    }

    pub fn save_ok(&mut self) {
        match self.save() {
            Ok(()) => tracing::debug!("Successfully saved settings to {:?}", self.config_path),
            Err(e) => tracing::error!("Failed to save settings to {:?}: {e}", self.config_path),
        }
    }

    pub fn update_external_preferences(&mut self, prefs: serde_json::Value) {
        merge_json_objects(&mut self.external, prefs);
    }

    /// Attempts to find (and create) a directory to be used for configuration
    /// files
    ///
    /// # Errors
    /// If a valid config file directory could not be found (usually because a
    /// valid home directory was not found)
    pub fn locate_config_directory(app_details: AppDetails) -> Result<PathBuf, ConfigFilesError> {
        let dirs = ProjectDirs::from(
            app_details.qualifier,
            app_details.organization,
            app_details.application,
        )
        .ok_or(ConfigFilesError::NoValidHome)?;
        let dir = dirs.config_dir();
        std::fs::create_dir_all(dir)?;
        Ok(PathBuf::from(dir))
    }

    /// # Errors
    /// If a valid config file path could not be found (usually because a
    /// valid home directory was not found)
    pub fn locate_config_file_path(app_details: AppDetails) -> Result<PathBuf, ConfigFilesError> {
        Self::locate_config_directory(app_details).map(|dir| dir.join("config.yaml"))
    }
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            steam_user: None,
            config_path: None,
            tf2_directory: None,
            rcon_password: "tf2monitor".into(),
            steam_api_key: String::new(),
            masterbase_key: String::new(),
            masterbase_host: "megaanticheat.com".into(),
            friends_api_usage: FriendsAPIUsage::CheatersOnly,
            request_playtime: true,
            webui_port: 3621,
            autolaunch_ui: false,
            rcon_port: 27015,
            external: serde_json::Value::Object(Map::new()),
            upload_demos: false,
            minimal_demo_parsing: false,
            masterbase_http: false,
            autokick_bots: false,
            web_ui_source: UISource::default(),
        }
    }
}

// Useful

/// Combines the second provided Json Object into the first. If the given
/// [Value]s are not `Value::Object`s, this will do nothing.
pub fn merge_json_objects(a: &mut Value, b: Value) {
    if let Value::Object(a) = a {
        if let Value::Object(b) = b {
            for (k, v) in b {
                // Remove if null or empty
                if v.is_null()
                    || v.as_str().is_some_and(str::is_empty)
                    || v.as_array().is_some_and(Vec::is_empty)
                    || v.as_object().is_some_and(Map::is_empty)
                {
                    a.remove(&k);
                } else {
                    merge_json_objects(a.entry(k).or_insert(Value::Null), v);
                }
            }

            return;
        }
    }

    *a = b;
}
