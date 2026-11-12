pub mod actions;
pub mod auth;
pub mod commands;
pub mod config;
pub mod elevation;
pub mod hub;
pub mod net;
pub mod server;
pub mod state;
mod themes;

use state::{AppState, CoreEvent, EventSink};
use std::sync::atomic::Ordering;
use std::sync::Arc;
use tauri::{Emitter, Manager};

/// Forwards every core event to the desktop window. Called under the hub's lock, so it only
/// hands the event to Tauri and never touches app state.
struct TauriSink(tauri::AppHandle);

impl EventSink for TauriSink {
    fn emit(&self, event: CoreEvent) {
        let _ = self.0.emit("core-event", event);
    }
}

fn build_state(app: &tauri::App) -> Result<Arc<AppState>, Box<dyn std::error::Error>> {
    let dir = app.path().app_data_dir()?;
    std::fs::create_dir_all(&dir)?;
    let (cfg, first_run, recovered_backup) = match config::load_or_init(&dir)? {
        config::LoadOutcome::Loaded(c) => (c, false, None),
        config::LoadOutcome::Created(c) => (c, true, None),
        config::LoadOutcome::Recovered { config, backup } => {
            (config, false, Some(backup.to_string_lossy().into_owned()))
        }
    };
    let preferred_port = cfg.settings.port;
    let events = Arc::new(TauriSink(app.handle().clone()));
    // Inside the runtime: AppState::new spawns the OBS task.
    tauri::async_runtime::block_on(async move {
        let mut state = AppState::new(dir, cfg, events, |obs| Arc::new(actions::SystemExecutor { obs }));
        state.startup.first_run = first_run;
        state.startup.recovered_backup = recovered_backup;
        let bound = server::bind(preferred_port).await;
        match &bound {
            Ok((_, port)) => state.port.store(*port, Ordering::Relaxed),
            Err(_) => state.startup.server_error = Some("bind_failed"),
        }
        let state = Arc::new(state);
        if let Ok((listener, _)) = bound {
            let router = server::router(state.clone());
            tauri::async_runtime::spawn(async move {
                let _ = axum::serve(listener, router).await;
            });
        }
        Ok(state)
    })
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(w) = app.get_webview_window("main") {
                let _ = w.unminimize();
                let _ = w.show();
                let _ = w.set_focus();
            }
        }))
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let state = build_state(app)?;
            app.manage(state);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_config,
            commands::save_config,
            commands::get_runtime,
            commands::pairing_info,
            commands::regenerate_token,
            commands::test_button,
            commands::obs_scenes,
            commands::import_icon,
            commands::list_themes,
            commands::get_theme,
            commands::import_theme,
            commands::delete_theme,
            commands::get_status,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
