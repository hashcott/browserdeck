pub mod actions;
mod app_icon;
pub mod admin_token;
pub mod auth;
pub mod autostart;
pub mod commands;
pub mod config;
pub mod elevation;
pub mod hub;
pub mod net;
pub mod server;
pub mod state;
mod themes;
pub mod tray;
pub mod usage;

use commands::SystemSettingError;
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
    let (autostart, run_as_admin) = (cfg.settings.autostart, cfg.settings.run_as_admin);
    let events = Arc::new(TauriSink(app.handle().clone()));
    // Inside the runtime: AppState::new spawns the OBS task.
    tauri::async_runtime::block_on(async move {
        let mut state = AppState::new(dir, cfg, events, |obs| Arc::new(actions::SystemExecutor { obs }));
        state.startup.first_run = first_run;
        state.startup.recovered_backup = recovered_backup;
        state.startup.admin_error = app.try_state::<AdminInactive>().map(|a| a.0);
        // Keep the start-with-Windows entry in step with the settings, e.g. switch to the elevated task once
        // administrator mode has relaunched the app. Not while administrator mode is on but this process is not
        // elevated: the task cannot be created then. Before the window shows, so Settings sees the outcome.
        if cfg!(windows) && (elevation::self_elevated() || !run_as_admin) {
            *state.autostart_error.get_mut().unwrap_or_else(|e| e.into_inner()) =
                autostart::apply(app.handle(), autostart, run_as_admin).err();
        }
        // Elevated, the phone server takes its token from the registry only (see admin_token). Without one it stays
        // off rather than fall back to the token in the user-writable config.json.
        let bound = match admin_token::for_process(elevation::self_elevated(), admin_token::load_or_create) {
            Ok(token) => {
                let elevated = token.is_some();
                *state.admin_token.get_mut().unwrap_or_else(|e| e.into_inner()) = token;
                server::bind(preferred_port, elevated).await.map_err(|_| "bind_failed")
            }
            Err(code) => Err(code),
        };
        match &bound {
            Ok((_, port)) => state.port.store(*port, Ordering::Relaxed),
            Err(code) => state.startup.server_error = Some(code),
        }
        let state = Arc::new(state);
        usage::spawn(state.clone());
        if let Ok((listener, _)) = bound {
            let router = server::router(state.clone());
            tauri::async_runtime::spawn(async move {
                // With the client address, which the elevated app's gate checks.
                let service = router.into_make_service_with_connect_info::<std::net::SocketAddr>();
                let _ = axum::serve(listener, service).await;
            });
        }
        Ok(state)
    })
}

fn args() -> Vec<String> {
    std::env::args_os().map(|a| a.to_string_lossy().into_owned()).collect()
}

/// Why administrator mode is on but this process runs unelevated.
struct AdminInactive(SystemSettingError);

/// Registered before the single-instance plugin, so the startup relaunch happens while this process holds neither
/// the instance lock nor the phone port, and a copy started by `set_admin_mode` first waits for its predecessor to
/// let go of both.
fn startup_gate() -> tauri::plugin::TauriPlugin<tauri::Wry> {
    tauri::plugin::Builder::new("startup-gate")
        .setup(|app, _| {
            let args = args();
            if let Some(pid) = elevation::wait_pid(&args) {
                elevation::wait_for_exit(pid);
            }
            let run_as_admin = app.path().app_data_dir().is_ok_and(|dir| config::peek_run_as_admin(&dir));
            if elevation::should_relaunch(run_as_admin, elevation::self_elevated(), &args) {
                // Never elevate an exe a standard user can replace. A refused UAC prompt (or any failure) also keeps
                // this process running unelevated; Settings says why.
                let reason = if !elevation::machine_install() {
                    SystemSettingError::NeedsMachineInstall
                } else {
                    let mut extra = vec![elevation::ELEVATED_CHILD_ARG];
                    if args.iter().any(|a| a == autostart::HIDDEN_ARG) {
                        extra.push(autostart::HIDDEN_ARG);
                    }
                    if elevation::relaunch_elevated(&extra).is_ok() {
                        std::process::exit(0); // nothing is held yet: no lock, no port, no window
                    }
                    SystemSettingError::UacDeclined
                };
                app.manage(AdminInactive(reason));
            }
            Ok(())
        })
        .build()
}

pub fn run() {
    // First, before Tauri starts any thread or WebView2 reads the environment.
    elevation::harden_if_elevated();
    tauri::Builder::default()
        .plugin(startup_gate())
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| tray::show_window(app)))
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec![autostart::HIDDEN_ARG]),
        ))
        .setup(|app| {
            let state = build_state(app)?;
            let language = state.config.blocking_read().settings.language.clone();
            app.manage(state);
            tray::create(app.handle(), &language)?;
            if !args().iter().any(|a| a == autostart::HIDDEN_ARG) {
                tray::show_window(app.handle());
            }
            Ok(())
        })
        .on_window_event(tray::on_window_event)
        .invoke_handler(tauri::generate_handler![
            commands::get_config,
            commands::save_config,
            commands::get_runtime,
            commands::pairing_info,
            commands::regenerate_token,
            commands::test_button,
            commands::obs_scenes,
            commands::import_icon,
            commands::app_icon,
            commands::list_themes,
            commands::get_theme,
            commands::import_theme,
            commands::delete_theme,
            commands::get_status,
            commands::set_admin_mode,
            commands::set_autostart,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
