//! The status bar, the port of `src/ui/StatusBar.tsx`: the Dec/Hex switch, the
//! two compare modes, the lock, the waveform, and either the status message or
//! the playback progress.

use egui::{Align, Layout, RichText, Ui};

use spectape_core::compare::{BlockCompareMode, TapeCompareMode};

use crate::app::App;
use crate::fmt;
use crate::icons::{self, Icon};

/// One clickable cell: the icon the web bar shows, then its text.
fn cell(
    ui: &mut Ui,
    icon: Option<&Icon>,
    text: String,
    on: bool,
    hover: &str,
    tok: &crate::theme::Tokens,
) -> bool {
    let colour = if on { tok.accent } else { tok.muted };
    let mut clicked = false;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        if let Some(icon) = icon {
            clicked |= icons::inline(ui, icon, colour, 13.0).on_hover_text(hover).clicked();
        }
        let label =
            egui::Label::new(RichText::new(text).size(11.0).color(colour)).sense(egui::Sense::click());
        clicked |= ui.add(label).on_hover_text(hover).clicked();
    });
    clicked
}

pub fn show(app: &mut App, ui: &mut Ui) {
    let tok = app.tokens;
    ui.horizontal(|ui| {
        let hex = app.store.hex;
        let base = if hex { "Hex" } else { "Dec" };
        if cell(ui, Some(&icons::HASH), base.into(), hex, "Number base for all numbers", &tok) {
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
        if cell(ui, None, format!("Block compare {bc_label}"), false, "How two blocks are compared", &tok) {
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
        let hover = "Which blocks take part in tape compare";
        if cell(ui, Some(&icons::COMPARE), format!("Tape compare {tc_label}"), false, hover, &tok) {
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
        let icon = if locked { &icons::LOCK } else { &icons::UNLOCK };
        let text = if locked { "Locked" } else { "Unlocked" };
        if cell(ui, Some(icon), text.into(), locked, hover, &tok) {
            app.store.toggle_lock();
        }
        ui.separator();

        let mic = app.store.audio_mic;
        let label = if mic { "MIC emulation" } else { "Square wave" };
        let hover = "Waveform used for playback and WAV export";
        if cell(ui, Some(&icons::WAVE), label.into(), false, hover, &tok) {
            app.store.audio_mic = !mic;
        }
        ui.separator();

        // The theme switch lives at the right end of the menu bar, where
        // `MenuBar.tsx` has always had it — not here as well.

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
    icons::inline(ui, &icons::PLAY, tok.ok, 11.0);
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
