use crate::actions::ActionError;
use crate::config::{Button, Grid, Page};
use crate::state::{AppState, CoreEvent, EventSink};
use crate::themes::{self, Theme};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex, MutexGuard};
use tokio::sync::mpsc;

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ServerMsg {
    /// `grid` and `theme` are the defaults; every page carries the grid and theme it is drawn with.
    /// `build` identifies the phone page this app serves.
    Layout { pages: Vec<LayoutPage>, grid: Grid, language: String, theme: Theme, build: String },
    Result { press_id: u64, ok: bool, error: Option<ActionError> },
    /// Every few seconds while a page has a usage key.
    Usage(crate::usage::Usage),
    Kicked,
}

/// The phone only ever says "button N was pressed"; nothing else is accepted.
#[derive(Deserialize, Debug)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClientMsg {
    Press { press_id: u64, button_id: String },
}

/// A page as the phone draws it: its own grid and theme resolved against the defaults.
#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct LayoutPage {
    pub id: String,
    pub name: String,
    pub buttons: Vec<Button>,
    pub grid: Grid,
    pub theme: Theme,
}

/// Layout from the current config; also used to push a fresh one after a config change.
pub async fn layout_msg(state: &AppState) -> ServerMsg {
    let c = state.config.read().await;
    let theme = themes::resolve(&state.data_dir, &c.settings.theme);
    let pages = c
        .pages
        .iter()
        .map(|p: &Page| LayoutPage {
            id: p.id.clone(),
            name: p.name.clone(),
            buttons: p.buttons.clone(),
            grid: c.page_grid(p).clone(),
            theme: p.theme.as_deref().map_or_else(|| theme.clone(), |id| themes::resolve(&state.data_dir, id)),
        })
        .collect();
    let (grid, language) = (c.settings.grid.clone(), c.settings.language.clone());
    ServerMsg::Layout { pages, grid, language, theme, build: crate::server::phone_build() }
}

/// At most one phone at a time: a new connection kicks the old one.
/// The hub owns `PhoneStatus` events and emits them under its lock, so they are
/// ordered with the state changes they describe.
pub struct Hub {
    next_id: std::sync::atomic::AtomicU64,
    events: Arc<dyn EventSink>,
    // Sync lock: nothing is awaited while held, so a Drop guard can detach.
    current: Mutex<Option<(u64, mpsc::UnboundedSender<ServerMsg>)>>,
}

impl Hub {
    pub fn new(events: Arc<dyn EventSink>) -> Hub {
        Hub { next_id: 1.into(), events, current: Mutex::new(None) }
    }

    fn lock(&self) -> MutexGuard<'_, Option<(u64, mpsc::UnboundedSender<ServerMsg>)>> {
        self.current.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub async fn attach(&self) -> (u64, mpsc::UnboundedReceiver<ServerMsg>) {
        let id = self.next_id.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let (tx, rx) = mpsc::unbounded_channel();
        let mut cur = self.lock();
        if let Some((_, old)) = cur.replace((id, tx)) {
            let _ = old.send(ServerMsg::Kicked);
        }
        self.events.emit(CoreEvent::PhoneStatus { connected: true });
        (id, rx)
    }

    /// Only removes `conn_id` if it is still the current connection.
    pub fn detach_now(&self, conn_id: u64) {
        let mut cur = self.lock();
        if cur.as_ref().is_some_and(|(id, _)| *id == conn_id) {
            *cur = None;
            self.events.emit(CoreEvent::PhoneStatus { connected: false });
        }
    }

    pub async fn detach(&self, conn_id: u64) {
        self.detach_now(conn_id)
    }

    pub async fn send(&self, msg: ServerMsg) {
        if let Some((_, tx)) = &*self.lock() {
            let _ = tx.send(msg);
        }
    }

    pub async fn kick(&self) {
        let mut cur = self.lock();
        if let Some((_, tx)) = cur.take() {
            let _ = tx.send(ServerMsg::Kicked);
            self.events.emit(CoreEvent::PhoneStatus { connected: false });
        }
    }

    pub async fn connected(&self) -> bool {
        self.lock().is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::actions::testing::RecordingSink;

    fn hub() -> (Hub, Arc<RecordingSink>) {
        let sink = Arc::new(RecordingSink::default());
        (Hub::new(sink.clone()), sink)
    }

    fn layout() -> ServerMsg {
        ServerMsg::Result { press_id: 1, ok: true, error: None }
    }

    #[tokio::test]
    async fn second_attach_kicks_first() {
        let (hub, _sink) = hub();
        let (_, mut rx1) = hub.attach().await;
        let (_, mut rx2) = hub.attach().await;
        assert_eq!(rx1.recv().await, Some(ServerMsg::Kicked));
        assert_eq!(rx1.recv().await, None);
        hub.send(layout()).await;
        assert_eq!(rx2.recv().await, Some(layout()));
        assert!(hub.connected().await);
    }

    #[tokio::test]
    async fn stale_detach_does_not_drop_current() {
        let (hub, _sink) = hub();
        let (id1, _rx1) = hub.attach().await;
        let (_id2, _rx2) = hub.attach().await;
        hub.detach(id1).await;
        assert!(hub.connected().await);
    }

    #[tokio::test]
    async fn kick_disconnects() {
        let (hub, _sink) = hub();
        let (_, mut rx) = hub.attach().await;
        hub.kick().await;
        assert_eq!(rx.recv().await, Some(ServerMsg::Kicked));
        assert!(!hub.connected().await);
    }

    #[tokio::test]
    async fn status_events_follow_state_changes() {
        let (hub, sink) = hub();
        let t = CoreEvent::PhoneStatus { connected: true };
        let f = CoreEvent::PhoneStatus { connected: false };
        let (a, _ra) = hub.attach().await;
        hub.detach(a).await;
        let (_b, _rb) = hub.attach().await;
        hub.detach(a).await; // stale: no event
        assert_eq!(*sink.0.lock().unwrap(), vec![t.clone(), f.clone(), t.clone()]);
        hub.kick().await;
        hub.kick().await; // nothing connected: no event
        assert_eq!(*sink.0.lock().unwrap(), vec![t.clone(), f.clone(), t, f]);
    }
}
