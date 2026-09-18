// The command table the native menu is built from.
//
// `menu.rs` builds the platform menu from it — muda on macOS, an egui bar
// elsewhere — and `tests/menu.rs` checks it against the web app's table. The ids
// are the ids of `COMMANDS` in `src/state/commands.ts`, which
// `src-tauri/src/menu.rs` already shares, so the three menus cannot drift apart.
//
// The skeleton does not run most of the commands: activating one reports its id
// in the status bar. Stage 4 fills them in area by area, against the parity
// checklist in docs/rust-migration.md.

/// When an item is enabled. The port of the `enabled` predicates in `commands.ts`;
/// stage 3 only knows about the tape, not about a clipboard or an undo stack.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Need {
    /// Always enabled.
    Always,
    /// Needs a tape with at least one block.
    Blocks,
    /// Needs a block under the cursor.
    Cursor,
    /// Needs state the skeleton does not keep yet (an undo stack, a clipboard,
    /// a second pane, a playing tape), so it is greyed out here.
    Never,
}

pub struct Item {
    /// Command id, matching `COMMANDS` in `src/state/commands.ts`. Empty for a separator.
    pub id: &'static str,
    pub label: &'static str,
    /// Accelerator in muda's spelling, the same strings `src-tauri/src/menu.rs`
    /// gives Tauri: `CmdOrCtrl` is ⌘ on macOS and Ctrl elsewhere. Empty for none.
    pub keys: &'static str,
    pub check: bool,
    pub need: Need,
}

pub struct MenuDef {
    /// Read by `build.rs`, not at runtime.
    #[allow(dead_code)]
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

use Need::{Always, Blocks, Cursor, Never};

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
            cmd("undo", "Undo", "CmdOrCtrl+Z", Never),
            cmd("redo", "Redo", "CmdOrCtrl+Shift+Z", Never),
            sep(),
            cmd("cut", "Cut", "CmdOrCtrl+X", Cursor),
            cmd("copy", "Copy", "CmdOrCtrl+C", Cursor),
            cmd("paste", "Paste", "CmdOrCtrl+V", Never),
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
            cmd("find-match", "Find Match", "CmdOrCtrl+F", Never),
            cmd("set-timings", "Set Selection Timings to Current", "", Cursor),
        ],
    },
    MenuDef {
        title: "Tape",
        items: &[
            cmd("play", "Play Tape", "", Blocks),
            cmd("play-cursor", "Play from Cursor", "CmdOrCtrl+P", Cursor),
            cmd("play-selection", "Play Selection", "", Cursor),
            cmd("stop", "Stop Playback", "CmdOrCtrl+.", Never),
            sep(),
            cmd("emu-tape", "Open Tape in Emulator", "CmdOrCtrl+R", Blocks),
            cmd("emu-cursor", "Open from Cursor in Emulator", "CmdOrCtrl+Shift+R", Cursor),
            cmd("emu-selection", "Open Selection in Emulator", "", Cursor),
            sep(),
            cmd("programs", "Programs…", "CmdOrCtrl+J", Blocks),
            cmd("tape-info", "Tape Info…", "CmdOrCtrl+I", Blocks),
            cmd("consistency", "Check Consistency…", "CmdOrCtrl+K", Blocks),
            cmd("compare", "Compare Tapes", "", Never),
            cmd("clear-compare", "Clear Compare Marks", "", Never),
            sep(),
            cmd("switch-pane", "Switch Active Pane", "CmdOrCtrl+`", Never),
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

/// Flat index of every item, the way the generated markup addresses them.
#[allow(dead_code)]
pub fn flat() -> Vec<&'static Item> {
    MENUS.iter().flat_map(|m| m.items.iter()).collect()
}

/// A guard against the two readers disagreeing: both use this order.
#[allow(dead_code)]
pub fn item_count() -> usize {
    MENUS.iter().map(|m| m.items.len()).sum()
}

/// Enabled state for every item, in flat order.
#[allow(dead_code)]
pub fn enabled_flags(blocks: usize, has_cursor: bool) -> Vec<bool> {
    flat()
        .iter()
        .map(|i| match i.need {
            Need::Always => true,
            Need::Blocks => blocks > 0,
            Need::Cursor => has_cursor,
            Need::Never => false,
        })
        .collect()
}
