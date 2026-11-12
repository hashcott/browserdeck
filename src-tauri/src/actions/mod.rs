use crate::config::Action;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ActionError {
    UnknownButton,
    FileNotFound,
    LaunchFailed,
    InvalidHotkey,
    BlockedByElevatedWindow,
    VolumeFailed,
    ObsNotConnected,
    ObsSceneMissing,
    UnsupportedPlatform,
}

#[async_trait::async_trait]
pub trait Executor: Send + Sync {
    async fn run(&self, action: &Action) -> Result<(), ActionError>;
}
