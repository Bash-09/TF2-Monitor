use serde::{Deserialize, Serialize};

pub const SETTINGS_IDENTIFIER: &'static str = "MACClientSettings";

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct AppSettings {
    pub window_pos: Option<(i32, i32)>,
    pub window_size: Option<(u32, u32)>,
}
