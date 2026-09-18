//! The native menu against `src/state/commands.ts`.
//!
//! The web app, the Tauri menu and this one all address commands by the same ids,
//! so the one thing worth testing about a skeleton menu is that none of them has
//! drifted. The table is included rather than imported because it belongs to a
//! binary crate, the way `build.rs` reads it.

include!("../src/menutable.rs");

/// Ids that `COMMANDS` has and no menu bar shows: the context menu owns them.
/// `src-tauri/src/menu.rs` leaves the same one out.
const NOT_IN_A_MENU: &[&str] = &["toggle-collapse"];

fn command_ids() -> Vec<String> {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/state/commands.ts");
    let source = std::fs::read_to_string(path).expect("src/state/commands.ts");
    // The table's keys are the only single-quoted strings at two spaces of indent
    // followed by a colon.
    source
        .lines()
        .filter_map(|line| {
            let rest = line.strip_prefix("  '")?;
            let (id, tail) = rest.split_once('\'')?;
            tail.starts_with(':').then(|| id.to_string())
        })
        .collect()
}

#[test]
fn every_menu_item_is_a_command() {
    let ids = command_ids();
    assert!(ids.len() > 40, "commands.ts parsed as {} ids", ids.len());
    for item in flat() {
        if item.id.is_empty() {
            continue;
        }
        assert!(ids.iter().any(|c| c == item.id), "menu item {:?} is not a command id", item.id);
    }
}

#[test]
fn every_command_is_in_the_menu() {
    let menu: Vec<&str> = flat().iter().map(|i| i.id).collect();
    for id in command_ids() {
        if NOT_IN_A_MENU.contains(&id.as_str()) {
            continue;
        }
        assert!(menu.contains(&id.as_str()), "command {id:?} has no menu item");
    }
}

#[test]
fn ids_are_unique_and_flat_order_is_stable() {
    let items = flat();
    assert_eq!(items.len(), item_count());
    let mut ids: Vec<&str> = items.iter().map(|i| i.id).filter(|i| !i.is_empty()).collect();
    let before = ids.len();
    ids.sort_unstable();
    ids.dedup();
    assert_eq!(before, ids.len(), "duplicate command id in the menu");
}

#[test]
fn enabled_flags_follow_the_tape() {
    let empty = enabled_flags(0, false);
    let loaded = enabled_flags(19, true);
    assert_eq!(empty.len(), item_count());
    let index = |id: &str| flat().iter().position(|i| i.id == id).unwrap();
    assert!(!empty[index("save")], "Save is enabled without blocks");
    assert!(loaded[index("save")], "Save is disabled with a tape loaded");
    assert!(!loaded[index("undo")], "Undo has nothing to undo in the skeleton");
    assert!(loaded[index("new")] && empty[index("new")], "New is always available");
}
