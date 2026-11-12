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
    UnsupportedPlatform,
}

#[async_trait::async_trait]
pub trait Executor: Send + Sync {
    async fn run(&self, action: &Action) -> Result<(), ActionError>;
}

/// Looks the button up in the current config and runs its stored action.
pub async fn press(state: &AppState, button_id: &str) -> Result<(), ActionError> {
    // Clone and release the lock so a slow action never blocks config access.
    let action = state.config.read().await.find_button(button_id).map(|b| b.action.clone());
    let result = match action {
        Some(a) => state.executor.run(&a).await,
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
    use crate::hub::Hub;
    use crate::state::EventSink;
    use std::sync::{atomic::AtomicU16, Arc, Mutex};

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

    pub fn fixture(result: Result<(), ActionError>) -> Fixture {
        let dir = tempfile::tempdir().unwrap();
        let exec = Arc::new(RecordingExecutor { ran: Mutex::new(vec![]), result });
        let sink = Arc::new(RecordingSink::default());
        let state = Arc::new(AppState {
            data_dir: dir.path().to_path_buf(),
            config: tokio::sync::RwLock::new(Config::default_with_token("secret".into())),
            port: AtomicU16::new(0),
            hub: Hub::new(),
            executor: exec.clone(),
            events: sink.clone(),
        });
        Fixture { state, exec, sink, _dir: dir }
    }
}

#[cfg(test)]
mod tests {
    use super::testing::fixture;
    use super::*;
    use crate::config::VolumeOp;

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
}
