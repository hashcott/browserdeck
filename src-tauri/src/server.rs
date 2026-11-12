use crate::auth;
use crate::config::is_icon_file_name;
use crate::state::AppState;
use axum::{
    extract::{Path, Request, State},
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
}
