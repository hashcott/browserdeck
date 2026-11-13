pub mod hotkey;
pub mod keymap;
pub mod launch;
pub mod obs;
pub mod volume;

use crate::config::Action;
use crate::state::{AppState, CoreEvent};
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
    ObsRequestFailed,
    UnsupportedPlatform,
    Timeout,
}

#[async_trait::async_trait]
pub trait Executor: Send + Sync {
    async fn run(&self, action: &Action) -> Result<(), ActionError>;
}

/// Keeps COM initialised on the current thread until dropped. A thread already in the other
/// apartment mode (`RPC_E_CHANGED_MODE`) stays as it is; any other failure surfaces in the COM call
/// that follows. Not `Send`, so it is always released on the thread that initialised it.
#[cfg(windows)]
pub(crate) struct ComGuard(bool, std::marker::PhantomData<*const ()>);

#[cfg(windows)]
impl ComGuard {
    pub(crate) fn init(mode: windows::Win32::System::Com::COINIT) -> ComGuard {
        // SAFETY: the reserved pointer is None; a success is balanced by CoUninitialize in Drop.
        let hr = unsafe { windows::Win32::System::Com::CoInitializeEx(None, mode) };
        // S_OK and S_FALSE (already initialised) both succeed and both need a CoUninitialize.
        ComGuard(hr.is_ok(), std::marker::PhantomData)
    }
}

#[cfg(windows)]
impl Drop for ComGuard {
    fn drop(&mut self) {
        if self.0 {
            // SAFETY: balances this guard's successful CoInitializeEx, on the same thread.
            unsafe { windows::Win32::System::Com::CoUninitialize() };
        }
    }
}

/// Runs the real actions. The blocking ones go through `spawn_blocking`; a panic inside one
/// (a `JoinError`) maps to that action's own failure code.
pub struct SystemExecutor {
    pub obs: obs::ObsClient,
}

#[async_trait::async_trait]
impl Executor for SystemExecutor {
    async fn run(&self, action: &Action) -> Result<(), ActionError> {
        match action {
            Action::Launch { path, args, admin } => {
                let (path, args, admin) = (path.clone(), args.clone(), *admin);
                tokio::task::spawn_blocking(move || launch::launch(&path, &args, admin))
                    .await
                    .unwrap_or(Err(ActionError::LaunchFailed))
            }
            Action::Volume { op } => {
                let op = *op;
                tokio::task::spawn_blocking(move || volume::volume(op))
                    .await
                    .unwrap_or(Err(ActionError::VolumeFailed))
            }
            // No hotkey code fits a panic better; the chord was not (fully) sent.
            Action::Hotkey { keys } => {
                let keys = keys.clone();
                tokio::task::spawn_blocking(move || hotkey::send_chord(&keys))
                    .await
                    .unwrap_or(Err(ActionError::InvalidHotkey))
            }
            Action::ObsScene { scene } => self.obs.set_scene(scene).await,
        }
    }
}

pub const PRESS_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

/// Looks the button up in the current config and runs its stored action.
pub async fn press(state: &AppState, button_id: &str) -> Result<(), ActionError> {
    // Clone and release the lock so a slow action never blocks config access.
    let action = state.config.read().await.find_button(button_id).map(|b| b.action.clone());
    let result = match action {
        Some(a) => tokio::time::timeout(PRESS_TIMEOUT, state.executor.run(&a))
            .await
            .unwrap_or(Err(ActionError::Timeout)),
        None => Err(ActionError::UnknownButton),
    };
    if let Err(error) = result {
        state.events.emit(CoreEvent::ActionFailed { button_id: button_id.to_string(), error });
    }
    result
}

#[cfg(test)]
pub(crate) mod testing {
    use super::*;
    use crate::config::Config;
    use crate::state::EventSink;
    use std::sync::{Arc, Mutex};

    pub struct RecordingExecutor {
        pub ran: Mutex<Vec<Action>>,
        pub result: Result<(), ActionError>,
    }
    #[async_trait::async_trait]
    impl Executor for RecordingExecutor {
        async fn run(&self, action: &Action) -> Result<(), ActionError> {
            self.ran.lock().unwrap().push(action.clone());
            self.result
        }
    }

    #[derive(Default)]
    pub struct RecordingSink(pub Mutex<Vec<CoreEvent>>);
    impl EventSink for RecordingSink {
        fn emit(&self, e: CoreEvent) {
            self.0.lock().unwrap().push(e);
        }
    }

    pub struct Fixture {
        pub state: Arc<AppState>,
        pub exec: Arc<RecordingExecutor>,
        pub sink: Arc<RecordingSink>,
        pub _dir: tempfile::TempDir,
    }

    pub struct Hangs;
    #[async_trait::async_trait]
    impl Executor for Hangs {
        async fn run(&self, _: &Action) -> Result<(), ActionError> {
            std::future::pending().await
        }
    }

    pub struct Panics;
    #[async_trait::async_trait]
    impl Executor for Panics {
        async fn run(&self, _: &Action) -> Result<(), ActionError> {
            panic!("executor panicked (expected in test)")
        }
    }

    pub fn state_with(exec: Arc<dyn Executor>, sink: Arc<RecordingSink>) -> (Arc<AppState>, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let state = Arc::new(AppState::new(
            dir.path().to_path_buf(),
            Config::default_with_token("secret".into()),
            sink,
            |_| exec,
        ));
        (state, dir)
    }

    pub fn fixture(result: Result<(), ActionError>) -> Fixture {
        let exec = Arc::new(RecordingExecutor { ran: Mutex::new(vec![]), result });
        let sink = Arc::new(RecordingSink::default());
        let (state, dir) = state_with(exec.clone(), sink.clone());
        Fixture { state, exec, sink, _dir: dir }
    }
}

#[cfg(test)]
mod tests {
    use super::testing::fixture;
    use super::*;
    use crate::config::VolumeOp;
    use std::sync::Arc;

    async fn first_button_id(f: &testing::Fixture) -> String {
        f.state.config.read().await.pages[0].buttons[1].id.clone()
    }

    #[tokio::test]
    async fn press_runs_the_stored_action() {
        let f = fixture(Ok(()));
        let id = first_button_id(&f).await; // default slot 1 = Volume Up
        assert_eq!(press(&f.state, &id).await, Ok(()));
        assert_eq!(*f.exec.ran.lock().unwrap(), vec![Action::Volume { op: VolumeOp::Up }]);
    }

    #[tokio::test]
    async fn press_unknown_button_is_an_error() {
        let f = fixture(Ok(()));
        assert_eq!(press(&f.state, "nope").await, Err(ActionError::UnknownButton));
        assert!(f.exec.ran.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn failed_press_emits_action_failed() {
        let f = fixture(Err(ActionError::FileNotFound));
        let id = first_button_id(&f).await;
        assert_eq!(press(&f.state, &id).await, Err(ActionError::FileNotFound));
        assert_eq!(
            *f.sink.0.lock().unwrap(),
            vec![CoreEvent::ActionFailed { button_id: id, error: ActionError::FileNotFound }]
        );
    }

    #[tokio::test(start_paused = true)]
    async fn hanging_executor_times_out() {
        let sink = Arc::new(testing::RecordingSink::default());
        let (state, _d) = testing::state_with(Arc::new(testing::Hangs), sink.clone());
        let id = state.config.read().await.pages[0].buttons[0].id.clone();
        assert_eq!(press(&state, &id).await, Err(ActionError::Timeout));
        assert_eq!(
            *sink.0.lock().unwrap(),
            vec![CoreEvent::ActionFailed { button_id: id, error: ActionError::Timeout }]
        );
    }

    fn system_executor() -> SystemExecutor {
        let (_tx, rx) = tokio::sync::watch::channel(crate::config::ObsSettings {
            host: "127.0.0.1".into(),
            port: {
                let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
                l.local_addr().unwrap().port()
            },
            password: String::new(),
        });
        // `_tx` drops here: the OBS task exits and the client stays Disconnected.
        SystemExecutor { obs: obs::ObsClient::spawn(rx, Arc::new(testing::RecordingSink::default())) }
    }

    #[tokio::test]
    async fn executor_routes_obs_scene_to_obs_client() {
        let r = system_executor().run(&Action::ObsScene { scene: "Live".into() }).await;
        assert_eq!(r, Err(ActionError::ObsNotConnected));
    }

    #[tokio::test]
    async fn executor_routes_launch() {
        let r = system_executor()
            .run(&Action::Launch { path: "/definitely/not/here.exe".into(), args: vec![], admin: false })
            .await;
        assert_eq!(r, Err(ActionError::FileNotFound));
    }
}
