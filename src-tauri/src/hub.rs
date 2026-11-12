use crate::actions::ActionError;
use crate::config::{Grid, Page};
use crate::state::AppState;
use crate::themes::{self, Theme};
use serde::{Deserialize, Serialize};
use tokio::sync::{mpsc, Mutex};

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ServerMsg {
    Layout { pages: Vec<Page>, grid: Grid, language: String, theme: Theme },
    Result { press_id: u64, ok: bool, error: Option<ActionError> },
    Kicked,
}

/// The phone only ever says "button N was pressed"; nothing else is accepted.
#[derive(Deserialize, Debug)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClientMsg {
    Press { press_id: u64, button_id: String },
}

/// Layout from the current config; also used to push a fresh one after a config change.
pub async fn layout_msg(state: &AppState) -> ServerMsg {
    let c = state.config.read().await;
    ServerMsg::Layout {
        pages: c.pages.clone(),
        grid: c.settings.grid.clone(),
        language: c.settings.language.clone(),
        theme: themes::resolve(&state.data_dir, &c.settings.theme),
    }
}

/// At most one phone at a time: a new connection kicks the old one.
pub struct Hub {
    next_id: std::sync::atomic::AtomicU64,
    current: Mutex<Option<(u64, mpsc::UnboundedSender<ServerMsg>)>>,
}

impl Hub {
    pub fn new() -> Hub {
        Hub { next_id: 1.into(), current: Mutex::new(None) }
    }

    pub async fn attach(&self) -> (u64, mpsc::UnboundedReceiver<ServerMsg>) {
        let id = self.next_id.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let (tx, rx) = mpsc::unbounded_channel();
        if let Some((_, old)) = self.current.lock().await.replace((id, tx)) {
            let _ = old.send(ServerMsg::Kicked);
        }
        (id, rx)
    }

    /// Only removes `conn_id` if it is still the current connection.
    pub async fn detach(&self, conn_id: u64) {
        let mut cur = self.current.lock().await;
        if cur.as_ref().is_some_and(|(id, _)| *id == conn_id) {
            *cur = None;
        }
    }

    pub async fn send(&self, msg: ServerMsg) {
        if let Some((_, tx)) = &*self.current.lock().await {
            let _ = tx.send(msg);
        }
    }

    pub async fn kick(&self) {
        if let Some((_, tx)) = self.current.lock().await.take() {
            let _ = tx.send(ServerMsg::Kicked);
        }
    }

    pub async fn connected(&self) -> bool {
        self.current.lock().await.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn layout() -> ServerMsg {
        ServerMsg::Result { press_id: 1, ok: true, error: None }
    }

    #[tokio::test]
    async fn second_attach_kicks_first() {
        let hub = Hub::new();
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
        let hub = Hub::new();
        let (id1, _rx1) = hub.attach().await;
        let (_id2, _rx2) = hub.attach().await;
        hub.detach(id1).await;
        assert!(hub.connected().await);
    }

    #[tokio::test]
    async fn kick_disconnects() {
        let hub = Hub::new();
        let (_, mut rx) = hub.attach().await;
        hub.kick().await;
        assert_eq!(rx.recv().await, Some(ServerMsg::Kicked));
        assert!(!hub.connected().await);
    }
}
