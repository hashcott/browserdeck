use crate::actions::{ActionError, Executor};
use crate::actions::obs::ObsClient;
use crate::commands::SystemSettingError;
use crate::config::{Config, ObsSettings};
use crate::hub::Hub;
use serde::Serialize;
use std::path::PathBuf;
use std::sync::atomic::AtomicU16;
use std::sync::Arc;
use tokio::sync::watch;

#[derive(Serialize, Clone, Copy, Debug, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ObsState {
    Connected,
    Disconnected,
    AuthFailed,
    /// OBS or obs-websocket is older than the `obws` crate supports.
    Unsupported,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum CoreEvent {
    PhoneStatus { connected: bool },
    ObsStatus { state: ObsState },
    ActionFailed { button_id: String, error: ActionError },
}

/// Contract: the hub calls `emit` while holding its lock, so an implementation must be
/// non-blocking and must never call back into the hub or take any app lock.
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
    pub obs: ObsClient,
    /// Must live as long as the app: the OBS task ends when it is dropped.
    pub obs_settings: watch::Sender<ObsSettings>,
    pub startup: Startup,
    /// How the last attempt to register start-with-Windows ended (at startup or from Settings).
    pub autostart_error: std::sync::Mutex<Option<SystemSettingError>>,
}

/// What happened while starting up; read-only afterwards.
#[derive(Default)]
pub struct Startup {
    pub first_run: bool,
    pub recovered_backup: Option<String>,
    /// Short code, set when no port could be bound.
    pub server_error: Option<&'static str>,
    /// Why administrator mode is on but this process is not elevated, when known.
    pub admin_error: Option<SystemSettingError>,
}

impl AppState {
    /// Spawns the OBS task, so it needs a Tokio runtime. `executor` receives the OBS client it should drive.
    pub fn new(
        data_dir: PathBuf,
        config: Config,
        events: Arc<dyn EventSink>,
        executor: impl FnOnce(ObsClient) -> Arc<dyn Executor>,
    ) -> AppState {
        let (obs_settings, rx) = watch::channel(config.settings.obs.clone());
        let obs = ObsClient::spawn(rx, events.clone());
        AppState {
            data_dir,
            config: tokio::sync::RwLock::new(config),
            port: AtomicU16::new(0),
            hub: Hub::new(events.clone()),
            executor: executor(obs.clone()),
            events,
            obs,
            obs_settings,
            startup: Startup::default(),
            autostart_error: std::sync::Mutex::new(None),
        }
    }
}
