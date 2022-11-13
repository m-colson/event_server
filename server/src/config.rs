use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CalendarConfig {
    pub auth: bool,
    pub enabled: bool,
    pub token_file: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ManagerConfig {
    pub remotes: Vec<String>,
    pub google_calendar: CalendarConfig,
}
