use serde::{Deserialize, Serialize};

pub const SETTINGS_IDENTIFIER: &str = "MACClientSettings";

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
#[allow(clippy::module_name_repetitions)]
pub struct AppSettings {
    pub window_pos: Option<(i32, i32)>,
    pub window_size: Option<(u32, u32)>,
}
