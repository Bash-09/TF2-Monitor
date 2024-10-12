use std::path::PathBuf;

use keyvalues_parser::Vdf;
use steamid_ng::SteamID;
use steamlocate::SteamDir;

use crate::players::friends::Friend;

pub mod api;
pub mod launch_options;

pub const TF2_GAME_ID: u32 = 440;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Steamlocate({0})")]
    Steamlocate(#[from] steamlocate::Error),
    #[error("IO({0})")]
    Io(#[from] std::io::Error),
    #[error("Could not find an installation of TF2")]
    NoTF2Installation,
    #[error("VdfFile did not have expected structure")]
    InvalidStructure,
    #[error("VDF({0})")]
    Vdf(Box<keyvalues_parser::error::Error>),
    #[error("No valid users were found")]
    NoValidUser,
}

impl From<keyvalues_parser::error::Error> for Error {
    fn from(value: keyvalues_parser::error::Error) -> Self {
        Self::Vdf(Box::new(value))
    }
}

/// Reads the Steam/config/loginusers.vdf file to find the currently logged
/// in steam ID.
///
/// # Errors
/// - If steam file could not be located or parsed
/// - If no suitable user could be identified
pub fn find_current_steam_user() -> Result<SteamID, Error> {
    let user_conf_path = SteamDir::locate()?.path().join("config/loginusers.vdf");

    let user_conf_contents = std::fs::read(user_conf_path)?;
    let login_users_contents = String::from_utf8_lossy(&user_conf_contents);

    let login_vdf = Vdf::parse(&login_users_contents)?;
    let users_obj = login_vdf.value.get_obj().ok_or(Error::InvalidStructure)?;
    let mut latest_timestamp = 0;
    let mut latest_user_sid64: Option<SteamID> = None;

    for (user_sid64, user_data_values) in users_obj {
        user_data_values
            .iter()
            .filter_map(|value| value.get_obj())
            .for_each(|user_data_obj| {
                if let Some(timestamp) = user_data_obj
                    .get("Timestamp")
                    .and_then(|timestamp_values| timestamp_values.first())
                    .and_then(|timestamp_vdf| timestamp_vdf.get_str())
                    .and_then(|timestamp_str| timestamp_str.parse::<i64>().ok())
                {
                    if timestamp > latest_timestamp {
                        if let Ok(user_steamid) = user_sid64.parse::<u64>().map(SteamID::from) {
                            latest_timestamp = timestamp;
                            latest_user_sid64 = Some(user_steamid);
                        }
                    }
                }
            });
    }

    latest_user_sid64.ok_or(Error::NoValidUser)
}

/// Attempts to find the given user's friend list by reading the local steam config files.
///
/// # Errors
/// * If the given steam user does not have any local configs
/// * If the config files for the player are not valid or complete
/// * IO errors
pub fn find_steam_user_friends(steamid: SteamID) -> Result<Vec<Friend>, Error> {
    #[allow(clippy::unreadable_literal)]
    let id_32 = u64::from(steamid) & 0xFFFFFFFF;

    let user_conf_path = SteamDir::locate()?
        .path()
        .join(format!("userdata/{id_32}/config/localconfig.vdf"));
    let user_conf_contents = std::fs::read(user_conf_path)?;
    let user_conf_string = String::from_utf8_lossy(&user_conf_contents);
    let user_vdf = Vdf::parse(&user_conf_string)?;

    let friends = user_vdf
        .value
        .get_obj()
        .ok_or(Error::InvalidStructure)?
        .get("friends")
        .ok_or(Error::InvalidStructure)?
        .iter()
        .filter_map(|v| v.get_obj())
        .flat_map(|o| o.keys())
        .filter_map(|s| SteamID::from_steam3(format!("[U:1:{s}]").as_str()).ok())
        .map(|steamid| Friend {
            steamid,
            friend_since: 0,
        })
        .filter(|f| f.steamid != steamid)
        .collect();

    Ok(friends)
}

/// # Errors
/// - If the Steam directory couldn't be found
/// - If the user's localconfig file could not be found in the Steam directory
pub fn locate_steam_launch_configs(steam_user: SteamID) -> Result<PathBuf, Error> {
    let account_id = steam_user.account_id();
    let local_config_path = format!("userdata/{account_id}/config/localconfig.vdf");

    let steam = SteamDir::locate()?;
    Ok(steam.path().join(local_config_path))
}

/// Attempts to open the TF2 directory or locate it if it's not in the expected
/// place
///
/// # Errors
/// - If the Steam directory could not be found
/// - If the user's TF2 installation could not be found through Steam
pub fn locate_tf2_folder() -> Result<PathBuf, Error> {
    let sd = SteamDir::locate()?;
    let (app, library) = sd.find_app(TF2_GAME_ID)?.ok_or(Error::NoTF2Installation)?;
    Ok(library.resolve_app_dir(&app))
}
