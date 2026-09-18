// The command table the native menu is built from.
//
// `menu.rs` builds the platform menu from it — muda on macOS and Windows, an egui
// bar elsewhere — and `tests/menu.rs` checks it against the web app's table. The
// ids are the ids of `COMMANDS` in `src/state/commands.ts`, so the desktop app and
// the browser one cannot drift apart.
//
// What an item *does* lives in `commands.rs`; what it is called, what it is
// bound to and when it is enabled lives here, because the menu, the context
// menu, the keyboard and the tests all have to agree on those.

/// When an item is enabled: the port of the `enabled` predicates in
/// `commands.ts`, as a table rather than as closures, so `tests/menu.rs` can
/// read it without the rest of the app.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Need {
    /// Always enabled.
    Always,
    /// Needs a tape with at least one block.
    Blocks,
    /// Needs a block under the cursor.
    Cursor,
    /// Needs something on the undo or redo stack.
    Undo,
    Redo,
    /// Needs blocks on the clipboard.
    Clipboard,
    /// Needs a tape that is playing.
    Playing,
    /// Needs the cursor on a group or loop start.
    Collapsible,
}

/// The state the enabled rules ask about.
#[derive(Clone, Copy, Default)]
pub struct MenuState {
    pub blocks: usize,
    pub has_cursor: bool,
    pub can_undo: bool,
    pub can_redo: bool,
    pub clipboard: bool,
    pub playing: bool,
    pub collapsible: bool,
}

impl Need {
    pub fn met(self, s: &MenuState) -> bool {
        match self {
            Need::Always => true,
            Need::Blocks => s.blocks > 0,
            Need::Cursor => s.has_cursor,
            Need::Undo => s.can_undo,
            Need::Redo => s.can_redo,
            Need::Clipboard => s.clipboard,
            Need::Playing => s.playing,
            Need::Collapsible => s.collapsible,
        }
    }
}

pub struct Item {
    /// Command id, matching `COMMANDS` in `src/state/commands.ts`. Empty for a separator.
    pub id: &'static str,
    pub label: &'static str,
    /// Accelerator in muda's spelling: `CmdOrCtrl` is ⌘ on macOS and Ctrl
    /// elsewhere. Empty for none.
    pub keys: &'static str,
    pub check: bool,
    pub need: Need,
}

pub struct MenuDef {
    pub title: &'static str,
    pub items: &'static [Item],
}

const fn cmd(id: &'static str, label: &'static str, keys: &'static str, need: Need) -> Item {
    Item { id, label, keys, check: false, need }
}

const fn check(id: &'static str, label: &'static str, keys: &'static str) -> Item {
    Item { id, label, keys, check: true, need: Need::Always }
}

const fn sep() -> Item {
    Item { id: "", label: "", keys: "", check: false, need: Need::Always }
}

use Need::{Always, Blocks, Clipboard, Cursor, Playing, Redo, Undo};

pub const MENUS: &[MenuDef] = &[
    MenuDef {
        title: "File",
        items: &[
            cmd("new", "New Tape", "CmdOrCtrl+N", Always),
            cmd("open", "Open…", "CmdOrCtrl+O", Always),
            cmd("open-other", "Open in Other Pane…", "CmdOrCtrl+Shift+O", Always),
            cmd("insert-file", "Insert File at Cursor…", "", Always),
            sep(),
            cmd("save", "Save", "CmdOrCtrl+S", Blocks),
            cmd("save-as", "Save As TZX…", "CmdOrCtrl+Shift+S", Blocks),
            cmd("save-tap", "Save As TAP…", "", Blocks),
            cmd("export-wav", "Export WAV…", "CmdOrCtrl+E", Blocks),
        ],
    },
    MenuDef {
        title: "Edit",
        items: &[
            cmd("undo", "Undo", "CmdOrCtrl+Z", Undo),
            cmd("redo", "Redo", "CmdOrCtrl+Shift+Z", Redo),
            sep(),
            cmd("cut", "Cut", "CmdOrCtrl+X", Cursor),
            cmd("copy", "Copy", "CmdOrCtrl+C", Cursor),
            cmd("paste", "Paste", "CmdOrCtrl+V", Clipboard),
            cmd("duplicate", "Duplicate Block", "CmdOrCtrl+D", Cursor),
            cmd("delete", "Delete Block", "", Cursor),
            sep(),
            cmd("select-all", "Select All", "CmdOrCtrl+A", Blocks),
        ],
    },
    MenuDef {
        title: "Block",
        items: &[
            cmd("insert", "Insert Block…", "CmdOrCtrl+Shift+N", Always),
            cmd("view-data", "View Data", "", Cursor),
            cmd("view-as-one", "View Selected as One", "", Cursor),
            sep(),
            cmd("move-up", "Move Up", "CmdOrCtrl+Up", Cursor),
            cmd("move-down", "Move Down", "CmdOrCtrl+Down", Cursor),
            cmd("group", "Group Selection", "CmdOrCtrl+G", Cursor),
            cmd("collapse-all", "Collapse All Groups", "", Always),
            cmd("expand-all", "Expand All Groups", "", Always),
            sep(),
            cmd("select-program", "Select Program", "CmdOrCtrl+Shift+A", Cursor),
            cmd("extract", "Extract to Other Pane", "CmdOrCtrl+Shift+E", Cursor),
            sep(),
            cmd("find-match", "Find Match", "CmdOrCtrl+F", Cursor),
            cmd("set-timings", "Set Selection Timings to Current", "", Cursor),
        ],
    },
    MenuDef {
        title: "Tape",
        items: &[
            cmd("play", "Play Tape", "", Blocks),
            cmd("play-cursor", "Play from Cursor", "CmdOrCtrl+P", Blocks),
            cmd("play-selection", "Play Selection", "", Blocks),
            cmd("stop", "Stop Playback", "CmdOrCtrl+.", Playing),
            sep(),
            cmd("emu-tape", "Open Tape in Emulator", "CmdOrCtrl+R", Blocks),
            cmd("emu-cursor", "Open from Cursor in Emulator", "CmdOrCtrl+Shift+R", Cursor),
            cmd("emu-selection", "Open Selection in Emulator", "", Cursor),
            sep(),
            cmd("programs", "Programs…", "CmdOrCtrl+J", Blocks),
            cmd("tape-info", "Tape Info…", "CmdOrCtrl+I", Blocks),
            cmd("consistency", "Check Consistency…", "CmdOrCtrl+K", Blocks),
            cmd("compare", "Compare Tapes", "", Always),
            cmd("clear-compare", "Clear Compare Marks", "", Always),
            sep(),
            cmd("switch-pane", "Switch Active Pane", "CmdOrCtrl+`", Always),
            cmd("toggle-lock", "Toggle Lock", "CmdOrCtrl+L", Always),
        ],
    },
    MenuDef {
        title: "Options",
        items: &[
            check("toggle-hex", "Hex for All Numbers", "CmdOrCtrl+H"),
            check("opt-hex-bytes", "Flag and Checksum Bytes in Hex", ""),
            check("opt-zero-based", "Number Blocks from 0", ""),
            sep(),
            cmd("emu-settings", "Emulator…", "", Always),
        ],
    },
    MenuDef {
        title: "Help",
        items: &[
            cmd("shortcuts", "Keyboard Shortcuts…", "", Always),
            cmd("about", "About SpecTape…", "", Always),
        ],
    },
];

/// Flat index of every item, the order `menu.rs` addresses them in.
pub fn flat() -> Vec<&'static Item> {
    MENUS.iter().flat_map(|m| m.items.iter()).collect()
}

/// A guard against the two readers disagreeing: both use this order. Read by
/// `tests/menu.rs` and by the in-window menu bar.
#[allow(dead_code)]
pub fn item_count() -> usize {
    MENUS.iter().map(|m| m.items.len()).sum()
}

/// The table entry for a command id, wherever it is shown.
pub fn item(id: &str) -> Option<&'static Item> {
    MENUS.iter().flat_map(|m| m.items.iter()).find(|i| i.id == id)
}

/// Enabled state for every item, in flat order.
pub fn enabled_flags(state: &MenuState) -> Vec<bool> {
    flat().iter().map(|i| i.need.met(state)).collect()
}
