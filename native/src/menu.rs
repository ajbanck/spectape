//! The application menu, built from the table in `menutable.rs`.
//!
//! On macOS this is the platform's own menu bar: `muda` sets it on `NSApp`, the
//! same crate `src-tauri/src/menu.rs` reaches through Tauri today, so the native
//! shell's menu is the menu SpecTape already has — accelerators included, with
//! `CmdOrCtrl+S` meaning ⌘S here and Ctrl+S on Windows.
//!
//! Elsewhere the menu is drawn inside the window by egui (`fallback_bar`). On
//! Windows that is temporary: muda wants `init_for_hwnd`, which needs the window
//! handle out of eframe, and it belongs with the rest of the Windows work.
//!
//! Clicks arrive on muda's own thread, so they land in a queue and wake the UI;
//! `take_activated` drains it at the top of a frame.

use std::sync::{Arc, Mutex};

use crate::menutable::{Item, MENUS};

#[derive(Default)]
pub struct Queue(Arc<Mutex<Vec<String>>>);

impl Queue {
    /// Command ids activated since the last call.
    pub fn take(&self) -> Vec<String> {
        std::mem::take(&mut *self.0.lock().unwrap())
    }
}

#[cfg(target_os = "macos")]
mod native {
    use super::*;
    use muda::accelerator::Accelerator;
    use muda::{AboutMetadata, CheckMenuItem, Menu as MudaMenu, MenuItem, PredefinedMenuItem, Submenu};

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
        pub fn new(ctx: &egui::Context) -> Self {
            let bar = MudaMenu::new();

            // macOS expects the first menu to be the application's own; muda, unlike
            // Slint, does not add it for you.
            let about = AboutMetadata {
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
            bar.init_for_nsapp();

            let queue = Queue::default();
            let sink = queue.0.clone();
            let ctx = ctx.clone();
            muda::MenuEvent::set_event_handler(Some(move |event: muda::MenuEvent| {
                sink.lock().unwrap().push(event.id.0.clone());
                // The click happens outside egui's event loop, so ask for a frame.
                ctx.request_repaint();
            }));

            Menu { handles, _bar: bar, queue }
        }

        pub fn set_enabled(&self, flags: &[bool]) {
            for (h, on) in self.handles.iter().zip(flags) {
                match h {
                    Handle::Item(i) => i.set_enabled(*on),
                    Handle::Check(i) => i.set_enabled(*on),
                    Handle::Separator => {}
                }
            }
        }

        pub fn set_checked(&self, flat: usize, on: bool) {
            if let Some(Handle::Check(i)) = self.handles.get(flat) {
                i.set_checked(on);
            }
        }

        pub fn take_activated(&self) -> Vec<String> {
            self.queue.take()
        }
    }
}

#[cfg(not(target_os = "macos"))]
mod native {
    use super::*;

    pub struct Menu {
        enabled: std::cell::RefCell<Vec<bool>>,
        checked: std::cell::RefCell<Vec<bool>>,
        queue: Queue,
    }

    impl Menu {
        pub fn new(_ctx: &egui::Context) -> Self {
            let n = crate::menutable::item_count();
            Menu { enabled: vec![true; n].into(), checked: vec![false; n].into(), queue: Queue::default() }
        }

        pub fn set_enabled(&self, flags: &[bool]) {
            self.enabled.replace(flags.to_vec());
        }

        pub fn set_checked(&self, flat: usize, on: bool) {
            self.checked.borrow_mut()[flat] = on;
        }

        pub fn take_activated(&self) -> Vec<String> {
            self.queue.take()
        }

        /// The in-window menu bar, drawn from the same table.
        pub fn bar(&self, ui: &mut egui::Ui) {
            let enabled = self.enabled.borrow();
            let checked = self.checked.borrow();
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
                            let button = egui::Button::new(label).shortcut_text(item.keys);
                            if ui.add_enabled(enabled[at], button).clicked() {
                                self.queue.0.lock().unwrap().push(item.id.to_string());
                                ui.close();
                            }
                        }
                    });
                }
            });
        }
    }
}

pub use native::Menu;
