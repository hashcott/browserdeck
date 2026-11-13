//! The tray icon. Closing the window only hides it; the phone server keeps running until "Quit".

use crate::commands::update_settings;
use crate::state::AppState;
use std::sync::Arc;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager, Window, WindowEvent};
use tauri_plugin_dialog::DialogExt;

const TRAY_ID: &str = "main";
const OPEN: &str = "open";
const QUIT: &str = "quit";

/// (open window, quit), in the language the settings pick; "auto" follows the system.
pub fn tray_labels(language_setting: &str, os_locale: &str) -> (&'static str, &'static str) {
    if is_vi(language_setting, os_locale) {
        ("Mở", "Thoát")
    } else {
        ("Open", "Quit")
    }
}

/// The rule of `resolveLang` in src/shared/i18n.ts.
fn is_vi(setting: &str, os_locale: &str) -> bool {
    setting == "vi" || (setting == "auto" && os_locale.to_ascii_lowercase().starts_with("vi"))
}

fn os_locale() -> String {
    sys_locale::get_locale().unwrap_or_default()
}

/// `tray.stillRunning` from the desktop's translation tables.
fn notice_text(vi: bool) -> String {
    let table = if vi { include_str!("../../src/shared/locales/vi.json") } else { include_str!("../../src/shared/locales/en.json") };
    serde_json::from_str::<serde_json::Value>(table)
        .ok()
        .and_then(|t| t["tray.stillRunning"].as_str().map(str::to_string))
        .unwrap_or_default()
}

fn menu(app: &AppHandle, language: &str) -> tauri::Result<Menu<tauri::Wry>> {
    let (open, quit) = tray_labels(language, &os_locale());
    let open = MenuItem::with_id(app, OPEN, open, true, None::<&str>)?;
    let quit = MenuItem::with_id(app, QUIT, quit, true, None::<&str>)?;
    Menu::with_items(app, &[&open, &quit])
}

pub fn create(app: &AppHandle, language: &str) -> tauri::Result<()> {
    let mut tray = TrayIconBuilder::with_id(TRAY_ID)
        .tooltip("Browserdeck")
        .menu(&menu(app, language)?)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id().as_ref() {
            OPEN => show_window(app),
            QUIT => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = event {
                show_window(tray.app_handle());
            }
        });
    if let Some(icon) = app.default_window_icon() {
        tray = tray.icon(icon.clone());
    }
    tray.build(app)?;
    Ok(())
}

/// Follows a language change made in Settings.
pub fn relabel(app: &AppHandle, language: &str) {
    if let Some(tray) = app.tray_by_id(TRAY_ID) {
        let _ = menu(app, language).and_then(|m| tray.set_menu(Some(m)));
    }
}

pub fn show_window(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
    }
}

/// Closing the window hides it. The first time, a message over the window first says the app is still in the tray.
pub fn on_window_event(window: &Window, event: &WindowEvent) {
    let WindowEvent::CloseRequested { api, .. } = event else { return };
    api.prevent_close();
    let state = window.try_state::<Arc<AppState>>().map(|s| s.inner().clone());
    // try_read: never block the UI thread; while a save holds the lock, just hide.
    let pending = state.as_ref().and_then(|s| {
        let c = s.config.try_read().ok()?;
        (!c.settings.tray_notice_shown).then(|| c.settings.language.clone())
    });
    let (Some(state), Some(language)) = (state, pending) else {
        let _ = window.hide();
        return;
    };
    tauri::async_runtime::spawn(async move {
        let _ = update_settings(&state, |s| s.tray_notice_shown = true).await;
    });
    let w = window.clone();
    window
        .dialog()
        .message(notice_text(is_vi(&language, &os_locale())))
        .title("Browserdeck")
        .parent(window)
        .show(move |_| {
            let _ = w.hide();
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tray_labels_follow_language() {
        assert_eq!(tray_labels("vi", "en-US"), ("Mở", "Thoát"));
        assert_eq!(tray_labels("auto", "vi-VN"), ("Mở", "Thoát"));
        assert_eq!(tray_labels("auto", "fr-FR"), ("Open", "Quit"));
        assert_eq!(tray_labels("en", "vi-VN"), ("Open", "Quit"));
    }

    #[test]
    fn notice_comes_from_the_translation_tables() {
        assert_eq!(notice_text(true), "App vẫn đang chạy ở khay hệ thống.");
        assert_eq!(notice_text(false), "The app is still running in the system tray.");
    }
}
