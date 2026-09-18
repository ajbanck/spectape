//! SpecTape's native shell — the egui skeleton stage 4 builds on.
//!
//! A window, the platform's menu built from one command table, and a read-only
//! block list, all on `spectape-core` linked directly: no wasm, no wire format,
//! no web view. Stage 3 of docs/rust-migration.md chose the toolkit and measured
//! the boundary; stage 4 fills the app in area by area against the parity
//! checklist there.
//!
//!     spectape-native [TAPE] [--rows N] [--bench N] [--exit-on-draw] [--measure] [--hex]
//!
//! `--rows` repeats the tape's blocks until the list is N rows long (3,000 is the
//! size the plan measures rendering with), `--bench` moves the cursor once per
//! frame N times and reports the distribution, `--exit-on-draw` quits on the
//! first frame, so `time spectape-native …` is the cold start, and `--measure`
//! prints what the core costs in process and opens no window at all.

#![windows_subsystem = "windows"]

mod app;
mod menu;
mod menutable;

use std::path::{Path, PathBuf};
use std::time::Instant;

use spectape_core::types::{Block, Body};
use spectape_core::{audio, consistency, content, describe, parser, programs, writer};

use app::Row;

/// Where a tape comes from when none is named on the command line.
const DEFAULT_TAPE: &str = "public/samples/SpecTape demo.tzx";

struct Opts {
    path: Option<PathBuf>,
    rows: Option<usize>,
    bench: Option<usize>,
    exit_on_draw: bool,
    measure: bool,
    hex: bool,
}

fn parse_args() -> Opts {
    let mut o = Opts { path: None, rows: None, bench: None, exit_on_draw: false, measure: false, hex: false };
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
                    "spectape-native [TAPE] [--rows N] [--bench N] [--exit-on-draw] [--measure] [--hex]"
                );
                std::process::exit(0);
            }
            // macOS hands a bundled app a process serial number argument.
            _ if a.starts_with('-') => {}
            _ => o.path = Some(PathBuf::from(a)),
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

struct Tape {
    blocks: Vec<Block>,
    name: String,
    major: u8,
    minor: u8,
    warnings: Vec<String>,
}

fn load(opts: &Opts) -> Tape {
    let path = opts.path.clone().or_else(default_tape);
    let Some(path) = path else {
        return Tape {
            blocks: Vec::new(),
            name: "No tape loaded".into(),
            major: 0,
            minor: 0,
            warnings: vec![format!("{DEFAULT_TAPE} not found; pass a tape on the command line")],
        };
    };
    let bytes = match std::fs::read(&path) {
        Ok(b) => b,
        Err(e) => {
            return Tape {
                blocks: Vec::new(),
                name: file_name(&path),
                major: 0,
                minor: 0,
                warnings: vec![format!("{}: {e}", path.display())],
            }
        }
    };
    match parser::parse_tape(&bytes) {
        Ok(t) => Tape {
            blocks: t.blocks,
            name: file_name(&path),
            major: t.major,
            minor: t.minor,
            warnings: t.warnings,
        },
        Err(e) => Tape { blocks: Vec::new(), name: file_name(&path), major: 0, minor: 0, warnings: vec![e] },
    }
}

fn file_name(p: &Path) -> String {
    p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
}

/// Repeat the tape's blocks until the list is `rows` long, the way the stage 2
/// benchmarks built their 3,000-block tape.
fn grow(blocks: &[Block], rows: usize) -> Vec<Block> {
    if blocks.is_empty() {
        return Vec::new();
    }
    (0..rows).map(|i| blocks[i % blocks.len()].clone_fresh()).collect()
}

/// `category()` in src/ui/TapePane.tsx, as an index into `app::tok::CAT`.
fn category(b: &Block) -> usize {
    if b.body.is_unknown() {
        return 5;
    }
    match b.id() {
        0x10 | 0x11 | 0x14 | 0x15 | 0x18 | 0x19 => 0,
        0x12 | 0x13 | 0x2b => 1,
        0x21 | 0x22 => 3,
        0x20 | 0x23..=0x2a => 2,
        _ => 4,
    }
}

/// Indent per block, from the group and loop nesting the web list shows.
fn depths(blocks: &[Block]) -> Vec<i32> {
    let mut depth = vec![0; blocks.len()];
    let mut level = 0i32;
    for (i, b) in blocks.iter().enumerate() {
        if matches!(b.body, Body::GroupEnd | Body::LoopEnd) {
            level = (level - 1).max(0);
        }
        depth[i] = level;
        if matches!(b.body, Body::GroupStart { .. } | Body::LoopStart { .. }) {
            level += 1;
        }
    }
    depth
}

fn build_rows(blocks: &[Block], hex: bool) -> Vec<Row> {
    let kinds = content::content_labels(blocks);
    let depth = depths(blocks);
    blocks
        .iter()
        .enumerate()
        .map(|(i, b)| Row {
            no: (i + 1).to_string(),
            id: format!("{:02X}", b.id()),
            desc: describe::describe_block(b, hex),
            kind: kinds[i].clone(),
            len: describe::fmt(describe::block_length(b), hex, 0),
            indent: depth[i] as f32 * 12.0,
            cat: category(b),
        })
        .collect()
}

/// `(errors, warnings, programs)`, counted once when the tape is loaded.
pub type Counts = (usize, usize, usize);

pub fn status_line(blocks: &[Block], cursor: usize, counts: Counts) -> String {
    let Some(b) = blocks.get(cursor) else { return "No tape loaded".into() };
    let (errors, warnings, programs) = counts;
    format!(
        "Block {} of {} · {} · {programs} program(s) · {errors} error(s), {warnings} warning(s)",
        cursor + 1,
        blocks.len(),
        describe::describe_block(b, false)
    )
}

/// The skeleton runs the three commands that only read the tape; the rest report
/// their id, which is what the status bar is for until stage 4 fills them in.
pub fn run_command(item: &menutable::Item, blocks: &[Block], cursor: usize, checked: bool) -> String {
    match item.id {
        "consistency" => {
            let issues = consistency::check_consistency(blocks, 1);
            let errors = issues.iter().filter(|i| i.severity == consistency::Severity::Error).count();
            match issues.first() {
                None => "Consistency: nothing to report".into(),
                Some(first) => format!(
                    "Consistency: {} issue(s), {errors} error(s) — first: {}",
                    issues.len(),
                    first.message
                ),
            }
        }
        "programs" => {
            let progs = programs::detect_programs(blocks);
            let names: Vec<String> = progs.iter().take(4).map(|p| p.name.clone()).collect();
            format!("Programs: {} — {}", progs.len(), names.join(", "))
        }
        "tape-info" => {
            let (secs, _) = audio::tape_duration(blocks);
            let bytes: usize = blocks.iter().filter_map(|b| b.body.data()).map(|d| d.len()).sum();
            let v = writer::required_version(blocks);
            format!(
                "Tape info: {} blocks, {bytes} bytes of data, TZX {}.{:02}, {:.0}:{:02.0} playing",
                blocks.len(),
                v.major,
                v.minor,
                (secs / 60.0).floor(),
                secs % 60.0
            )
        }
        _ if item.check => {
            format!("{} ({}) {}", item.label, item.id, if checked { "on" } else { "off" })
        }
        _ => {
            let at = blocks.get(cursor).map(|b| describe::describe_block(b, false)).unwrap_or_default();
            format!("{} ({}) — not implemented yet. Cursor: {at}", item.label, item.id)
        }
    }
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

fn main() -> eframe::Result<()> {
    let t0 = Instant::now();
    let opts = parse_args();

    let t_parse = Instant::now();
    let tape = load(&opts);
    let parse_ms = t_parse.elapsed().as_secs_f64() * 1000.0;

    let blocks = match opts.rows {
        Some(n) if n > tape.blocks.len() => grow(&tape.blocks, n),
        Some(n) => tape.blocks[..n.min(tape.blocks.len())].to_vec(),
        None => tape.blocks.clone(),
    };

    if opts.measure {
        measure(&blocks, parse_ms, opts.hex);
        return Ok(());
    }

    let t_rows = Instant::now();
    let rows = build_rows(&blocks, opts.hex);
    let rows_ms = t_rows.elapsed().as_secs_f64() * 1000.0;

    let t_extra = Instant::now();
    let issues = consistency::check_consistency(&blocks, 1);
    let progs = programs::detect_programs(&blocks);
    let version = writer::required_version(&blocks);
    let extra_ms = t_extra.elapsed().as_secs_f64() * 1000.0;

    println!(
        "core: parse {parse_ms:.1} ms, {} rows described in {rows_ms:.1} ms, \
         consistency + programs + version {extra_ms:.1} ms",
        rows.len()
    );
    if tape.major > 0 {
        println!("{}: TZX {}.{:02} as loaded", tape.name, tape.major, tape.minor);
    }
    for w in &tape.warnings {
        println!("warning: {w}");
    }

    let counts: Counts = (
        issues.iter().filter(|i| i.severity == consistency::Severity::Error).count(),
        issues.iter().filter(|i| i.severity == consistency::Severity::Warning).count(),
        progs.len(),
    );
    let info = format!("{} blocks · TZX {}.{:02}", blocks.len(), version.major, version.minor);
    let name = tape.name.clone();
    let bench = opts.bench.unwrap_or(0);
    let exit_on_draw = opts.exit_on_draw;

    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("SpecTape (native)")
            .with_inner_size([980.0, 620.0])
            .with_min_inner_size([560.0, 300.0]),
        ..Default::default()
    };
    eframe::run_native(
        "SpecTape",
        native_options,
        Box::new(move |cc| {
            Ok(Box::new(app::App::new(cc, blocks, rows, counts, name, info, t0, bench, exit_on_draw)))
        }),
    )
}
