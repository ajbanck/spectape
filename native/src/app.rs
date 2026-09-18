//! The window: tape header, block list, status bar — and the measuring harness.
//!
//! egui is immediate mode, so there is no second language and no binding graph:
//! the frame reads this struct and draws it. The rows are built once from the
//! core, because `describe_block` on 3,000 blocks is 0.9 ms and a frame is 16.

use std::time::Instant;

use egui::{pos2, vec2, Align2, Color32, CornerRadius, FontId, Rect, Sense};

use spectape_core::types::Block;

use crate::menu::Menu;
use crate::menutable;
use crate::{run_command, status_line, Counts};

/// The light half of src/style.css, the tokens the web list uses.
pub mod tok {
    use egui::Color32;
    pub const BG: Color32 = Color32::from_rgb(0xee, 0xf0, 0xf4);
    pub const SURFACE: Color32 = Color32::from_rgb(0xff, 0xff, 0xff);
    pub const SURFACE_2: Color32 = Color32::from_rgb(0xf6, 0xf7, 0xf9);
    pub const BORDER: Color32 = Color32::from_rgb(0xdf, 0xe3, 0xea);
    pub const TEXT: Color32 = Color32::from_rgb(0x17, 0x1b, 0x26);
    pub const MUTED: Color32 = Color32::from_rgb(0x6b, 0x72, 0x80);
    pub const FAINT: Color32 = Color32::from_rgb(0x9a, 0xa3, 0xb2);
    pub const ACCENT: Color32 = Color32::from_rgb(0x2f, 0x6f, 0xed);
    pub const ACCENT_SOFT: Color32 = Color32::from_rgb(0xe6, 0xee, 0xfc);
    /// `category()` in src/ui/TapePane.tsx.
    pub const CAT: [Color32; 6] = [
        Color32::from_rgb(0x2f, 0x6f, 0xed), // data
        Color32::from_rgb(0x0e, 0x9f, 0x9f), // signal
        Color32::from_rgb(0xd9, 0x77, 0x06), // flow
        Color32::from_rgb(0x7c, 0x3a, 0xed), // struct
        Color32::from_rgb(0x64, 0x74, 0x8b), // info
        Color32::from_rgb(0xb9, 0x1c, 0x1c), // unknown
    ];
}

const ROW_H: f32 = 20.0;

pub struct Row {
    pub no: String,
    pub id: String,
    pub desc: String,
    pub kind: String,
    pub len: String,
    pub indent: f32,
    pub cat: usize,
}

/// What `--bench` collects, and what the status bar shows while the window is open.
#[derive(Default)]
struct Perf {
    update: Vec<f64>,
    frame: Vec<f64>,
    last_update: f64,
}

fn stats(v: &[f64]) -> (f64, f64, f64) {
    let mut s = v.to_vec();
    s.sort_by(|a, b| a.partial_cmp(b).unwrap());
    (s[0], s[s.len() / 2], s[s.len() - 1])
}

pub struct App {
    blocks: Vec<Block>,
    rows: Vec<Row>,
    counts: Counts,
    cursor: usize,
    status: String,
    tape_name: String,
    tape_info: String,
    checked: Vec<bool>,
    menu: Menu,

    start: Instant,
    first_frame: bool,
    exit_on_draw: bool,
    bench_left: usize,
    benching: bool,
    perf: Perf,

    scroll_to_cursor: bool,
    scroll_offset: f32,
    view_h: f32,
}

impl App {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        cc: &eframe::CreationContext<'_>,
        blocks: Vec<Block>,
        rows: Vec<Row>,
        counts: Counts,
        tape_name: String,
        tape_info: String,
        start: Instant,
        bench: usize,
        exit_on_draw: bool,
    ) -> Self {
        style(&cc.egui_ctx);
        let menu = Menu::new(&cc.egui_ctx);
        menu.set_enabled(&menutable::enabled_flags(blocks.len(), !blocks.is_empty()));
        let status = status_line(&blocks, 0, counts);
        App {
            blocks,
            rows,
            counts,
            cursor: 0,
            status,
            tape_name,
            tape_info,
            checked: vec![false; menutable::item_count()],
            menu,
            start,
            first_frame: true,
            exit_on_draw,
            bench_left: bench,
            benching: bench > 0,
            perf: Perf::default(),
            scroll_to_cursor: false,
            scroll_offset: 0.0,
            view_h: 400.0,
        }
    }

    fn move_cursor(&mut self, delta: i64) {
        let last = self.rows.len().saturating_sub(1) as i64;
        self.set_cursor((self.cursor as i64 + delta).clamp(0, last.max(0)) as usize);
    }

    fn set_cursor(&mut self, index: usize) {
        self.cursor = index.min(self.rows.len().saturating_sub(1));
        // Rebuilt from the core on every move: the per-row work the wasm boundary
        // used to charge for.
        self.status = status_line(&self.blocks, self.cursor, self.counts);
        self.scroll_to_cursor = true;
    }

    fn handle_menu(&mut self) {
        for id in self.menu.take_activated() {
            let items = menutable::flat();
            let Some((flat, item)) = items.iter().enumerate().find(|(_, it)| it.id == id) else {
                continue;
            };
            if item.check {
                self.checked[flat] = !self.checked[flat];
                self.menu.set_checked(flat, self.checked[flat]);
            }
            self.status = run_command(item, &self.blocks, self.cursor, self.checked[flat]);
        }
    }

    fn handle_keys(&mut self, ctx: &egui::Context) {
        use egui::Key;
        let page = ((self.view_h / ROW_H).floor() as i64 - 1).max(1);
        let (mut delta, mut to) = (0i64, None);
        ctx.input(|i| {
            if i.key_pressed(Key::ArrowDown) {
                delta += 1;
            }
            if i.key_pressed(Key::ArrowUp) {
                delta -= 1;
            }
            if i.key_pressed(Key::PageDown) {
                delta += page;
            }
            if i.key_pressed(Key::PageUp) {
                delta -= page;
            }
            if i.key_pressed(Key::Home) {
                to = Some(0);
            }
            if i.key_pressed(Key::End) {
                to = Some(self.rows.len().saturating_sub(1));
            }
        });
        if let Some(index) = to {
            self.set_cursor(index);
        } else if delta != 0 {
            self.move_cursor(delta);
        }
    }

    /// `--bench`: one cursor move per frame, so every sample is a real frame.
    fn bench_step(&mut self, ctx: &egui::Context) {
        if !self.benching {
            return;
        }
        if self.bench_left == 0 {
            if !self.perf.update.is_empty() {
                let (lo, mid, hi) = stats(&self.perf.update);
                let (flo, fmid, fhi) = stats(&self.perf.frame);
                println!(
                    "cursor move over {} frames on {} rows:\n  \
                     our own frame build  min {lo:.2} ms, median {mid:.2} ms, max {hi:.2} ms\n  \
                     whole frame          min {flo:.2} ms, median {fmid:.2} ms, max {fhi:.2} ms",
                    self.perf.update.len(),
                    self.rows.len()
                );
            }
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            return;
        }
        self.bench_left -= 1;
        let down = self.perf.update.len() % 40 < 20;
        self.move_cursor(if down { 1 } else { -1 });
        ctx.request_repaint();
    }

    fn list(&mut self, ui: &mut egui::Ui) {
        let mut area = egui::ScrollArea::vertical().auto_shrink([false, false]);
        if self.scroll_to_cursor {
            self.scroll_to_cursor = false;
            let spacing = ui.spacing().item_spacing.y;
            let top = self.cursor as f32 * (ROW_H + spacing);
            let offset = if top < self.scroll_offset {
                top
            } else if top + ROW_H > self.scroll_offset + self.view_h {
                top + ROW_H - self.view_h
            } else {
                self.scroll_offset
            };
            area = area.vertical_scroll_offset(offset);
        }

        let (rows, cursor) = (&self.rows, self.cursor);
        let mut clicked = None;
        let out = area.show_rows(ui, ROW_H, rows.len(), |ui, range| {
            for i in range {
                let (rect, response) =
                    ui.allocate_exact_size(vec2(ui.available_width(), ROW_H), Sense::click());
                if response.clicked() {
                    clicked = Some(i);
                }
                if ui.is_rect_visible(rect) {
                    draw_row(ui, rect, &rows[i], i, cursor);
                }
            }
        });
        self.scroll_offset = out.state.offset.y;
        self.view_h = out.inner_rect.height();
        if let Some(i) = clicked {
            self.set_cursor(i);
        }
    }
}

fn draw_row(ui: &egui::Ui, rect: Rect, row: &Row, index: usize, cursor: usize) {
    let p = ui.painter();
    if index == cursor {
        p.rect_filled(rect, CornerRadius::ZERO, tok::ACCENT_SOFT);
        let bar = Rect::from_min_size(rect.left_top(), vec2(3.0, rect.height()));
        p.rect_filled(bar, CornerRadius::ZERO, tok::ACCENT);
    } else if index % 2 == 1 {
        p.rect_filled(rect, CornerRadius::ZERO, tok::SURFACE_2);
    }

    let y = rect.center().y;
    let mono = FontId::monospace(11.0);
    p.text(pos2(rect.left() + 52.0, y), Align2::RIGHT_CENTER, &row.no, mono.clone(), tok::FAINT);

    let badge = Rect::from_min_size(pos2(rect.left() + 60.0, y - 7.5), vec2(26.0, 15.0));
    p.rect_filled(badge, CornerRadius::same(4), tok::CAT[row.cat]);
    p.text(badge.center(), Align2::CENTER_CENTER, &row.id, FontId::monospace(10.0), Color32::WHITE);

    // The description gets what is left between the badge and the two right columns.
    let left = rect.left() + 94.0 + row.indent;
    let right = rect.right() - 170.0;
    if right > left {
        let clip = Rect::from_min_max(pos2(left, rect.top()), pos2(right, rect.bottom()));
        p.with_clip_rect(clip).text(
            pos2(left, y),
            Align2::LEFT_CENTER,
            &row.desc,
            FontId::proportional(12.0),
            tok::TEXT,
        );
    }
    p.text(
        pos2(rect.right() - 86.0, y),
        Align2::RIGHT_CENTER,
        &row.kind,
        FontId::proportional(11.0),
        tok::MUTED,
    );
    p.text(pos2(rect.right() - 8.0, y), Align2::RIGHT_CENTER, &row.len, mono, tok::MUTED);
}

fn style(ctx: &egui::Context) {
    let mut visuals = egui::Visuals::light();
    visuals.panel_fill = tok::BG;
    visuals.window_fill = tok::SURFACE;
    visuals.extreme_bg_color = tok::SURFACE;
    visuals.override_text_color = Some(tok::TEXT);
    visuals.widgets.noninteractive.bg_stroke = egui::Stroke::new(1.0, tok::BORDER);
    visuals.selection.bg_fill = tok::ACCENT_SOFT;
    visuals.selection.stroke = egui::Stroke::new(1.0, tok::ACCENT);
    ctx.set_visuals(visuals);
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let t = Instant::now();
        let ctx = ui.ctx().clone();
        let ctx = &ctx;
        self.handle_menu();
        self.handle_keys(ctx);
        self.bench_step(ctx);

        // On macOS the menu is the platform's own, set by muda; elsewhere it is
        // drawn here from the same table.
        #[cfg(not(target_os = "macos"))]
        egui::Panel::top("menubar").show(ui, |ui| self.menu.bar(ui));

        egui::Panel::top("header").show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.add_space(2.0);
                ui.label(egui::RichText::new(&self.tape_name).size(13.0).strong());
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(egui::RichText::new(&self.tape_info).monospace().size(11.0).color(tok::MUTED));
                });
            });
        });

        egui::Panel::bottom("status").show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(&self.status).size(11.0).color(tok::MUTED));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let text = if self.perf.update.is_empty() {
                        String::new()
                    } else {
                        let (_, mid, hi) = stats(&self.perf.update);
                        format!(
                            "frame {:.2} ms · {} moves, median {mid:.2} / max {hi:.2}",
                            self.perf.last_update,
                            self.perf.update.len()
                        )
                    };
                    ui.label(egui::RichText::new(text).monospace().size(10.0).color(tok::FAINT));
                });
            });
        });

        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(tok::SURFACE).inner_margin(egui::Margin::same(1)))
            .show(ui, |ui| self.list(ui));

        let ms = t.elapsed().as_secs_f64() * 1000.0;
        self.perf.last_update = ms;
        if !self.first_frame {
            // The first frame includes font atlas building and window setup; it is
            // the cold start number, not a cursor move.
            self.perf.update.push(ms);
            self.perf.frame.push(f64::from(ctx.input(|i| i.unstable_dt)) * 1000.0);
        }

        if self.first_frame {
            self.first_frame = false;
            println!(
                "first frame: {:.0} ms after main() started ({} rows)",
                self.start.elapsed().as_secs_f64() * 1000.0,
                self.rows.len()
            );
            if self.exit_on_draw {
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }
    }
}
