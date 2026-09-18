//! The application menu, built from the table in `menutable.rs`.
//!
//! On macOS and Windows this is the platform's own menu bar: `muda` hangs it on
//! `NSApp` or on the window, the same crate the Tauri shell reached through
//! until stage 5, so the accelerators are the platform's too — `CmdOrCtrl+S`
//! meaning ⌘S here and Ctrl+S there.
//!
//! Elsewhere — Linux, and Windows if `SPECTAPE_EGUI_MENU` is set or the window
//! handle is not there to hang a menu on — the menu is drawn inside the window
//! by egui from the same table, and `app.rs` handles the accelerators itself.
//! Windows needs that anyway: muda's own accelerators want a `TranslateAccelerator`
//! in the message loop, which winit does not have.
//!
//! Clicks arrive on muda's own thread, so they land in a queue and wake the UI;
//! `take_activated` drains it at the top of a frame.

use std::sync::{Arc, Mutex};

use crate::menutable::{MenuState, MENUS};
use crate::theme::Tokens;

#[derive(Default)]
pub struct Queue(Arc<Mutex<Vec<String>>>);

impl Queue {
    /// Command ids activated since the last call.
    pub fn take(&self) -> Vec<String> {
        std::mem::take(&mut *self.0.lock().unwrap())
    }
}

#[cfg(any(target_os = "macos", target_family = "windows"))]
mod platform {
    use super::*;
    use crate::menutable::Item;
    use muda::accelerator::Accelerator;
    use muda::{CheckMenuItem, Menu as MudaMenu, MenuItem, PredefinedMenuItem, Submenu};

    enum Handle {
        Item(MenuItem),
        Check(CheckMenuItem),
        Separator,
    }

    pub struct Menu {
        /// One entry per flat index of `menutable::flat()`, so the app can address
        /// items the way the table lists them.
        handles: Vec<Handle>,
        /// Kept alive: dropping the menu takes it off the menu bar.
        _bar: MudaMenu,
        queue: Queue,
        /// What was last pushed, so an unchanged frame touches nothing.
        last: std::cell::RefCell<(Vec<bool>, Vec<bool>)>,
    }

    fn accelerator(item: &Item) -> Option<Accelerator> {
        if item.keys.is_empty() {
            return None;
        }
        match item.keys.parse() {
            Ok(a) => Some(a),
            Err(e) => {
                eprintln!("menu: {:?} has an unusable accelerator {:?}: {e}", item.id, item.keys);
                None
            }
        }
    }

    impl Menu {
        /// The platform menu, or `None` when this platform has nowhere to put it —
        /// on Windows, a window handle eframe did not hand out.
        pub fn new(ctx: &egui::Context, window: Option<isize>) -> Option<Menu> {
            let bar = MudaMenu::new();

            // macOS expects the first menu to be the application's own; muda, unlike
            // Slint, does not add it for you. Windows has no such menu.
            #[cfg(target_os = "macos")]
            {
                let about = muda::AboutMetadata {
                    name: Some("SpecTape".into()),
                    version: Some(env!("CARGO_PKG_VERSION").into()),
                    comments: Some("ZX Spectrum TZX/TAP tape editor".into()),
                    ..Default::default()
                };
                let app_menu = Submenu::new("SpecTape", true);
                let _ = app_menu.append_items(&[
                    &PredefinedMenuItem::about(Some("About SpecTape"), Some(about)),
                    &PredefinedMenuItem::separator(),
                    &PredefinedMenuItem::services(None),
                    &PredefinedMenuItem::separator(),
                    &PredefinedMenuItem::hide(None),
                    &PredefinedMenuItem::hide_others(None),
                    &PredefinedMenuItem::show_all(None),
                    &PredefinedMenuItem::separator(),
                    &PredefinedMenuItem::quit(None),
                ]);
                let _ = bar.append(&app_menu);
            }

            let mut handles = Vec::new();
            for menu in MENUS {
                let sub = Submenu::new(menu.title, true);
                for item in menu.items {
                    if item.id.is_empty() {
                        let sep = PredefinedMenuItem::separator();
                        let _ = sub.append(&sep);
                        handles.push(Handle::Separator);
                    } else if item.check {
                        let it = CheckMenuItem::with_id(item.id, item.label, true, false, accelerator(item));
                        let _ = sub.append(&it);
                        handles.push(Handle::Check(it));
                    } else {
                        let it = MenuItem::with_id(item.id, item.label, true, accelerator(item));
                        let _ = sub.append(&it);
                        handles.push(Handle::Item(it));
                    }
                }
                let _ = bar.append(&sub);
            }

            #[cfg(target_os = "macos")]
            {
                let _ = window;
                bar.init_for_nsapp();
            }
            #[cfg(target_family = "windows")]
            {
                // Safety: eframe hands out the handle of the window it created, and
                // the menu outlives this call.
                match window.map(|hwnd| unsafe { bar.init_for_hwnd(hwnd) }) {
                    Some(Ok(())) => {}
                    other => {
                        if let Some(Err(e)) = other {
                            eprintln!("menu: the window would not take a menu bar ({e}); drawing our own");
                        }
                        return None;
                    }
                }
            }

            let queue = Queue::default();
            let sink = queue.0.clone();
            let ctx = ctx.clone();
            muda::MenuEvent::set_event_handler(Some(move |event: muda::MenuEvent| {
                sink.lock().unwrap().push(event.id.0.clone());
                // The click happens outside egui's event loop, so ask for a frame.
                ctx.request_repaint();
            }));

            Some(Menu { handles, _bar: bar, queue, last: std::cell::RefCell::new((Vec::new(), Vec::new())) })
        }

        /// Push enabled and checked state, skipping the frames where nothing moved.
        pub fn set_state(&self, enabled: &[bool], checked: &[bool]) {
            let mut last = self.last.borrow_mut();
            if last.0 == enabled && last.1 == checked {
                return;
            }
            for (i, h) in self.handles.iter().enumerate() {
                match h {
                    Handle::Item(it) => it.set_enabled(enabled[i]),
                    Handle::Check(it) => {
                        it.set_enabled(enabled[i]);
                        it.set_checked(checked[i]);
                    }
                    Handle::Separator => {}
                }
            }
            *last = (enabled.to_vec(), checked.to_vec());
        }

        pub fn take_activated(&self) -> Vec<String> {
            self.queue.take()
        }
    }
}

/// The menu bar egui draws inside the window, from the same table.
mod in_window {
    use super::*;

    pub struct Menu {
        checked: std::cell::RefCell<Vec<bool>>,
        queue: Queue,
    }

    impl Menu {
        pub fn new() -> Menu {
            Menu { checked: vec![false; crate::menutable::item_count()].into(), queue: Queue::default() }
        }

        pub fn set_state(&self, _enabled: &[bool], checked: &[bool]) {
            self.checked.replace(checked.to_vec());
        }

        pub fn take_activated(&self) -> Vec<String> {
            self.queue.take()
        }

        /// Returns the id clicked this frame, if any.
        pub fn bar(&self, ui: &mut egui::Ui, state: &MenuState, tok: &Tokens) -> Option<String> {
            let checked = self.checked.borrow();
            let mut fired = None;
            egui::MenuBar::new().ui(ui, |ui| {
                // The base index of each menu is counted outside the button, because
                // a closed menu never runs its closure.
                let mut base = 0usize;
                for menu in MENUS {
                    let first = base;
                    base += menu.items.len();
                    ui.menu_button(menu.title, |ui| {
                        for (k, item) in menu.items.iter().enumerate() {
                            let at = first + k;
                            if item.id.is_empty() {
                                ui.separator();
                                continue;
                            }
                            let label = if item.check && checked[at] {
                                format!("✓ {}", item.label)
                            } else {
                                item.label.to_string()
                            };
                            let button = egui::Button::new(label).shortcut_text(crate::fmt::accel(item.keys));
                            if ui.add_enabled(item.need.met(state), button).clicked() {
                                fired = Some(item.id.to_string());
                                ui.close();
                            }
                        }
                    });
                }
                let _ = tok;
            });
            fired
        }
    }
}

/// Where this run's menu lives.
pub enum Menu {
    /// The platform's own bar, through muda.
    #[cfg(any(target_os = "macos", target_family = "windows"))]
    Platform(platform::Menu),
    /// Drawn in the window by egui.
    InWindow(in_window::Menu),
    /// Neither: the frames the tests draw have no menu at all.
    Headless,
}

impl Menu {
    /// The menu this platform gets. Windows takes the platform bar unless
    /// `SPECTAPE_EGUI_MENU` is set, which is the way back if a window ever
    /// refuses one.
    pub fn new(cc: &eframe::CreationContext<'_>) -> Menu {
        #[cfg(any(target_os = "macos", target_family = "windows"))]
        if std::env::var_os("SPECTAPE_EGUI_MENU").is_none() {
            if let Some(menu) = platform::Menu::new(&cc.egui_ctx, window_handle(cc)) {
                return Menu::Platform(menu);
            }
        }
        let _ = cc;
        Menu::InWindow(in_window::Menu::new())
    }

    /// A menu that talks to no platform, for drawing frames in a test.
    pub fn headless() -> Menu {
        Menu::Headless
    }

    /// The in-window bar, whatever the platform: what Linux runs, and what lets a
    /// test on any platform draw it.
    #[allow(dead_code)]
    pub fn in_window() -> Menu {
        Menu::InWindow(in_window::Menu::new())
    }

    /// Whether `bar` has anything to draw this frame.
    pub fn draws_in_window(&self) -> bool {
        matches!(self, Menu::InWindow(_))
    }

    /// Push the enabled and checked state of every item.
    pub fn set_state(&self, enabled: &[bool], checked: &[bool]) {
        match self {
            #[cfg(any(target_os = "macos", target_family = "windows"))]
            Menu::Platform(m) => m.set_state(enabled, checked),
            Menu::InWindow(m) => m.set_state(enabled, checked),
            Menu::Headless => {}
        }
    }

    /// Command ids activated since the last call.
    pub fn take_activated(&self) -> Vec<String> {
        match self {
            #[cfg(any(target_os = "macos", target_family = "windows"))]
            Menu::Platform(m) => m.take_activated(),
            Menu::InWindow(m) => m.take_activated(),
            Menu::Headless => Vec::new(),
        }
    }

    /// Draw the in-window bar; `None` when the platform owns the menu.
    pub fn bar(&self, ui: &mut egui::Ui, state: &MenuState, tok: &Tokens) -> Option<String> {
        match self {
            Menu::InWindow(m) => m.bar(ui, state, tok),
            _ => None,
        }
    }
}

/// The window to hang a menu on, as a plain handle: only Windows needs one.
#[cfg(target_family = "windows")]
fn window_handle(cc: &eframe::CreationContext<'_>) -> Option<isize> {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    match cc.window_handle().ok()?.as_raw() {
        RawWindowHandle::Win32(h) => Some(h.hwnd.get()),
        _ => None,
    }
}

/// macOS hangs the menu on the application, not on a window.
#[cfg(target_os = "macos")]
fn window_handle(_cc: &eframe::CreationContext<'_>) -> Option<isize> {
    None
}
