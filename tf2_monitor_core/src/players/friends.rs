use serde::Serialize;
use steamid_ng::SteamID;

use super::serialize_steamid_as_string;

#[derive(Debug, Clone, Serialize)]
pub struct Friend {
    #[serde(rename = "steamID64", serialize_with = "serialize_steamid_as_string")]
    pub steamid: SteamID,
    #[serde(rename = "friendSince")]
    pub friend_since: u64,
}

#[derive(Debug, Serialize, Default)]
pub struct FriendInfo {
    pub public: Option<bool>,
    pub friends: Vec<Friend>,
}

impl FriendInfo {
    #[must_use]
    pub fn friends(&self) -> &[Friend] {
        &self.friends
    }
}
