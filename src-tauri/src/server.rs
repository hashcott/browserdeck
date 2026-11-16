use crate::actions;
use crate::auth;
use crate::config::is_icon_file_name;
use crate::hub::{layout_msg, ClientMsg, ServerMsg};
use crate::state::{AppState};
use axum::{
    body::Bytes,
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
use std::time::Duration;
use tokio::net::TcpListener;
use tokio::time::Instant;

pub const DEFAULT_PORT: u16 = 47821;

const NOT_PAIRED: &str = "<!doctype html><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width,initial-scale=1\"><title>Not paired</title><p>Not paired. Scan the QR code in the desktop app.</p><p>Chưa ghép. Hãy quét mã QR trong app trên máy tính.</p>";

#[derive(RustEmbed)]
#[folder = "../dist"]
struct Dist;

/// Tries `preferred..=preferred+9` on all interfaces. `exclusive`: see `bind_port`.
pub async fn bind(preferred: u16, exclusive: bool) -> io::Result<(TcpListener, u16)> {
    let mut last = io::Error::new(io::ErrorKind::AddrInUse, "no free port");
    for port in preferred..=preferred.saturating_add(9) {
        match bind_port(port, exclusive) {
            Ok(l) => return Ok((l, port)),
            Err(e) => last = e,
        }
    }
    Err(last)
}

/// `exclusive` (Windows, the elevated app): while this app holds the port, no other program can bind a more specific
/// address on it (for example this PC's LAN address) and answer the phone in its place. The cost: after a restart
/// the port stays blocked while connections this app closed wait in TIME_WAIT (about two minutes), so the server
/// may move to the next port and the phone must scan again. A plain bind has neither.
fn bind_port(port: u16, exclusive: bool) -> io::Result<TcpListener> {
    let socket = tokio::net::TcpSocket::new_v4()?;
    #[cfg(windows)]
    if exclusive {
        use std::os::windows::io::AsRawSocket;
        use windows::Win32::Networking::WinSock::{setsockopt, SOCKET, SOL_SOCKET, SO_EXCLUSIVEADDRUSE};
        let on = 1i32.to_ne_bytes();
        // SAFETY: the socket is open and not yet bound; the option value is a live 4-byte BOOL.
        let r = unsafe { setsockopt(SOCKET(socket.as_raw_socket() as usize), SOL_SOCKET, SO_EXCLUSIVEADDRUSE, Some(&on)) };
        if r != 0 {
            return Err(io::Error::last_os_error());
        }
    }
    #[cfg(not(windows))]
    {
        let _ = exclusive; // no such option here
        socket.set_reuseaddr(true)?; // what TcpListener::bind does on Unix
    }
    socket.bind((std::net::Ipv4Addr::UNSPECIFIED, port).into())?;
    socket.listen(1024)
}

pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/", get(index))
        .route("/assets/{*path}", get(asset))
        .route("/icons/{file}", get(icon))
        .route("/glyphs/{set}/{file}", get(glyph))
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

/// Runs before routing touches anything: every request needs the pairing token. The elevated app also refuses every
/// client that cannot be the phone (`net::may_be_the_phone`): a program on this PC could have stolen its token (see
/// admin_token). No client address (not served through `into_make_service_with_connect_info`) is refused too.
async fn gate(State(state): State<Arc<AppState>>, req: Request, next: Next) -> Response {
    let elevated = state.admin_token.lock().unwrap_or_else(|e| e.into_inner()).is_some();
    if elevated {
        let peer = req.extensions().get::<axum::extract::ConnectInfo<std::net::SocketAddr>>().map(|c| c.0.ip());
        if !peer.is_some_and(|ip| crate::net::may_be_the_phone(ip, &crate::net::own_ips())) {
            return StatusCode::FORBIDDEN.into_response();
        }
    }
    let from_query = req.uri().query().and_then(query_token);
    let from_cookie = req
        .headers()
        .get(header::COOKIE)
        .and_then(|v| v.to_str().ok())
        .and_then(cookie_token);
    let expected = state.token().await;
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

/// Changes whenever the phone page is rebuilt (an app update); an open phone reloads when it sees a new one.
pub fn phone_build() -> String {
    Dist::get("phone.html")
        .map(|f| f.metadata.sha256_hash()[..6].iter().map(|b| format!("{b:02x}")).collect())
        .unwrap_or_default()
}

async fn index() -> Response {
    match Dist::get("phone.html") {
        // no-cache: a reload after an update must get the new page, not a stored copy.
        Some(f) => (
            [(header::CACHE_CONTROL, HeaderValue::from_static("no-cache"))],
            file_response("phone.html", f.data.into_owned()),
        )
            .into_response(),
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

/// A bundled key icon (scripts/glyphs.mjs puts them in dist/glyphs). Only the two sets and plain names pass.
async fn glyph(Path((set, file)): Path<(String, String)>) -> Response {
    let ok = matches!(set.as_str(), "ph" | "si") && file.strip_suffix(".svg").is_some_and(crate::config::is_glyph_name);
    match Dist::get(&format!("glyphs/{set}/{file}")).filter(|_| ok) {
        // The sets only change with an app update, which also reloads the phone page.
        Some(f) => ([(header::CACHE_CONTROL, HeaderValue::from_static("max-age=86400"))], file_response(&file, f.data.into_owned()))
            .into_response(),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

const MAX_WS_MESSAGE: usize = 4096;
const PING_INTERVAL: Duration = Duration::from_secs(15);
const IDLE_TIMEOUT: Duration = Duration::from_secs(45);

async fn ws_upgrade(State(state): State<Arc<AppState>>, ws: WebSocketUpgrade) -> Response {
    ws.max_message_size(MAX_WS_MESSAGE).on_upgrade(move |socket| phone_session(state, socket))
}

async fn send_msg(socket: &mut WebSocket, msg: &ServerMsg) -> bool {
    let Ok(json) = serde_json::to_string(msg) else { return false };
    socket.send(Message::Text(json.into())).await.is_ok()
}

/// Detaches on every exit path (return, panic, task dropped at shutdown).
struct Attached(Arc<AppState>, u64);
impl Drop for Attached {
    fn drop(&mut self) {
        self.0.hub.detach_now(self.1);
    }
}

async fn phone_session(state: Arc<AppState>, mut socket: WebSocket) {
    let (conn_id, mut rx) = state.hub.attach().await;
    let _attached = Attached(state.clone(), conn_id);
    if !send_msg(&mut socket, &layout_msg(&state).await).await {
        return;
    }
    let mut ping = tokio::time::interval_at(Instant::now() + PING_INTERVAL, PING_INTERVAL);
    let mut last_rx = Instant::now();
    loop {
        // Biased: a pending Kicked is always handled before another press.
        tokio::select! {
            biased;
            out = rx.recv() => match out {
                Some(msg) => {
                    let kicked = msg == ServerMsg::Kicked;
                    if !send_msg(&mut socket, &msg).await { break }
                    if kicked {
                        let _ = socket.send(Message::Close(None)).await;
                        break;
                    }
                }
                None => break,
            },
            _ = ping.tick() => {
                if last_rx.elapsed() >= IDLE_TIMEOUT
                    || socket.send(Message::Ping(Bytes::new())).await.is_err()
                {
                    break;
                }
            }
            inc = socket.recv() => {
                last_rx = Instant::now();
                match inc {
                    Some(Ok(Message::Text(t))) => {
                        // Garbage (or any type other than `press`) is ignored, connection stays open.
                        let Ok(ClientMsg::Press { press_id, button_id }) = serde_json::from_str(&t) else { continue };
                        let error = actions::press(&state, &button_id).await.err();
                        let res = ServerMsg::Result { press_id, ok: error.is_none(), error };
                        if !send_msg(&mut socket, &res).await { break }
                    }
                    Some(Ok(Message::Close(_))) | Some(Err(_)) | None => break,
                    Some(Ok(_)) => {}
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::actions::{ActionError, Executor};
    use crate::config::{Action, Config};
    use crate::state::{CoreEvent, EventSink};
    use axum::body::Body;
    use axum::http::Request;
    use http_body_util::BodyExt;
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
        let state = Arc::new(AppState::new(
            dir.path().to_path_buf(),
            Config::default_with_token("secret".into()),
            Arc::new(NullSink),
            |_| Arc::new(OkExec),
        ));
        (router(state), dir)
    }

    async fn get(app: &Router, uri: &str, cookie: Option<&str>) -> Response {
        get_from(app, uri, cookie, None).await
    }

    /// `peer`: the client address `axum::serve` would attach (it is absent in `oneshot` otherwise).
    async fn get_from(app: &Router, uri: &str, cookie: Option<&str>, peer: Option<&str>) -> Response {
        let mut r = Request::builder().uri(uri);
        if let Some(c) = cookie {
            r = r.header(header::COOKIE, c);
        }
        if let Some(p) = peer {
            r = r.extension(axum::extract::ConnectInfo(p.parse::<std::net::SocketAddr>().unwrap()));
        }
        app.clone().oneshot(r.body(Body::empty()).unwrap()).await.unwrap()
    }

    fn elevated_app() -> (Router, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let state = Arc::new(AppState::new(
            dir.path().to_path_buf(),
            Config::default_with_token("secret".into()),
            Arc::new(NullSink),
            |_| Arc::new(OkExec),
        ));
        *state.admin_token.lock().unwrap() = Some("admintok".into());
        (router(state), dir)
    }

    const LAN: Option<&str> = Some("192.168.1.50:5000");

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
        let state = Arc::new(AppState::new(
            d.path().to_path_buf(),
            Config::default_with_token(String::new()),
            Arc::new(NullSink),
            |_| Arc::new(OkExec),
        ));
        let app = router(state);
        assert_eq!(get(&app, "/?token=", None).await.status(), StatusCode::UNAUTHORIZED);
        assert_eq!(get(&app, "/", Some("sd_token=")).await.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn admin_token_is_the_only_token_while_set() {
        let (app, _d) = elevated_app();
        assert_eq!(get_from(&app, "/?token=admintok", None, LAN).await.status(), StatusCode::SEE_OTHER);
        assert_eq!(get_from(&app, "/?token=secret", None, LAN).await.status(), StatusCode::UNAUTHORIZED);
        assert_eq!(get_from(&app, "/", OK, LAN).await.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn elevated_server_refuses_this_pc() {
        let (app, _d) = elevated_app();
        let status = |r: Response| r.status();
        assert_eq!(status(get_from(&app, "/?token=admintok", None, Some("127.0.0.1:5000")).await), StatusCode::FORBIDDEN);
        assert_eq!(status(get_from(&app, "/?token=admintok", None, Some("[::1]:5000")).await), StatusCode::FORBIDDEN);
        assert_eq!(status(get_from(&app, "/?token=admintok", None, None).await), StatusCode::FORBIDDEN);
        assert_eq!(status(get_from(&app, "/?token=admintok", None, LAN).await), StatusCode::SEE_OTHER);
    }

    #[tokio::test]
    async fn elevated_server_refuses_the_internet() {
        let (app, _d) = elevated_app();
        let r = get_from(&app, "/?token=admintok", None, Some("203.0.113.9:5000")).await;
        assert_eq!(r.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn unelevated_server_accepts_this_pc() {
        let (app, _d) = app();
        let r = get_from(&app, "/?token=secret", None, Some("127.0.0.1:5000")).await;
        assert_eq!(r.status(), StatusCode::SEE_OTHER);
    }

    #[cfg(windows)]
    #[tokio::test]
    async fn nobody_can_bind_a_more_specific_address_on_our_port() {
        let (_held, port) = bind(48500, true).await.unwrap();
        let squat = tokio::net::TcpSocket::new_v4().unwrap();
        squat.set_reuseaddr(true).unwrap();
        assert!(squat.bind((std::net::Ipv4Addr::LOCALHOST, port).into()).is_err());
    }

    /// An exclusive port stays blocked while a connection this side closed first sits in TIME_WAIT (about two
    /// minutes), so a plain bind is what lets an unelevated restart keep the port the phone knows.
    #[cfg(windows)]
    #[tokio::test]
    async fn plain_bind_keeps_the_port_right_after_closing_a_connection() {
        let (listener, port) = bind(48520, false).await.unwrap();
        let client = tokio::net::TcpStream::connect((std::net::Ipv4Addr::LOCALHOST, port)).await.unwrap();
        let (server_side, _) = listener.accept().await.unwrap();
        drop(server_side); // this side closes first, so its end waits in TIME_WAIT
        drop(listener);
        drop(client);
        let (_again, again) = bind(port, false).await.unwrap();
        assert_eq!(again, port);
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
    async fn glyph_outside_the_two_sets_or_with_a_strange_name_is_404() {
        let (app, _d) = app();
        for uri in ["/glyphs/xx/play.svg", "/glyphs/ph/..%2F..%2Fphone.html", "/glyphs/ph/Play.svg", "/glyphs/ph/play.png", "/glyphs/ph/ph.json"] {
            assert_eq!(get(&app, uri, OK).await.status(), StatusCode::NOT_FOUND, "{uri}");
        }
    }

    #[tokio::test]
    async fn bind_falls_back_when_port_busy() {
        let (_held, p) = bind(48100, false).await.unwrap();
        let (_l, got) = bind(p, false).await.unwrap();
        assert!((p + 1..=p + 9).contains(&got));
    }

    /// Same two checks as a manual curl against the running app, over a real socket.
    #[tokio::test]
    async fn real_socket_303_with_cookie_and_401_without_token() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let (_, d) = app();
        let state = Arc::new(AppState::new(
            d.path().to_path_buf(),
            Config::default_with_token("secret".into()),
            Arc::new(NullSink),
            |_| Arc::new(OkExec),
        ));
        let (listener, port) = bind(48300, false).await.unwrap();
        let router = router(state);
        tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
        let fetch = |target: &'static str| async move {
            let mut s = tokio::net::TcpStream::connect(("127.0.0.1", port)).await.unwrap();
            let req = format!("GET {target} HTTP/1.1\r\nHost: x\r\nConnection: close\r\n\r\n");
            s.write_all(req.as_bytes()).await.unwrap();
            let mut out = String::new();
            s.read_to_string(&mut out).await.unwrap();
            out
        };
        let ok = fetch("/?token=secret").await;
        assert!(ok.starts_with("HTTP/1.1 303"), "{ok}");
        assert!(ok.to_lowercase().contains("set-cookie: sd_token=secret"), "{ok}");
        assert!(fetch("/").await.starts_with("HTTP/1.1 401"));
    }

    mod ws {
        use super::*;
        use crate::actions::testing::{fixture, state_with, Fixture, Hangs, Panics, RecordingSink};
        use futures_util::{SinkExt, StreamExt};
        use serde_json::{json, Value};
        use tokio_tungstenite::tungstenite::{client::IntoClientRequest, Message as M};

        type Client = tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

        async fn serve(state: &Arc<AppState>) -> u16 {
            let l = TcpListener::bind("127.0.0.1:0").await.unwrap();
            let port = l.local_addr().unwrap().port();
            let app = router(state.clone());
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
            let mut c = connect(serve(&f.state).await, OK).await.unwrap();
            assert_eq!(next_json(&mut c).await["type"], "layout");
            c
        }

        #[tokio::test]
        async fn ws_without_token_is_rejected() {
            let f = fixture(Ok(()));
            let port = serve(&f.state).await;
            match connect(port, None).await {
                Err(tokio_tungstenite::tungstenite::Error::Http(r)) => assert_eq!(r.status(), 401),
                other => panic!("expected 401, got {:?}", other.map(|_| ())),
            }
        }

        #[tokio::test]
        async fn ws_sends_layout_first() {
            let f = fixture(Ok(()));
            let mut c = connect(serve(&f.state).await, OK).await.unwrap();
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
            let ended = tokio::time::timeout(Duration::from_secs(5), async {
                loop {
                    match c.next().await {
                        None | Some(Err(_)) | Some(Ok(M::Close(_))) => break,
                        _ => {}
                    }
                }
            })
            .await;
            assert!(ended.is_ok(), "connection should end");
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

        async fn eventually(limit: Duration, mut ok: impl FnMut() -> bool) {
            let end = Instant::now() + limit;
            while !ok() {
                assert!(Instant::now() < end, "condition not reached in time");
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        }

        const UP: CoreEvent = CoreEvent::PhoneStatus { connected: true };
        const DOWN: CoreEvent = CoreEvent::PhoneStatus { connected: false };

        #[tokio::test]
        async fn ws_phone_status_follows_current_connection() {
            let f = fixture(Ok(()));
            let port = serve(&f.state).await;
            let mut a = connect(port, OK).await.unwrap();
            next_json(&mut a).await;
            let mut b = connect(port, OK).await.unwrap();
            next_json(&mut b).await;
            assert_eq!(next_json(&mut a).await["type"], "kicked");
            assert!(matches!(a.next().await, Some(Ok(M::Close(_)))), "close frame after kicked");
            b.close(None).await.unwrap();
            eventually(Duration::from_secs(5), || *f.sink.0.lock().unwrap() == [UP, UP, DOWN]).await;
        }

        #[tokio::test]
        async fn ws_press_after_kick_does_not_run() {
            let f = fixture(Ok(()));
            let port = serve(&f.state).await;
            let mut a = connect(port, OK).await.unwrap();
            next_json(&mut a).await;
            let mut b = connect(port, OK).await.unwrap();
            next_json(&mut b).await;
            assert_eq!(next_json(&mut a).await["type"], "kicked");
            let id = f.state.config.read().await.pages[0].buttons[0].id.clone();
            let _ = a.send(M::text(json!({"type":"press","press_id":1,"button_id":id}).to_string())).await;
            let _ = tokio::time::timeout(Duration::from_millis(300), a.next()).await;
            assert!(f.exec.ran.lock().unwrap().is_empty());
        }

        #[tokio::test]
        async fn ws_executor_panic_still_detaches() {
            let sink = Arc::new(RecordingSink::default());
            let (state, _d) = state_with(Arc::new(Panics), sink.clone());
            let mut c = connect(serve(&state).await, OK).await.unwrap();
            next_json(&mut c).await;
            let id = state.config.read().await.pages[0].buttons[0].id.clone();
            c.send(M::text(json!({"type":"press","press_id":1,"button_id":id}).to_string())).await.unwrap();
            eventually(Duration::from_secs(5), || *sink.0.lock().unwrap() == [UP, DOWN]).await;
            assert!(!state.hub.connected().await);
        }

        #[tokio::test(start_paused = true)]
        async fn ws_unresponsive_client_is_detached() {
            let sink = Arc::new(RecordingSink::default());
            let (state, _d) = state_with(Arc::new(Hangs), sink.clone());
            let _silent = connect(serve(&state).await, OK).await.unwrap(); // never polled again: no pongs
            eventually(Duration::from_secs(300), || *sink.0.lock().unwrap() == [UP, DOWN]).await;
            assert!(!state.hub.connected().await);
        }
    }
}
