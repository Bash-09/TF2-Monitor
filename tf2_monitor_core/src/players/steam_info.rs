use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SteamInfo {
    #[serde(rename = "name")]
    pub account_name: String,
    pub profile_url: String,
    #[serde(rename = "pfp")]
    pub pfp_url: String,
    pub pfp_hash: String,
    pub profile_visibility: ProfileVisibility,
    pub time_created: Option<u64>,
    pub country_code: Option<String>,
    pub vac_bans: u32,
    pub game_bans: u32,
    pub days_since_last_ban: Option<u32>,
    pub playtime: Option<u64>,
    pub fetched: DateTime<Utc>,
}

impl SteamInfo {
    #[must_use]
    pub fn expired(&self) -> bool {
        Utc::now().signed_duration_since(self.fetched).num_hours() > 3
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProfileVisibility {
    Private = 1,
    FriendsOnly = 2,
    Public = 3,
}

impl From<u8> for ProfileVisibility {
    fn from(value: u8) -> Self {
        match value {
            2 => Self::FriendsOnly,
            3 => Self::Public,
            _ => Self::Private,
        }
    }
}
