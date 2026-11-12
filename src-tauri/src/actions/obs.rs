use crate::actions::ActionError;
use crate::config::ObsSettings;
use crate::state::{CoreEvent, EventSink, ObsState};
use obws::responses::StatusCode;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::watch;

pub const RETRY_INTERVAL: Duration = Duration::from_secs(3);
const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
const AUTH_FAILED_CLOSE_CODE: u16 = 4009;

struct Shared {
    state: Mutex<ObsState>,
    client: Mutex<Option<Arc<obws::Client>>>,
    events: Arc<dyn EventSink>,
}

impl Shared {
    /// Updates the state and emits only on a real change. Never emits under the lock.
    fn set_state(&self, new: ObsState) {
        let changed = {
            let mut s = self.state.lock().unwrap();
            let changed = *s != new;
            *s = new;
            changed
        };
        if changed {
            self.events.emit(CoreEvent::ObsStatus { state: new });
        }
    }

    fn set_client(&self, c: Option<Arc<obws::Client>>) {
        *self.client.lock().unwrap() = c;
    }
}

#[derive(Clone)]
pub struct ObsClient(Arc<Shared>);

impl ObsClient {
    pub fn spawn(settings: watch::Receiver<ObsSettings>, events: Arc<dyn EventSink>) -> ObsClient {
        Self::spawn_with_interval(settings, events, RETRY_INTERVAL)
    }

    /// `interval` is both the reconnect delay and the liveness-check period of a live connection.
    fn spawn_with_interval(
        mut settings: watch::Receiver<ObsSettings>,
        events: Arc<dyn EventSink>,
        interval: Duration,
    ) -> ObsClient {
        let shared = Arc::new(Shared {
            state: Mutex::new(ObsState::Disconnected),
            client: Mutex::new(None),
            events,
        });
        let task = shared.clone();
        tokio::spawn(async move {
            loop {
                let current = settings.borrow_and_update().clone();
                tokio::select! {
                    _ = settings.changed() => {}
                    _ = session(&task, &current, interval) => {}
                }
                // Settings changed (or the sender is gone): drop the old connection.
                task.set_client(None);
                task.set_state(ObsState::Disconnected);
                if settings.has_changed().is_err() {
                    return;
                }
            }
        });
        ObsClient(shared)
    }

    pub fn state(&self) -> ObsState {
        *self.0.state.lock().unwrap()
    }

    fn connected(&self) -> Result<Arc<obws::Client>, ActionError> {
        self.0.client.lock().unwrap().clone().ok_or(ActionError::ObsNotConnected)
    }

    pub async fn set_scene(&self, scene: &str) -> Result<(), ActionError> {
        let client = self.connected()?;
        client.scenes().set_current_program_scene(scene).await.map_err(|e| match e {
            obws::error::Error::Api { code: StatusCode::ResourceNotFound, .. } => ActionError::ObsSceneMissing,
            _ => ActionError::ObsNotConnected,
        })
    }

    pub async fn scenes(&self) -> Result<Vec<String>, ActionError> {
        let client = self.connected()?;
        let list = client.scenes().list().await.map_err(|_| ActionError::ObsNotConnected)?;
        Ok(list.scenes.into_iter().map(|s| s.id.name).collect())
    }
}

fn is_auth_failure(e: &obws::error::Error) -> bool {
    use obws::client::HandshakeError;
    matches!(
        e,
        obws::error::Error::Handshake(HandshakeError::ConnectionClosed(Some(d)))
            if u16::from(d.code) == AUTH_FAILED_CLOSE_CODE
    )
}

/// Connects, then watches the connection; reconnects every `interval`. Never returns: the caller
/// cancels it when the settings change.
async fn session(shared: &Shared, s: &ObsSettings, interval: Duration) {
    loop {
        let password = (!s.password.is_empty()).then_some(s.password.as_str());
        let connect = tokio::time::timeout(CONNECT_TIMEOUT, obws::Client::connect(&s.host, s.port, password));
        match connect.await {
            Ok(Ok(client)) => {
                let client = Arc::new(client);
                shared.set_client(Some(client.clone()));
                shared.set_state(ObsState::Connected);
                // obws without its `events` feature has no drop notification; probe instead.
                loop {
                    tokio::time::sleep(interval).await;
                    if client.general().version().await.is_err() {
                        break;
                    }
                }
                shared.set_client(None);
                shared.set_state(ObsState::Disconnected);
            }
            Ok(Err(e)) if is_auth_failure(&e) => {
                shared.set_state(ObsState::AuthFailed);
                std::future::pending::<()>().await; // wait for new settings
            }
            _ => shared.set_state(ObsState::Disconnected),
        }
        tokio::time::sleep(interval).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::actions::testing::RecordingSink;
    use crate::config::ObsSettings;
    use std::sync::Arc;
    use std::time::Duration;

    const FAST: Duration = Duration::from_millis(40);

    fn closed_port() -> u16 {
        let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        l.local_addr().unwrap().port()
    }

    fn settings(port: u16) -> ObsSettings {
        ObsSettings { host: "127.0.0.1".into(), port, password: "pw".into() }
    }

    async fn wait_for(mut f: impl FnMut() -> bool) {
        for _ in 0..100 {
            if f() {
                return;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        panic!("condition not reached in time");
    }

    #[tokio::test]
    async fn set_scene_without_obs_is_not_connected() {
        let sink = Arc::new(RecordingSink::default());
        let (_tx, rx) = tokio::sync::watch::channel(settings(closed_port()));
        let obs = ObsClient::spawn_with_interval(rx, sink.clone(), FAST);
        tokio::time::sleep(FAST * 4).await; // several failed retries
        assert_eq!(obs.set_scene("Live").await, Err(ActionError::ObsNotConnected));
        assert_eq!(obs.scenes().await, Err(ActionError::ObsNotConnected));
        assert_eq!(obs.state(), ObsState::Disconnected);
        // Retries must not re-announce an unchanged state.
        assert!(sink.0.lock().unwrap().is_empty());
    }

    /// Fake obs-websocket that says Hello, then closes with 4009 (authentication failed).
    async fn fake_auth_failing_obs() -> (u16, Arc<std::sync::atomic::AtomicUsize>) {
        use futures_util::{SinkExt, StreamExt};
        use tokio_tungstenite::tungstenite::protocol::{frame::coding::CloseCode, CloseFrame};
        use tokio_tungstenite::tungstenite::Message;
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let accepts = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let n = accepts.clone();
        tokio::spawn(async move {
            loop {
                let (sock, _) = listener.accept().await.unwrap();
                n.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                let mut ws = tokio_tungstenite::accept_async(sock).await.unwrap();
                let hello = r#"{"op":0,"d":{"obsWebSocketVersion":"5.5.0","rpcVersion":1,"authentication":{"challenge":"c","salt":"s"}}}"#;
                ws.send(Message::text(hello)).await.unwrap();
                let _ = ws.next().await; // Identify
                let frame = CloseFrame { code: CloseCode::from(4009), reason: "auth".into() };
                let _ = ws.send(Message::Close(Some(frame))).await;
            }
        });
        (port, accepts)
    }

    #[tokio::test]
    async fn auth_failure_stops_retrying_until_settings_change() {
        use std::sync::atomic::Ordering::SeqCst;
        let (port, accepts) = fake_auth_failing_obs().await;
        let sink = Arc::new(RecordingSink::default());
        let (tx, rx) = tokio::sync::watch::channel(settings(port));
        let obs = ObsClient::spawn_with_interval(rx, sink.clone(), FAST);
        wait_for(|| obs.state() == ObsState::AuthFailed).await;
        tokio::time::sleep(FAST * 5).await;
        assert_eq!(accepts.load(SeqCst), 1, "must not retry after AuthFailed");
        assert_eq!(obs.set_scene("Live").await, Err(ActionError::ObsNotConnected));

        tx.send(ObsSettings { password: "other".into(), ..settings(port) }).unwrap();
        wait_for(|| accepts.load(SeqCst) == 2).await;
        wait_for(|| obs.state() == ObsState::AuthFailed).await;
        assert_eq!(
            *sink.0.lock().unwrap(),
            vec![
                CoreEvent::ObsStatus { state: ObsState::AuthFailed },
                CoreEvent::ObsStatus { state: ObsState::Disconnected },
                CoreEvent::ObsStatus { state: ObsState::AuthFailed },
            ]
        );
    }

    /// Needs a real OBS with the WebSocket server on and two scenes:
    /// `OBS_PASSWORD=... cargo test obs_live_switch -- --ignored`
    #[tokio::test]
    #[ignore]
    async fn obs_live_switch() {
        let password = std::env::var("OBS_PASSWORD").unwrap_or_default();
        let sink = Arc::new(RecordingSink::default());
        let (_tx, rx) = tokio::sync::watch::channel(ObsSettings { host: "127.0.0.1".into(), port: 4455, password });
        let obs = ObsClient::spawn(rx, sink);
        wait_for(|| obs.state() == ObsState::Connected).await;
        let scenes = obs.scenes().await.unwrap();
        assert!(scenes.len() >= 2, "create two scenes in OBS first: {scenes:?}");
        for s in &scenes {
            obs.set_scene(s).await.unwrap();
            tokio::time::sleep(Duration::from_millis(500)).await;
        }
        assert_eq!(obs.set_scene("definitely-not-a-scene").await, Err(ActionError::ObsSceneMissing));
    }
}
