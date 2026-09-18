//! SpecTape's native shell.
//!
//! A window, the platform's menu built from one command table, two tape panes
//! with their editors, the data window, the dialogs and playback — all on
//! `spectape-core` linked directly: no wasm, no wire format, no web view.
//! Stage 3 of docs/rust-migration.md chose the toolkit and measured the
//! boundary; stage 4 filled the app in area by area against the parity
//! checklist there.
//!
//!     spectape-native [TAPE…] [--rows N] [--bench N] [--exit-on-draw] [--measure] [--hex]
//!
//! `--rows` repeats the tape's blocks until the list is N rows long (3,000 is the
//! size the plan measures rendering with), `--bench` moves the cursor once per
//! frame N times and reports the distribution, `--exit-on-draw` quits on the
//! first frame, so `time spectape-native …` is the cold start, and `--measure`
//! prints what the core costs in process, plus what the app's own first frame
//! costs — all of it without opening a window.

#![windows_subsystem = "windows"]

mod actions;
mod app;
mod commands;
mod datawin;
mod dialogs;
mod editor;
mod emulator;
mod files;
mod fmt;
mod icons;
mod list;
mod menu;
mod menutable;
mod player;
mod settings;
mod state;
mod statusbar;
mod tables;
mod tape;
mod theme;
mod widgets;

use std::path::PathBuf;
use std::time::Instant;

use spectape_core::types::Block;
use spectape_core::{consistency, content, describe, programs, writer};

use settings::Settings;
use state::Store;

/// Where a tape comes from when none is named on the command line.
const DEFAULT_TAPE: &str = "public/samples/SpecTape demo.tzx";

struct Opts {
    paths: Vec<PathBuf>,
    rows: Option<usize>,
    bench: Option<usize>,
    exit_on_draw: bool,
    measure: bool,
    hex: bool,
}

fn parse_args() -> Opts {
    let mut o =
        Opts { paths: Vec::new(), rows: None, bench: None, exit_on_draw: false, measure: false, hex: false };
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--rows" => o.rows = args.next().and_then(|v| v.parse().ok()),
            "--bench" => o.bench = args.next().and_then(|v| v.parse().ok()),
            "--exit-on-draw" => o.exit_on_draw = true,
            "--measure" => o.measure = true,
            "--hex" => o.hex = true,
            "-h" | "--help" => {
                println!(
                    "spectape-native [TAPE…] [--rows N] [--bench N] [--exit-on-draw] [--measure] [--hex]"
                );
                std::process::exit(0);
            }
            // macOS hands a bundled app a process serial number argument.
            _ if a.starts_with('-') => {}
            _ => o.paths.push(PathBuf::from(a)),
        }
    }
    o
}

/// The sample tape, looked for up the tree from the working directory and next to
/// the crate, so both `cargo run` and the bundled app find it.
fn default_tape() -> Option<PathBuf> {
    let mut roots: Vec<PathBuf> = Vec::new();
    if let Ok(cwd) = std::env::current_dir() {
        let mut dir = Some(cwd.as_path());
        while let Some(d) = dir {
            roots.push(d.to_path_buf());
            dir = d.parent();
        }
    }
    roots.push(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(".."));
    roots.into_iter().map(|r| r.join(DEFAULT_TAPE)).find(|p| p.is_file())
}

/// Repeat the tape's blocks until the list is `rows` long, the way the stage 2
/// benchmarks built their 3,000-block tape.
fn grow(blocks: &[Block], rows: usize) -> Vec<Block> {
    if blocks.is_empty() {
        return Vec::new();
    }
    (0..rows).map(|i| blocks[i % blocks.len()].clone_fresh()).collect()
}

/// The stage 2 boundary table, one call per line, with the wire encoding gone:
/// this is the same work the web app pays for through wasm.
fn measure(blocks: &[Block], parse_ms: f64, hex: bool) {
    fn ms(f: impl FnOnce()) -> f64 {
        let t = Instant::now();
        f();
        t.elapsed().as_secs_f64() * 1000.0
    }
    println!("{} blocks, in process, no wire format:", blocks.len());
    println!("  parse                          {parse_ms:7.2} ms");
    let mut sink = 0usize;
    let describe = ms(|| {
        for b in blocks {
            sink += describe::describe_block(b, hex).len() + describe::block_length(b) as usize;
        }
    });
    println!("  describeBlock + blockLength    {describe:7.2} ms");
    println!(
        "  content labels                 {:7.2} ms",
        ms(|| sink += content::content_labels(blocks).len())
    );
    println!(
        "  checkConsistency               {:7.2} ms",
        ms(|| sink += consistency::check_consistency(blocks, 1).len())
    );
    println!(
        "  detectPrograms                 {:7.2} ms",
        ms(|| sink += programs::detect_programs(blocks).len())
    );
    println!(
        "  groupRanges                    {:7.2} ms",
        ms(|| sink += programs::group_ranges(blocks).len())
    );
    println!(
        "  requiredVersion                {:7.2} ms",
        ms(|| sink += writer::required_version(blocks).major as usize)
    );
    println!(
        "  serializeTzx                   {:7.2} ms",
        ms(|| sink += writer::serialize_tzx(blocks, None).len())
    );
    debug_assert!(sink > 0);
}

/// What the app's own first frame costs before a window is involved: laying out
/// and tessellating every panel, and rasterising the glyphs it uses. This is the
/// half of cold start that can be measured from a terminal — the other half is
/// process start, window creation and the GL context, and it needs a screen.
///
/// The two font sets are timed against each other because trimming them was the
/// obvious suspect for a slow start. It is not: the gap is about a millisecond.
fn measure_first_frame(store: Store) {
    let mut store = Some(store);
    println!("\nfirst headless frame (no window, no GL context, warm page cache):");
    for (label, fonts) in
        [("egui's default fonts", None), ("without the emoji fonts", Some(theme::latin_only_fonts()))]
    {
        let ctx = egui::Context::default();
        if let Some(fonts) = fonts {
            ctx.set_fonts(fonts);
        }
        // A fresh store per run: the second frame would find the rows cached.
        let taken = store.take().unwrap();
        let name = taken.tape(0).name.clone();
        let blocks = taken.tape(0).blocks.clone();
        let mut next = Store::new(Settings::default());
        next.tape_mut(0).load(name, None, blocks, None);
        let mut app = app::App::build(&ctx, menu::Menu::headless(), taken, Instant::now(), 0, false);
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1200.0, 800.0))),
            ..Default::default()
        };
        let t = Instant::now();
        ctx.run_ui(input, |ui| app.frame(ui)).drop_without_applying_deltas();
        println!("  {label:<24} {:7.2} ms", t.elapsed().as_secs_f64() * 1000.0);
        store = Some(next);
    }
}

fn main() -> eframe::Result<()> {
    let t0 = Instant::now();
    let opts = parse_args();
    let mut settings = Settings::load();
    if opts.hex {
        settings.hex_bytes = true;
    }
    let mut store = Store::new(settings);
    store.hex = opts.hex;

    // The command line is the "open with" queue `src-tauri/src/lib.rs` drains:
    // a tape named on it goes into the left pane, a second into the right.
    let t_parse = Instant::now();
    let paths: Vec<PathBuf> =
        if opts.paths.is_empty() { default_tape().into_iter().collect() } else { opts.paths.clone() };
    files::open_with(&mut store, &paths);
    let parse_ms = t_parse.elapsed().as_secs_f64() * 1000.0;
    store.active = 0;

    if let Some(n) = opts.rows {
        let blocks = &store.tape(0).blocks;
        let grown = if n > blocks.len() { grow(blocks, n) } else { blocks[..n.min(blocks.len())].to_vec() };
        let name = store.tape(0).name.clone();
        store.tape_mut(0).load(name, None, grown, None);
    }

    if opts.measure {
        measure(&store.tape(0).blocks, parse_ms, opts.hex);
        measure_first_frame(store);
        return Ok(());
    }

    let blocks = &store.tape(0).blocks;
    println!("core: parse {parse_ms:.1} ms, {} blocks", blocks.len());
    if let Some(v) = store.tape(0).loaded_version {
        println!("{}: TZX {}.{:02} as loaded", store.tape(0).name, v.major, v.minor);
    }

    let bench = opts.bench.unwrap_or(0);
    let exit_on_draw = opts.exit_on_draw;
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("SpecTape")
            .with_inner_size([1100.0, 720.0])
            .with_min_inner_size([680.0, 420.0]),
        ..Default::default()
    };
    eframe::run_native(
        "SpecTape",
        native_options,
        Box::new(move |cc| Ok(Box::new(app::App::new(cc, store, t0, bench, exit_on_draw)))),
    )
}
