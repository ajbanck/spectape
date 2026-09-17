//! Desktop shell for SpecTape. All editing logic lives in the web front end; this crate only
//! provides the window, native dialogs (via plugins) and "open with" file handling.

mod emulator;
mod menu;

use std::sync::Mutex;
use tauri::{Emitter, Manager};

/// Where the last used theme is remembered, so the window can be created in the right colour
/// instead of flashing white while the webview loads. Plain std so it works before the app exists.
fn theme_file() -> Option<std::path::PathBuf> {
    let id = "com.zxtoolkit.spectape";
    let dir = if cfg!(target_os = "macos") {
        std::path::PathBuf::from(std::env::var_os("HOME")?).join("Library/Application Support")
    } else if cfg!(windows) {
        std::path::PathBuf::from(std::env::var_os("APPDATA")?)
    } else if let Some(x) = std::env::var_os("XDG_CONFIG_HOME") {
        std::path::PathBuf::from(x)
    } else {
        std::path::PathBuf::from(std::env::var_os("HOME")?).join(".config")
    };
    Some(dir.join(id).join("theme"))
}

/// The background the window is created with: what the UI used last time. Light on first run,
/// which is what the system would paint anyway. Matches --bg in style.css.
fn remembered_background() -> tauri::utils::config::Color {
    let remembered = theme_file().and_then(|p| std::fs::read_to_string(p).ok());
    let dark = remembered.as_deref().map(str::trim) == Some("dark");
    if dark {
        tauri::utils::config::Color(0x0f, 0x12, 0x18, 0xff)
    } else {
        tauri::utils::config::Color(0xee, 0xf0, 0xf4, 0xff)
    }
}

/// Front end reports the theme it resolved to, for the next start.
#[tauri::command]
fn remember_theme(dark: bool) {
    if let Some(path) = theme_file() {
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = std::fs::write(path, if dark { "dark" } else { "light" });
    }
}

/// When the process started, for the SPECTAPE_TIMING breakdown: a monotonic mark, plus the
/// wall clock so the webview's own timeline can be lined up with it.
struct StartTime(std::time::Instant, f64);

fn epoch_ms() -> f64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs_f64() * 1000.0)
        .unwrap_or_default()
}

/// Elapsed milliseconds since the process started, printed when SPECTAPE_TIMING is set.
fn mark(start: &std::time::Instant, what: &str) {
    if std::env::var_os("SPECTAPE_TIMING").is_none() {
        return;
    }
    eprintln!("[timing] {:>6.0} ms  {what}", start.elapsed().as_secs_f64() * 1000.0);
}

fn timing(app: &tauri::AppHandle, what: &str) {
    if let Some(start) = app.try_state::<StartTime>() {
        mark(&start.0, what);
    }
}

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

/// The front end reports that it has rendered: show the window. It is created hidden because a
/// window on screen before the document paints is a white rectangle, whatever background colour
/// the config asks for. `ui_ms` is how long the front end itself took.
#[tauri::command]
fn front_end_ready(app: tauri::AppHandle, ui_ms: f64, time_origin: f64) {
    if let Some(window) = app.get_webview_window("main") {
        // Belt and braces: the same colour the window was created with, applied to the window
        // and the webview layer, so the frames between show() and the first composite are not
        // white. WKWebView draws white by default and only stops when a colour is set.
        let _ = window.set_background_color(Some(remembered_background()));
        let _ = window.show();
    }
    if let Some(start) = app.try_state::<StartTime>() {
        let began = time_origin - start.1;
        timing(&app, &format!("document began loading at {began:.0} ms"));
    }
    timing(&app, &format!("front end rendered (it used {ui_ms:.0} ms)"));
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

/// Without the WebView2 runtime the window cannot be created, and a release build has no console,
/// so the app would exit without a word. Windows 11 always has the runtime and most Windows 10
/// machines get it with Edge, but LTSC and locked-down installs may not; say so instead of
/// vanishing. The installers fetch the runtime themselves, so this only bites the portable exe.
#[cfg(windows)]
fn require_webview2() {
    use windows_sys::Win32::UI::WindowsAndMessaging::{MessageBoxW, MB_ICONERROR, MB_OK};

    if tauri::webview_version().is_ok() {
        return;
    }
    let wide = |s: &str| s.encode_utf16().chain(std::iter::once(0)).collect::<Vec<u16>>();
    let text = wide(concat!(
        "SpecTape needs the Microsoft Edge WebView2 runtime, which this computer does not have.\n\n",
        "Install it from https://developer.microsoft.com/microsoft-edge/webview2/ ",
        "or run the SpecTape installer, which fetches it for you."
    ));
    let caption = wide("SpecTape");
    unsafe {
        MessageBoxW(std::ptr::null_mut(), text.as_ptr(), caption.as_ptr(), MB_OK | MB_ICONERROR);
    }
    std::process::exit(1);
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    #[cfg(windows)]
    require_webview2();

    let start = std::time::Instant::now();
    mark(&start, "main entered");
    if std::env::var_os("SPECTAPE_TIMING").is_some() {
        let file = theme_file().and_then(|p| std::fs::read_to_string(p).ok());
        let known = matches!(file.as_deref().map(str::trim), Some("dark") | Some("light"));
        eprintln!(
            "[timing]        window background: {} ({})",
            if file.as_deref().map(str::trim) == Some("dark") { "dark" } else { "light" },
            if known { "remembered" } else { "nothing remembered yet" }
        );
    }
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .manage(PendingFiles(Mutex::new(Vec::new())))
        .manage(StartTime(start, epoch_ms()))
        .invoke_handler(tauri::generate_handler![take_pending_files, set_menu_checked, front_end_ready, remember_theme, emulator::detect_emulator, emulator::open_in_emulator])
        .on_page_load(|webview, payload| {
            use tauri::webview::PageLoadEvent;
            timing(
                webview.app_handle(),
                match payload.event() {
                    PageLoadEvent::Started => "page load started",
                    PageLoadEvent::Finished => "page load finished",
                },
            );
        })
        .setup(move |app| {
            timing(app.handle(), "setup: window and webview created");
            app.set_menu(menu::build(app.handle())?)?;
            timing(app.handle(), "setup: menu built");
            // If the front end never reports in, show the window anyway rather than leave the
            // app running with nothing on screen.
            if let Some(window) = app.get_webview_window("main") {
                std::thread::spawn(move || {
                    std::thread::sleep(std::time::Duration::from_millis(1500));
                    let _ = window.show();
                });
            }
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
        .build({
            let mut context = tauri::generate_context!();
            if let Some(window) = context.config_mut().app.windows.first_mut() {
                window.background_color = Some(remembered_background());
            }
            context
        })
        .expect("error while building tauri application");
    mark(&start, "app built, entering event loop");
    app.run(|app, event| {
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
