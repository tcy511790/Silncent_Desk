use std::thread;
use std::time::Duration;

use tauri::{AppHandle, Emitter, Manager, Wry};
use tauri_plugin_global_shortcut::{
    Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState,
};

const EVENT_WHEEL_OPEN: &str = "jingzhuo-wheel-open";
const EVENT_TOOLBAR_OPEN: &str = "jingzhuo-toolbar-open";
const EVENT_AI_OPEN: &str = "jingzhuo-ai-open";

pub fn plugin() -> tauri::plugin::TauriPlugin<Wry> {
    tauri_plugin_global_shortcut::Builder::new()
        .with_handler(|app, shortcut, event| {
            if event.state != ShortcutState::Pressed {
                return;
            }

            if *shortcut == toolbar_shortcut() {
                emit_frontend_event(app, EVENT_TOOLBAR_OPEN);
                return;
            }

            if *shortcut == wheel_shortcut() {
                emit_frontend_event(app, EVENT_WHEEL_OPEN);
                return;
            }

            if *shortcut == ai_shortcut() {
                emit_frontend_event(app, EVENT_AI_OPEN);
                return;
            }

            if *shortcut == quit_shortcut() {
                app.exit(0);
            }
        })
        .build()
}

pub fn register(app: &AppHandle) {
    let shortcuts = [
        toolbar_shortcut(),
        wheel_shortcut(),
        ai_shortcut(),
        quit_shortcut(),
    ];
    if let Err(err) = app.global_shortcut().register_multiple(shortcuts) {
        eprintln!("[jingzhuo-shortcuts] failed to register global shortcuts: {err}");
    }
}

pub fn enable_default_click_through(app: AppHandle) {
    thread::spawn(move || {
        thread::sleep(Duration::from_millis(1_000));
        set_click_through(&app, true);
    });
}

#[tauri::command]
pub fn set_overlay_interactive(app: AppHandle, interactive: bool) {
    if interactive {
        set_click_through(&app, false);
    } else {
        set_click_through(&app, true);
    }
}

#[tauri::command]
pub fn quit_app(app: AppHandle) {
    app.exit(0);
}

fn toolbar_shortcut() -> Shortcut {
    Shortcut::new(Some(Modifiers::CONTROL), Code::Space)
}

fn wheel_shortcut() -> Shortcut {
    Shortcut::new(Some(Modifiers::CONTROL | Modifiers::ALT), Code::Space)
}

fn ai_shortcut() -> Shortcut {
    Shortcut::new(Some(Modifiers::CONTROL), Code::KeyT)
}

fn quit_shortcut() -> Shortcut {
    Shortcut::new(Some(Modifiers::CONTROL | Modifiers::SHIFT), Code::KeyQ)
}

fn set_click_through(app: &AppHandle, ignore: bool) {
    if let Some(window) = app.get_webview_window("main") {
        if let Err(err) = window.set_ignore_cursor_events(ignore) {
            eprintln!("[jingzhuo-window] failed to set click-through={ignore}: {err}");
        }

        if !ignore {
            let _ = window.set_focus();
        }
    }
}

fn emit_frontend_event(app: &AppHandle, event: &str) {
    let _ = app.emit(event, ());
    if let Some(window) = app.get_webview_window("main") {
        let event_name = serde_json::to_string(event).unwrap_or_else(|_| "\"\"".to_string());
        let script = format!("window.dispatchEvent(new CustomEvent({event_name}));");
        let _ = window.eval(&script);
        let _ = window.set_focus();
    }
}
