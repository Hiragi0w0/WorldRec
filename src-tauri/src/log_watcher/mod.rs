pub mod finalize;
pub mod parser;
pub mod processor;
pub mod reader;
#[cfg_attr(test, allow(dead_code))]
pub mod service;

use serde::Serialize;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct LogWatcherStatus {
    pub running: bool,
    pub log_dir: String,
    pub db_path: String,
    pub last_error: Option<String>,
}

#[cfg(not(test))]
pub mod state;

pub mod stay_duration;
pub mod visit_session;
