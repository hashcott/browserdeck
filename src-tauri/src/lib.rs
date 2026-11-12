pub mod auth;
pub mod config;
#[allow(dead_code)]
mod themes;

pub fn run() {
    tauri::Builder::default()
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
