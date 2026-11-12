pub mod actions;
pub mod auth;
pub mod config;
pub mod elevation;
pub mod hub;
pub mod server;
pub mod state;
#[allow(dead_code)]
mod themes;

pub fn run() {
    tauri::Builder::default()
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
