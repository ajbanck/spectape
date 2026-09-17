//! Native application menu. Every item forwards its id to the front end as a `menu` event;
//! the front end decides whether the command applies to a text field or to the tape.

use tauri::menu::{AboutMetadata, CheckMenuItem, CheckMenuItemBuilder, Menu, MenuItemBuilder, PredefinedMenuItem, SubmenuBuilder};
use tauri::{AppHandle, Manager, Runtime};

/// Ids of the check items in the Options menu. The front end owns their state (it persists the
/// options) and pushes it back with `set_menu_checked`, since a click only toggles the native mark.
pub const OPTION_IDS: [&str; 3] = ["toggle-hex", "opt-hex-bytes", "opt-zero-based"];

pub struct OptionItems<R: Runtime>(pub Vec<CheckMenuItem<R>>);

fn check<R: Runtime>(app: &AppHandle<R>, id: &str, label: &str, accel: Option<&str>) -> tauri::Result<CheckMenuItem<R>> {
    let mut b = CheckMenuItemBuilder::with_id(id, label);
    if let Some(a) = accel {
        b = b.accelerator(a);
    }
    b.build(app)
}

fn item<R: Runtime>(app: &AppHandle<R>, id: &str, label: &str, accel: Option<&str>) -> tauri::Result<tauri::menu::MenuItem<R>> {
    let mut b = MenuItemBuilder::with_id(id, label);
    if let Some(a) = accel {
        b = b.accelerator(a);
    }
    b.build(app)
}

pub fn build<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<Menu<R>> {
    let about = AboutMetadata {
        name: Some("SpecTape".into()),
        version: Some(env!("CARGO_PKG_VERSION").into()),
        comments: Some("ZX Spectrum TZX/TAP tape editor".into()),
        ..Default::default()
    };

    #[cfg(target_os = "macos")]
    let app_menu = SubmenuBuilder::new(app, "SpecTape")
        .item(&PredefinedMenuItem::about(app, Some("About SpecTape"), Some(about.clone()))?)
        .separator()
        .item(&item(app, "shortcuts", "Keyboard Shortcuts…", None)?)
        .separator()
        .services()
        .separator()
        .hide()
        .hide_others()
        .show_all()
        .separator()
        .quit()
        .build()?;

    let file = SubmenuBuilder::new(app, "File")
        .item(&item(app, "new", "New Tape", Some("CmdOrCtrl+N"))?)
        .item(&item(app, "open", "Open…", Some("CmdOrCtrl+O"))?)
        .item(&item(app, "open-other", "Open in Other Pane…", Some("Shift+CmdOrCtrl+O"))?)
        .item(&item(app, "insert-file", "Insert File at Cursor…", None)?)
        .separator()
        .item(&item(app, "save", "Save", Some("CmdOrCtrl+S"))?)
        .item(&item(app, "save-as", "Save As TZX…", Some("Shift+CmdOrCtrl+S"))?)
        .item(&item(app, "save-tap", "Save As TAP…", None)?)
        .item(&item(app, "export-wav", "Export WAV…", Some("CmdOrCtrl+E"))?)
        .separator()
        .close_window()
        .build()?;

    let edit = SubmenuBuilder::new(app, "Edit")
        .item(&item(app, "undo", "Undo", Some("CmdOrCtrl+Z"))?)
        .item(&item(app, "redo", "Redo", Some("Shift+CmdOrCtrl+Z"))?)
        .separator()
        .item(&item(app, "cut", "Cut", Some("CmdOrCtrl+X"))?)
        .item(&item(app, "copy", "Copy", Some("CmdOrCtrl+C"))?)
        .item(&item(app, "paste", "Paste", Some("CmdOrCtrl+V"))?)
        .item(&item(app, "duplicate", "Duplicate Block", Some("CmdOrCtrl+D"))?)
        .item(&item(app, "delete", "Delete Block", None)?)
        .separator()
        .item(&item(app, "select-all", "Select All", Some("CmdOrCtrl+A"))?)
        .build()?;

    let block = SubmenuBuilder::new(app, "Block")
        .item(&item(app, "insert", "Insert Block…", Some("CmdOrCtrl+Shift+N"))?)
        .item(&item(app, "view-data", "View Data", None)?)
        .item(&item(app, "view-as-one", "View Selected as One", None)?)
        .separator()
        .item(&item(app, "move-up", "Move Up", Some("CmdOrCtrl+Up"))?)
        .item(&item(app, "move-down", "Move Down", Some("CmdOrCtrl+Down"))?)
        .item(&item(app, "group", "Group Selection", Some("CmdOrCtrl+G"))?)
        .item(&item(app, "collapse-all", "Collapse All Groups", None)?)
        .item(&item(app, "expand-all", "Expand All Groups", None)?)
        .separator()
        .item(&item(app, "select-program", "Select Program", Some("Shift+CmdOrCtrl+A"))?)
        .item(&item(app, "extract", "Extract to Other Pane", Some("Shift+CmdOrCtrl+E"))?)
        .separator()
        .item(&item(app, "find-match", "Find Match", Some("CmdOrCtrl+F"))?)
        .item(&item(app, "set-timings", "Set Selection Timings to Current", None)?)
        .build()?;

    let tape = SubmenuBuilder::new(app, "Tape")
        .item(&item(app, "play", "Play Tape", None)?)
        .item(&item(app, "play-cursor", "Play from Cursor", Some("CmdOrCtrl+P"))?)
        .item(&item(app, "play-selection", "Play Selection", None)?)
        .item(&item(app, "stop", "Stop Playback", Some("CmdOrCtrl+."))?)
        .separator()
        .item(&item(app, "emu-tape", "Open Tape in Emulator", Some("CmdOrCtrl+R"))?)
        .item(&item(app, "emu-cursor", "Open from Cursor in Emulator", Some("Shift+CmdOrCtrl+R"))?)
        .item(&item(app, "emu-selection", "Open Selection in Emulator", None)?)
        .separator()
        .item(&item(app, "programs", "Programs…", Some("CmdOrCtrl+J"))?)
        .item(&item(app, "tape-info", "Tape Info…", Some("CmdOrCtrl+I"))?)
        .item(&item(app, "consistency", "Check Consistency…", Some("CmdOrCtrl+K"))?)
        .item(&item(app, "compare", "Compare Tapes", None)?)
        .item(&item(app, "clear-compare", "Clear Compare Marks", None)?)
        .separator()
        .item(&item(app, "switch-pane", "Switch Active Pane", Some("CmdOrCtrl+`"))?)
        .item(&item(app, "toggle-lock", "Toggle Lock", Some("CmdOrCtrl+L"))?)
        .build()?;

    let opt_items = vec![
        check(app, OPTION_IDS[0], "Hex for All Numbers", Some("CmdOrCtrl+H"))?,
        check(app, OPTION_IDS[1], "Flag and Checksum Bytes in Hex", None)?,
        check(app, OPTION_IDS[2], "Number Blocks from 0", None)?,
    ];
    let options = SubmenuBuilder::new(app, "Options")
        .items(&[&opt_items[0], &opt_items[1], &opt_items[2]])
        .separator()
        .item(&item(app, "emu-settings", "Emulator…", None)?)
        .build()?;
    app.manage(OptionItems(opt_items));

    let window = SubmenuBuilder::new(app, "Window").minimize().maximize().separator().fullscreen().build()?;

    let help = SubmenuBuilder::new(app, "Help")
        .item(&item(app, "shortcuts", "Keyboard Shortcuts…", None)?)
        .item(&item(app, "about", "About SpecTape…", None)?)
        .build()?;

    let mut mb = tauri::menu::MenuBuilder::new(app);
    #[cfg(target_os = "macos")]
    {
        mb = mb.item(&app_menu);
    }
    let _ = about;
    mb.items(&[&file, &edit, &block, &tape, &options, &window, &help]).build()
}
