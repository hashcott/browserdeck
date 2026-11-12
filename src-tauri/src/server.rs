use crate::actions;
use crate::auth;
use crate::config::is_icon_file_name;
use crate::hub::{layout_msg, ClientMsg, ServerMsg};
use crate::state::{AppState, CoreEvent};
use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        Path, Request, State,
    },
    http::{header, HeaderValue, StatusCode},
    middleware::{self, Next},
    response::{Html, IntoResponse, Response},
    routing::get,
    Router,
};
use rust_embed::RustEmbed;
use std::io;
use std::sync::Arc;
use tokio::net::TcpListener;

pub const DEFAULT_PORT: u16 = 47821;

const NOT_PAIRED: &str = "<!doctype html><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width,initial-scale=1\"><title>Not paired</title><p>Not paired. Scan the QR code in the desktop app.</p><p>Chưa ghép. Hãy quét mã QR trong app trên máy tính.</p>";

#[derive(RustEmbed)]
#[folder = "../dist"]
struct Dist;

/// Tries `preferred..=preferred+9` on all interfaces.
pub async fn bind(preferred: u16) -> io::Result<(TcpListener, u16)> {
    let mut last = io::Error::new(io::ErrorKind::AddrInUse, "no free port");
    for port in preferred..=preferred.saturating_add(9) {
        match TcpListener::bind(("0.0.0.0", port)).await {
            Ok(l) => return Ok((l, port)),
            Err(e) => last = e,
        }
    }
    Err(last)
}

pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/", get(index))
        .route("/assets/{*path}", get(asset))
        .route("/icons/{file}", get(icon))
        .route("/ws", get(ws_upgrade))
        .layer(middleware::from_fn_with_state(state.clone(), gate))
        .with_state(state)
}

fn query_token(query: &str) -> Option<&str> {
    query.split('&').find_map(|kv| kv.strip_prefix("token="))
}

fn cookie_token(cookies: &str) -> Option<&str> {
    cookies.split(';').find_map(|kv| kv.trim().strip_prefix("sd_token="))
}

/// Runs before routing touches anything: every request needs the pairing token.
async fn gate(State(state): State<Arc<AppState>>, req: Request, next: Next) -> Response {
    let from_query = req.uri().query().and_then(query_token);
    let from_cookie = req
        .headers()
        .get(header::COOKIE)
        .and_then(|v| v.to_str().ok())
        .and_then(cookie_token);
    let expected = state.config.read().await.settings.token.clone();
    let Some(given) = from_query.or(from_cookie) else { return not_paired() };
    if !auth::verify(&expected, given) {
        return not_paired();
    }
    // Valid token in the URL on the page: move it into a cookie and out of the address bar.
    if from_query.is_some() && req.uri().path() == "/" {
        let cookie = format!("sd_token={expected}; HttpOnly; SameSite=Strict; Path=/; Max-Age=31536000");
        return (
            StatusCode::SEE_OTHER,
            [(header::LOCATION, "/".to_string()), (header::SET_COOKIE, cookie)],
        )
            .into_response();
    }
    next.run(req).await
}

fn not_paired() -> Response {
    (StatusCode::UNAUTHORIZED, Html(NOT_PAIRED)).into_response()
}

fn content_type(name: &str) -> &'static str {
    match name.rsplit_once('.').map(|(_, e)| e) {
        Some("html") => "text/html; charset=utf-8",
        Some("js") => "text/javascript; charset=utf-8",
        Some("css") => "text/css; charset=utf-8",
        Some("svg") => "image/svg+xml",
        Some("png") => "image/png",
        Some("jpg" | "jpeg") => "image/jpeg",
        Some("webp") => "image/webp",
        Some("json") => "application/json",
        Some("woff2") => "font/woff2",
        _ => "application/octet-stream",
    }
}

fn file_response(name: &str, bytes: Vec<u8>) -> Response {
    ([(header::CONTENT_TYPE, HeaderValue::from_static(content_type(name)))], bytes).into_response()
}

async fn index() -> Response {
    match Dist::get("phone.html") {
        Some(f) => file_response("phone.html", f.data.into_owned()),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

async fn asset(Path(path): Path<String>) -> Response {
    // rust-embed only knows its own keys, so `..` segments simply miss.
    match Dist::get(&format!("assets/{path}")) {
        Some(f) => file_response(&path, f.data.into_owned()),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

async fn icon(State(state): State<Arc<AppState>>, Path(file): Path<String>) -> Response {
    if !is_icon_file_name(&file) {
        return StatusCode::NOT_FOUND.into_response();
    }
    match tokio::fs::read(state.data_dir.join("icons").join(&file)).await {
        Ok(bytes) => file_response(&file, bytes),
        Err(_) => StatusCode::NOT_FOUND.into_response(),
    }
}

const MAX_WS_MESSAGE: usize = 4096;

async fn ws_upgrade(State(state): State<Arc<AppState>>, ws: WebSocketUpgrade) -> Response {
    ws.max_message_size(MAX_WS_MESSAGE).on_upgrade(move |socket| phone_session(state, socket))
}

async fn send_msg(socket: &mut WebSocket, msg: &ServerMsg) -> bool {
    let Ok(json) = serde_json::to_string(msg) else { return false };
    socket.send(Message::Text(json.into())).await.is_ok()
}

async fn phone_session(state: Arc<AppState>, mut socket: WebSocket) {
    let (conn_id, mut rx) = state.hub.attach().await;
    state.events.emit(CoreEvent::PhoneStatus { connected: true });
    if send_msg(&mut socket, &layout_msg(&state).await).await {
        loop {
            tokio::select! {
                out = rx.recv() => match out {
                    Some(msg) => {
                        let kicked = msg == ServerMsg::Kicked;
                        if !send_msg(&mut socket, &msg).await || kicked { break }
                    }
                    None => break,
                },
                inc = socket.recv() => match inc {
                    Some(Ok(Message::Text(t))) => {
                        // Garbage (or any type other than `press`) is ignored, connection stays open.
                        let Ok(ClientMsg::Press { press_id, button_id }) = serde_json::from_str(&t) else { continue };
                        let error = actions::press(&state, &button_id).await.err();
                        let res = ServerMsg::Result { press_id, ok: error.is_none(), error };
                        if !send_msg(&mut socket, &res).await { break }
                    }
                    Some(Ok(Message::Close(_))) | Some(Err(_)) | None => break,
                    Some(Ok(_)) => {}
                },
            }
        }
    }
    state.hub.detach(conn_id).await;
    // A kicked old connection closing must not report "disconnected" while a new phone is attached.
    if !state.hub.connected().await {
        state.events.emit(CoreEvent::PhoneStatus { connected: false });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::actions::{ActionError, Executor};
    use crate::config::{Action, Config};
    use crate::hub::Hub;
    use crate::state::{CoreEvent, EventSink};
    use axum::body::Body;
    use axum::http::Request;
    use http_body_util::BodyExt;
    use std::sync::atomic::AtomicU16;
    use tower::ServiceExt;

    struct OkExec;
    #[async_trait::async_trait]
    impl Executor for OkExec {
        async fn run(&self, _: &Action) -> Result<(), ActionError> {
            Ok(())
        }
    }
    struct NullSink;
    impl EventSink for NullSink {
        fn emit(&self, _: CoreEvent) {}
    }

    const ICON: &str = "01234567-89ab-cdef-0123-456789abcdef.png";

    fn app() -> (Router, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let state = Arc::new(AppState {
            data_dir: dir.path().to_path_buf(),
            config: tokio::sync::RwLock::new(Config::default_with_token("secret".into())),
            port: AtomicU16::new(0),
            hub: Hub::new(),
            executor: Arc::new(OkExec),
            events: Arc::new(NullSink),
        });
        (router(state), dir)
    }

    async fn get(app: &Router, uri: &str, cookie: Option<&str>) -> Response {
        let mut r = Request::builder().uri(uri);
        if let Some(c) = cookie {
            r = r.header(header::COOKIE, c);
        }
        app.clone().oneshot(r.body(Body::empty()).unwrap()).await.unwrap()
    }

    async fn body(r: Response) -> Vec<u8> {
        r.into_body().collect().await.unwrap().to_bytes().to_vec()
    }

    const OK: Option<&str> = Some("sd_token=secret");

    #[tokio::test]
    async fn no_token_gets_401_not_paired_page() {
        let (app, _d) = app();
        let r = get(&app, "/", None).await;
        assert_eq!(r.status(), StatusCode::UNAUTHORIZED);
        let b = String::from_utf8(body(r).await).unwrap();
        assert!(b.contains("Not paired") && b.contains("Chưa ghép"));
    }

    #[tokio::test]
    async fn wrong_token_gets_401() {
        let (app, _d) = app();
        assert_eq!(get(&app, "/?token=nope", None).await.status(), StatusCode::UNAUTHORIZED);
        assert_eq!(get(&app, "/", Some("sd_token=nope")).await.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn empty_configured_token_never_authenticates() {
        let (_, d) = app();
        let state = Arc::new(AppState {
            data_dir: d.path().to_path_buf(),
            config: tokio::sync::RwLock::new(Config::default_with_token(String::new())),
            port: AtomicU16::new(0),
            hub: Hub::new(),
            executor: Arc::new(OkExec),
            events: Arc::new(NullSink),
        });
        let app = router(state);
        assert_eq!(get(&app, "/?token=", None).await.status(), StatusCode::UNAUTHORIZED);
        assert_eq!(get(&app, "/", Some("sd_token=")).await.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn right_query_token_redirects_and_sets_cookie() {
        let (app, _d) = app();
        let r = get(&app, "/?token=secret", None).await;
        assert_eq!(r.status(), StatusCode::SEE_OTHER);
        assert_eq!(r.headers()[header::LOCATION], "/");
        let c = r.headers()[header::SET_COOKIE].to_str().unwrap();
        assert!(c.contains("sd_token=secret") && c.contains("HttpOnly") && c.contains("SameSite=Strict"));
    }

    #[tokio::test]
    async fn cookie_token_serves_phone_page() {
        let (app, _d) = app();
        let r = get(&app, "/", OK).await;
        assert_eq!(r.status(), StatusCode::OK);
        assert!(r.headers()[header::CONTENT_TYPE].to_str().unwrap().starts_with("text/html"));
    }

    #[tokio::test]
    async fn assets_and_icons_need_token() {
        let (app, _d) = app();
        assert_eq!(get(&app, "/assets/x.js", None).await.status(), StatusCode::UNAUTHORIZED);
        assert_eq!(get(&app, &format!("/icons/{ICON}"), None).await.status(), StatusCode::UNAUTHORIZED);
        assert_eq!(get(&app, "/nowhere", None).await.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn icon_is_served_from_data_dir() {
        let (app, d) = app();
        std::fs::create_dir(d.path().join("icons")).unwrap();
        std::fs::write(d.path().join("icons").join(ICON), b"png-bytes").unwrap();
        let r = get(&app, &format!("/icons/{ICON}"), OK).await;
        assert_eq!(r.status(), StatusCode::OK);
        assert_eq!(r.headers()[header::CONTENT_TYPE], "image/png");
        assert_eq!(body(r).await, b"png-bytes");
    }

    #[tokio::test]
    async fn icon_traversal_is_404() {
        let (app, d) = app();
        std::fs::write(d.path().join("config.json"), "{}").unwrap();
        for uri in ["/icons/..%2Fconfig.json", "/icons/config.json", "/assets/..%2F..%2FCargo.toml"] {
            assert_eq!(get(&app, uri, OK).await.status(), StatusCode::NOT_FOUND, "{uri}");
        }
    }

    #[tokio::test]
    async fn bind_falls_back_when_port_busy() {
        let (_held, p) = bind(48100).await.unwrap();
        let (_l, got) = bind(p).await.unwrap();
        assert!((p + 1..=p + 9).contains(&got));
    }

    mod ws {
        use super::*;
        use crate::actions::testing::{fixture, Fixture};
        use futures_util::{SinkExt, StreamExt};
        use serde_json::{json, Value};
        use tokio_tungstenite::tungstenite::{client::IntoClientRequest, Message as M};

        type Client = tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

        async fn serve(f: &Fixture) -> u16 {
            let l = TcpListener::bind("127.0.0.1:0").await.unwrap();
            let port = l.local_addr().unwrap().port();
            let app = router(f.state.clone());
            tokio::spawn(async move { axum::serve(l, app).await.unwrap() });
            port
        }

        async fn connect(port: u16, cookie: Option<&str>) -> Result<Client, tokio_tungstenite::tungstenite::Error> {
            let mut req = format!("ws://127.0.0.1:{port}/ws").into_client_request().unwrap();
            if let Some(c) = cookie {
                req.headers_mut().insert(header::COOKIE, c.parse().unwrap());
            }
            tokio_tungstenite::connect_async(req).await.map(|(c, _)| c)
        }

        async fn next_json(c: &mut Client) -> Value {
            loop {
                match c.next().await.unwrap().unwrap() {
                    M::Text(t) => return serde_json::from_str(&t).unwrap(),
                    _ => continue,
                }
            }
        }

        async fn paired(f: &Fixture) -> Client {
            let mut c = connect(serve(f).await, OK).await.unwrap();
            assert_eq!(next_json(&mut c).await["type"], "layout");
            c
        }

        #[tokio::test]
        async fn ws_without_token_is_rejected() {
            let f = fixture(Ok(()));
            let port = serve(&f).await;
            match connect(port, None).await {
                Err(tokio_tungstenite::tungstenite::Error::Http(r)) => assert_eq!(r.status(), 401),
                other => panic!("expected 401, got {:?}", other.map(|_| ())),
            }
        }

        #[tokio::test]
        async fn ws_sends_layout_first() {
            let f = fixture(Ok(()));
            let mut c = connect(serve(&f).await, OK).await.unwrap();
            let m = next_json(&mut c).await;
            assert_eq!(m["type"], "layout");
            assert_eq!(m["pages"].as_array().unwrap().len(), 1);
            assert_eq!(m["pages"][0]["buttons"].as_array().unwrap().len(), 3);
            assert_eq!(m["theme"]["id"], "keycap");
        }

        #[tokio::test]
        async fn ws_press_of_deleted_button_reports_unknown_button() {
            let f = fixture(Ok(()));
            let mut c = paired(&f).await;
            let (gone, kept) = {
                let mut cfg = f.state.config.write().await;
                let b = &mut cfg.pages[0].buttons;
                (b.remove(0).id, b[0].id.clone())
            };
            c.send(M::text(json!({"type":"press","press_id":7,"button_id":gone}).to_string())).await.unwrap();
            assert_eq!(
                next_json(&mut c).await,
                json!({"type":"result","press_id":7,"ok":false,"error":"unknown_button"})
            );
            c.send(M::text(json!({"type":"press","press_id":8,"button_id":kept}).to_string())).await.unwrap();
            assert_eq!(next_json(&mut c).await, json!({"type":"result","press_id":8,"ok":true,"error":null}));
        }

        #[tokio::test]
        async fn ws_ignores_garbage_and_stays_open() {
            let f = fixture(Ok(()));
            let mut c = paired(&f).await;
            let id = f.state.config.read().await.pages[0].buttons[0].id.clone();
            c.send(M::text("hello")).await.unwrap();
            c.send(M::text(r#"{"type":"nope"}"#)).await.unwrap();
            c.send(M::text(json!({"type":"press","press_id":2,"button_id":id}).to_string())).await.unwrap();
            assert_eq!(next_json(&mut c).await, json!({"type":"result","press_id":2,"ok":true,"error":null}));
        }

        #[tokio::test]
        async fn ws_oversized_message_runs_nothing() {
            let f = fixture(Ok(()));
            let mut c = paired(&f).await;
            let id = f.state.config.read().await.pages[0].buttons[0].id.clone();
            let big = json!({"type":"press","press_id":1,"button_id":id,"pad":"x".repeat(5000)}).to_string();
            c.send(M::text(big)).await.unwrap();
            let _ = tokio::time::timeout(std::time::Duration::from_millis(500), c.next()).await;
            assert!(f.exec.ran.lock().unwrap().is_empty());
        }

        #[tokio::test]
        async fn ws_message_carrying_a_path_does_nothing() {
            let f = fixture(Ok(()));
            let mut c = paired(&f).await;
            c.send(M::text(r#"{"type":"launch","path":"C:\\x.exe"}"#)).await.unwrap();
            let id = f.state.config.read().await.pages[0].buttons[0].id.clone();
            c.send(M::text(json!({"type":"press","press_id":3,"button_id":id}).to_string())).await.unwrap();
            assert_eq!(next_json(&mut c).await["press_id"], 3);
            assert_eq!(f.exec.ran.lock().unwrap().len(), 1); // only the real press
        }

        #[tokio::test]
        async fn ws_phone_status_follows_current_connection() {
            let f = fixture(Ok(()));
            let port = serve(&f).await;
            let mut a = connect(port, OK).await.unwrap();
            next_json(&mut a).await;
            let mut b = connect(port, OK).await.unwrap();
            next_json(&mut b).await;
            assert_eq!(next_json(&mut a).await["type"], "kicked");
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            assert!(f.sink.0.lock().unwrap().iter().all(|e| *e != CoreEvent::PhoneStatus { connected: false }));
            b.close(None).await.unwrap();
            tokio::time::sleep(std::time::Duration::from_millis(200)).await;
            assert_eq!(f.sink.0.lock().unwrap().last(), Some(&CoreEvent::PhoneStatus { connected: false }));
        }
    }
}
