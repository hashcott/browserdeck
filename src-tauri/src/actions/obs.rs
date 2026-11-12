use crate::actions::ActionError;
use crate::config::ObsSettings;
use crate::state::{CoreEvent, EventSink, ObsState};
use obws::responses::StatusCode;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::watch;

pub const RETRY_INTERVAL: Duration = Duration::from_secs(3);
/// Bounds connect+handshake, the liveness probe and every request, so a hung OBS never stalls us.
pub const REQUEST_TIMEOUT: Duration = Duration::from_secs(5);
const AUTH_FAILED_CLOSE_CODE: u16 = 4009;

struct Shared {
    state: Mutex<ObsState>,
    client: Mutex<Option<Arc<obws::Client>>>,
    /// Signals the background task (the only writer of `state`) that a request found the link dead.
    lost: tokio::sync::Notify,
    request_timeout: Duration,
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
        Self::spawn_with_timing(settings, events, RETRY_INTERVAL, REQUEST_TIMEOUT)
    }

    /// `interval` is both the reconnect delay and the liveness-check period of a live connection.
    fn spawn_with_timing(
        mut settings: watch::Receiver<ObsSettings>,
        events: Arc<dyn EventSink>,
        interval: Duration,
        request_timeout: Duration,
    ) -> ObsClient {
        let shared = Arc::new(Shared {
            state: Mutex::new(ObsState::Disconnected),
            client: Mutex::new(None),
            lost: tokio::sync::Notify::new(),
            request_timeout,
            events,
        });
        let task = shared.clone();
        // Ends when the settings sender is dropped: production must keep it alive for the app's lifetime.
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
        let r = tokio::time::timeout(self.0.request_timeout, client.scenes().set_current_program_scene(scene)).await;
        self.finish(&client, r)
    }

    pub async fn scenes(&self) -> Result<Vec<String>, ActionError> {
        let client = self.connected()?;
        let r = tokio::time::timeout(self.0.request_timeout, client.scenes().list()).await;
        let list = self.finish(&client, r)?;
        Ok(list.scenes.into_iter().map(|s| s.id.name).collect())
    }
}

impl ObsClient {
    /// Maps a request outcome; a dead link (transport error or timeout) is reported to the task.
    fn finish<T>(
        &self,
        client: &Arc<obws::Client>,
        r: Result<obws::error::Result<T>, tokio::time::error::Elapsed>,
    ) -> Result<T, ActionError> {
        let error = match r {
            Ok(Ok(v)) => return Ok(v),
            Ok(Err(e)) => map_error(&e),
            Err(_) => ActionError::ObsNotConnected,
        };
        if error == ActionError::ObsNotConnected {
            let current = self.0.client.lock().unwrap().clone();
            // Only the connection that failed counts, not a newer one.
            if current.is_some_and(|c| Arc::ptr_eq(&c, client)) {
                self.0.lost.notify_one();
            }
        }
        Err(error)
    }
}

/// OBS answered with an error -> request failed; nothing answered -> not connected.
fn map_error(e: &obws::error::Error) -> ActionError {
    match e {
        obws::error::Error::Api { code: StatusCode::ResourceNotFound, .. } => ActionError::ObsSceneMissing,
        obws::error::Error::Api { .. } => ActionError::ObsRequestFailed,
        _ => ActionError::ObsNotConnected,
    }
}

/// With no password obws sends `Identify` without credentials and a server that requires auth
/// closes the socket with 4009 (`HandshakeError::ConnectionClosed`), same as a wrong password.
fn is_auth_failure(e: &obws::error::Error) -> bool {
    use obws::client::HandshakeError;
    matches!(
        e,
        obws::error::Error::Handshake(HandshakeError::ConnectionClosed(Some(d)))
            if u16::from(d.code) == AUTH_FAILED_CLOSE_CODE
    )
}

/// `obws` refuses OBS Studio < 30.2 and obs-websocket outside ^5.5 right after the handshake.
fn is_unsupported_version(e: &obws::error::Error) -> bool {
    matches!(e, obws::error::Error::ObsStudioVersion(..) | obws::error::Error::ObsWebsocketVersion(..))
}

/// Connects, then watches the connection; reconnects every `interval`. Never returns: the caller
/// cancels it when the settings change.
async fn session(shared: &Shared, s: &ObsSettings, interval: Duration) {
    loop {
        let password = (!s.password.is_empty()).then_some(s.password.as_str());
        let connect = tokio::time::timeout(shared.request_timeout, obws::Client::connect(&s.host, s.port, password));
        match connect.await {
            Ok(Ok(client)) => {
                let client = Arc::new(client);
                shared.set_client(Some(client.clone()));
                shared.set_state(ObsState::Connected);
                // obws without its `events` feature has no drop notification; probe instead.
                loop {
                    tokio::select! {
                        _ = shared.lost.notified() => break,
                        _ = tokio::time::sleep(interval) => {}
                    }
                    let probe = tokio::time::timeout(shared.request_timeout, client.general().version()).await;
                    if !matches!(probe, Ok(Ok(_))) {
                        break;
                    }
                }
                shared.set_client(None);
                // Drop a signal that raced with the break so it cannot kill the next connection.
                let _ = tokio::time::timeout(Duration::ZERO, shared.lost.notified()).await;
                shared.set_state(ObsState::Disconnected);
            }
            Ok(Err(e)) if is_auth_failure(&e) => {
                shared.set_state(ObsState::AuthFailed);
                std::future::pending::<()>().await; // wait for new settings
            }
            // Keep retrying at the normal interval: the user may upgrade OBS.
            Ok(Err(e)) if is_unsupported_version(&e) => shared.set_state(ObsState::Unsupported),
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
    use std::sync::{Arc, Mutex};
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
        let obs = ObsClient::spawn_with_timing(rx, sink.clone(), FAST, FAST * 3);
        tokio::time::sleep(FAST * 4).await; // several failed retries
        assert_eq!(obs.set_scene("Live").await, Err(ActionError::ObsNotConnected));
        assert_eq!(obs.scenes().await, Err(ActionError::ObsNotConnected));
        assert_eq!(obs.state(), ObsState::Disconnected);
        // Retries must not re-announce an unchanged state.
        assert!(sink.0.lock().unwrap().is_empty());
    }

    /// Fake obs-websocket that says Hello, then closes with 4009 (authentication failed).
    async fn fake_auth_failing_obs() -> (u16, Arc<std::sync::atomic::AtomicUsize>, Arc<Mutex<Vec<String>>>) {
        use futures_util::{SinkExt, StreamExt};
        use tokio_tungstenite::tungstenite::protocol::{frame::coding::CloseCode, CloseFrame};
        use tokio_tungstenite::tungstenite::Message;
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let accepts = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let n = accepts.clone();
        let identifies = Arc::new(Mutex::new(vec![]));
        let seen = identifies.clone();
        tokio::spawn(async move {
            loop {
                let (sock, _) = listener.accept().await.unwrap();
                n.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                let mut ws = tokio_tungstenite::accept_async(sock).await.unwrap();
                let hello = r#"{"op":0,"d":{"obsWebSocketVersion":"5.5.0","rpcVersion":1,"authentication":{"challenge":"c","salt":"s"}}}"#;
                ws.send(Message::text(hello)).await.unwrap();
                if let Some(Ok(m)) = ws.next().await {
                    seen.lock().unwrap().push(m.to_string());
                } // Identify
                let frame = CloseFrame { code: CloseCode::from(4009), reason: "auth".into() };
                let _ = ws.send(Message::Close(Some(frame))).await;
            }
        });
        (port, accepts, identifies)
    }

    #[tokio::test]
    async fn auth_failure_stops_retrying_until_settings_change() {
        use std::sync::atomic::Ordering::SeqCst;
        let (port, accepts, _) = fake_auth_failing_obs().await;
        let sink = Arc::new(RecordingSink::default());
        let (tx, rx) = tokio::sync::watch::channel(settings(port));
        let obs = ObsClient::spawn_with_timing(rx, sink.clone(), FAST, FAST * 3);
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

    #[tokio::test]
    async fn empty_password_against_auth_server_is_auth_failed_and_not_retried() {
        use std::sync::atomic::Ordering::SeqCst;
        let (port, accepts, identifies) = fake_auth_failing_obs().await;
        let (_tx, rx) = tokio::sync::watch::channel(ObsSettings { password: String::new(), ..settings(port) });
        let obs = ObsClient::spawn_with_timing(rx, Arc::new(RecordingSink::default()), FAST, FAST * 3);
        wait_for(|| obs.state() == ObsState::AuthFailed).await;
        tokio::time::sleep(FAST * 5).await;
        assert_eq!(accepts.load(SeqCst), 1);
        assert!(!identifies.lock().unwrap()[0].contains("authentication"), "no credentials were sent");
    }

    /// Completes the handshake (no auth), then reads forever and never answers a request.
    async fn fake_hanging_obs() -> u16 {
        use futures_util::{SinkExt, StreamExt};
        use tokio_tungstenite::tungstenite::Message;
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        tokio::spawn(async move {
            loop {
                let (sock, _) = listener.accept().await.unwrap();
                tokio::spawn(async move {
                    let mut ws = tokio_tungstenite::accept_async(sock).await.unwrap();
                    let hello = r#"{"op":0,"d":{"obsWebSocketVersion":"5.5.0","rpcVersion":1}}"#;
                    ws.send(Message::text(hello)).await.unwrap();
                    let _ = ws.next().await; // Identify
                    ws.send(Message::text(r#"{"op":2,"d":{"negotiatedRpcVersion":1}}"#)).await.unwrap();
                    // obws verifies versions with one GetVersion request right after the handshake.
                    let req: serde_json::Value = serde_json::from_str(&ws.next().await.unwrap().unwrap().into_text().unwrap()).unwrap();
                    let reply = serde_json::json!({"op": 7, "d": {
                        "requestType": "GetVersion", "requestId": req["d"]["requestId"],
                        "requestStatus": {"result": true, "code": 100},
                        "responseData": {"obsVersion": "31.0.0", "obsWebSocketVersion": "5.5.0", "rpcVersion": 1,
                            "availableRequests": [], "supportedImageFormats": [], "platform": "test", "platformDescription": "test"}}});
                    ws.send(Message::text(reply.to_string())).await.unwrap();
                    while ws.next().await.is_some() {}
                });
            }
        });
        port
    }

    #[tokio::test]
    async fn unresponsive_obs_times_out_and_is_marked_disconnected() {
        let sink = Arc::new(RecordingSink::default());
        let (_tx, rx) = tokio::sync::watch::channel(settings(fake_hanging_obs().await));
        let obs = ObsClient::spawn_with_timing(rx, sink.clone(), FAST, FAST * 3);
        wait_for(|| obs.state() == ObsState::Connected).await;
        assert_eq!(obs.set_scene("Live").await, Err(ActionError::ObsNotConnected));
        wait_for(|| sink.0.lock().unwrap().len() >= 2).await;
        assert_eq!(
            sink.0.lock().unwrap()[..2],
            [
                CoreEvent::ObsStatus { state: ObsState::Connected },
                CoreEvent::ObsStatus { state: ObsState::Disconnected },
            ]
        );
    }

    #[test]
    fn error_mapping_separates_obs_errors_from_transport_errors() {
        use obws::error::Error;
        let api = |code| Error::Api { code, message: None };
        assert_eq!(map_error(&api(StatusCode::ResourceNotFound)), ActionError::ObsSceneMissing);
        assert_eq!(map_error(&api(StatusCode::RequestProcessingFailed)), ActionError::ObsRequestFailed);
        assert_eq!(map_error(&Error::Disconnected), ActionError::ObsNotConnected);
        assert_eq!(map_error(&Error::Timeout), ActionError::ObsNotConnected);
    }

    #[test]
    fn version_mismatch_errors_are_unsupported_others_are_not() {
        use obws::error::Error;
        let req = semver::Comparator { op: semver::Op::GreaterEq, major: 30, minor: Some(2), patch: None, pre: semver::Prerelease::EMPTY };
        let old = semver::Version::new(29, 0, 0);
        assert!(is_unsupported_version(&Error::ObsStudioVersion(old.clone(), req.clone())));
        assert!(is_unsupported_version(&Error::ObsWebsocketVersion(old, req)));
        assert!(!is_unsupported_version(&Error::Disconnected));
        assert!(!is_unsupported_version(&Error::Timeout));
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
