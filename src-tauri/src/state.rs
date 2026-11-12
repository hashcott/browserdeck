use crate::actions::{ActionError, Executor};
use crate::config::Config;
use crate::hub::Hub;
use serde::Serialize;
use std::path::PathBuf;
use std::sync::atomic::AtomicU16;
use std::sync::Arc;

#[derive(Serialize, Clone, Copy, Debug, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ObsState {
    Connected,
    Disconnected,
    AuthFailed,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum CoreEvent {
    PhoneStatus { connected: bool },
    ObsStatus { state: ObsState },
    ActionFailed { button_id: String, error: ActionError },
}

pub trait EventSink: Send + Sync {
    fn emit(&self, event: CoreEvent);
}

pub struct AppState {
    /// Holds config.json and icons/.
    pub data_dir: PathBuf,
    pub config: tokio::sync::RwLock<Config>,
    /// The port actually listened on.
    pub port: AtomicU16,
    pub hub: Hub,
    pub executor: Arc<dyn Executor>,
    pub events: Arc<dyn EventSink>,
}
