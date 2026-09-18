//! The status bar, the port of `src/ui/StatusBar.tsx`: the Dec/Hex switch, the
//! two compare modes, the lock, the waveform, and either the status message or
//! the playback progress.

use egui::{Align, Layout, RichText, Ui};

use spectape_core::compare::{BlockCompareMode, TapeCompareMode};

use crate::app::App;
use crate::fmt;

fn cell(ui: &mut Ui, text: String, on: bool, hover: &str, tok: &crate::theme::Tokens) -> bool {
    let colour = if on { tok.accent } else { tok.muted };
    let r =
        ui.add(egui::Label::new(RichText::new(text).size(11.0).color(colour)).sense(egui::Sense::click()));
    r.on_hover_text(hover).clicked()
}

pub fn show(app: &mut App, ui: &mut Ui) {
    let tok = app.tokens;
    ui.horizontal(|ui| {
        let hex = app.store.hex;
        if cell(
            ui,
            format!("# {}", if hex { "Hex" } else { "Dec" }),
            hex,
            "Number base for all numbers",
            &tok,
        ) {
            app.store.hex = !hex;
            app.store.touch_view();
        }
        ui.separator();

        let bc = app.store.block_compare;
        let bc_label = match bc {
            BlockCompareMode::Data => "data",
            BlockCompareMode::DataTimings => "data + timings",
            BlockCompareMode::DataTimingsPauses => "data + timings + pauses",
        };
        if cell(ui, format!("Block compare {bc_label}"), false, "How two blocks are compared", &tok) {
            app.store.block_compare = match bc {
                BlockCompareMode::Data => BlockCompareMode::DataTimings,
                BlockCompareMode::DataTimings => BlockCompareMode::DataTimingsPauses,
                BlockCompareMode::DataTimingsPauses => BlockCompareMode::Data,
            };
        }
        ui.separator();

        let tc = app.store.tape_compare;
        let tc_label = match tc {
            TapeCompareMode::DataBlocks => "data blocks only",
            TapeCompareMode::IgnoreMetadata => "ignore metadata",
            TapeCompareMode::All => "all blocks",
        };
        if cell(ui, format!("Tape compare {tc_label}"), false, "Which blocks take part in tape compare", &tok)
        {
            app.store.tape_compare = match tc {
                TapeCompareMode::DataBlocks => TapeCompareMode::IgnoreMetadata,
                TapeCompareMode::IgnoreMetadata => TapeCompareMode::All,
                TapeCompareMode::All => TapeCompareMode::DataBlocks,
            };
        }
        ui.separator();

        let locked = app.store.locked;
        let hover =
            if locked { "Locked: click to allow editing" } else { "Unlocked: click to prevent edits" };
        if cell(ui, if locked { "🔒 Locked".into() } else { "Unlocked".to_string() }, locked, hover, &tok) {
            app.store.toggle_lock();
        }
        ui.separator();

        let mic = app.store.audio_mic;
        let label = if mic { "MIC emulation" } else { "Square wave" };
        if cell(ui, format!("〜 {label}"), false, "Waveform used for playback and WAV export", &tok) {
            app.store.audio_mic = !mic;
        }
        ui.separator();

        let theme = app.store.settings.theme;
        let mark = match theme {
            crate::settings::Theme::Light => "☀",
            crate::settings::Theme::Dark => "☾",
            crate::settings::Theme::System => "◐",
        };
        if cell(ui, format!("{mark} {}", theme.name()), false, "Theme (click to change)", &tok) {
            app.store.settings.theme = theme.next();
            app.store.settings.save();
        }
        ui.separator();

        if app.progress.playing {
            progress(app, ui);
        } else {
            let status = app.store.status().to_string();
            ui.label(RichText::new(status).size(11.0).color(tok.muted));
        }
    });
}

fn progress(app: &mut App, ui: &mut Ui) {
    let tok = app.tokens;
    let p = app.progress;
    let zero = app.store.settings.zero_based;
    ui.label(RichText::new("▶").size(11.0).color(tok.ok));
    ui.label(
        RichText::new(format!("{} / {}", fmt::time(p.elapsed), fmt::time(p.total))).size(11.0).monospace(),
    );
    let (rect, response) = ui.allocate_exact_size(egui::vec2(160.0, 8.0), egui::Sense::click());
    let frac = if p.total > 0.0 { (p.elapsed / p.total) as f32 } else { 0.0 };
    ui.painter().rect_filled(rect, egui::CornerRadius::same(4), tok.surface_3);
    let filled = egui::Rect::from_min_size(rect.min, egui::vec2(rect.width() * frac, rect.height()));
    ui.painter().rect_filled(filled, egui::CornerRadius::same(4), tok.accent);
    if response.on_hover_text("Click to stop").clicked() {
        app.player.stop();
    }
    if p.block >= 0 {
        ui.label(
            RichText::new(format!("Block {}", fmt::block_no(p.block as usize, zero)))
                .size(11.0)
                .color(tok.muted),
        );
    }
    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
        if ui.small_button("Stop").clicked() {
            app.player.stop();
        }
    });
}
