//! Desktop shell for SpecTape. All editing logic lives in the web front end; this crate only
//! provides the window, native dialogs (via plugins) and "open with" file handling.

mod emulator;
mod menu;

use std::sync::Mutex;
use tauri::{Emitter, Manager};

/// Paths the OS asked us to open that the front end has not yet consumed.
struct PendingFiles(Mutex<Vec<String>>);

/// Front end drains the queue with this; called at startup and on every `open-files` event.
#[tauri::command]
fn take_pending_files(state: tauri::State<PendingFiles>) -> Vec<String> {
    std::mem::take(&mut *state.0.lock().unwrap())
}

fn queue_files(app: &tauri::AppHandle, files: Vec<String>) {
    if files.is_empty() {
        return;
    }
    if let Some(state) = app.try_state::<PendingFiles>() {
        state.0.lock().unwrap().extend(files);
    }
    let _ = app.emit("open-files", ());
}

/// Front end reports the state of an Options check item (see `menu::OPTION_IDS`).
#[tauri::command]
fn set_menu_checked(app: tauri::AppHandle, id: String, checked: bool) {
    if let Some(items) = app.try_state::<menu::OptionItems<tauri::Wry>>() {
        if let Some(it) = items.0.iter().find(|it| it.id().0 == id) {
            let _ = it.set_checked(checked);
        }
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .manage(PendingFiles(Mutex::new(Vec::new())))
        .invoke_handler(tauri::generate_handler![take_pending_files, set_menu_checked, emulator::detect_emulator, emulator::open_in_emulator])
        .setup(|app| {
            app.set_menu(menu::build(app.handle())?)?;
            // Windows and Linux pass associated files on the command line.
            let files: Vec<String> = std::env::args()
                .skip(1)
                .filter(|a| !a.starts_with('-') && std::path::Path::new(a).is_file())
                .collect();
            queue_files(app.handle(), files);
            Ok(())
        })
        .on_menu_event(|app, event| {
            let _ = app.emit("menu", event.id().0.clone());
        })
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app, event| {
            // macOS delivers "open with" / dock drops as Opened events, possibly before the
            // webview is ready, so they go through the same queue.
            #[cfg(any(target_os = "macos", target_os = "ios"))]
            if let tauri::RunEvent::Opened { urls } = event {
                let files: Vec<String> = urls
                    .iter()
                    .filter_map(|u| u.to_file_path().ok())
                    .map(|p| p.to_string_lossy().to_string())
                    .collect();
                queue_files(app, files);
            }
            #[cfg(not(any(target_os = "macos", target_os = "ios")))]
            let _ = (app, event);
        });
}
