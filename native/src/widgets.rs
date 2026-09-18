//! The small widgets the forms are made of: the port of `src/ui/fields.tsx`.
//!
//! `NumInput` on the web keeps the typed text in component state so a half-typed
//! number is not reformatted under the cursor, and marks it invalid until it
//! parses. egui has no component state, so the text lives in the context's
//! temporary memory under the widget's id and is refreshed from the value
//! whenever the field is not being edited — which comes to the same thing.

use std::fmt::Debug;
use std::hash::Hash;

use egui::{Color32, Response, RichText, TextEdit, Ui};

use crate::fmt;
use crate::theme::Tokens;

/// One labelled row of a form grid.
pub fn field<R>(ui: &mut Ui, label: &str, tok: &Tokens, add: impl FnOnce(&mut Ui) -> R) -> R {
    ui.horizontal(|ui| {
        ui.label(RichText::new(label).size(12.0).color(tok.muted));
        add(ui)
    })
    .inner
}

pub fn note(ui: &mut Ui, tok: &Tokens, text: impl Into<String>) {
    ui.label(RichText::new(text.into()).size(11.0).color(tok.muted));
}

/// A number field honouring the Dec/Hex switch. Returns true when `value`
/// changed this frame.
#[allow(clippy::too_many_arguments)]
pub fn num(
    ui: &mut Ui,
    id: impl Hash + Debug,
    value: &mut i64,
    min: i64,
    max: i64,
    hex: bool,
    width: f32,
    enabled: bool,
) -> bool {
    let id = ui.make_persistent_id(id);
    // While the field has focus the typed text is what the user sees, however
    // half-finished; the rest of the time it is the value, formatted in the
    // current base.
    let editing = ui.memory(|m| m.has_focus(id));
    let mut text = match editing {
        true => ui.data_mut(|d| d.get_temp::<String>(id)).unwrap_or_else(|| fmt::num(*value, hex)),
        false => fmt::num(*value, hex),
    };
    let parsed = fmt::parse_num(&text, hex);
    let bad = !parsed.is_some_and(|v| v >= min && v <= max);
    let mut edit = TextEdit::singleline(&mut text).id(id).desired_width(width);
    if bad {
        edit = edit.text_color(Color32::from_rgb(0xd2, 0x3f, 0x3f));
    }
    ui.add_enabled(enabled, edit);
    ui.data_mut(|d| d.insert_temp(id, text.clone()));
    match fmt::parse_num(&text, hex) {
        Some(v) if v >= min && v <= max && v != *value => {
            *value = v;
            true
        }
        _ => false,
    }
}

/// The same, for the many fields whose value is a `u16`.
pub fn num_u16(ui: &mut Ui, id: impl Hash + Debug, value: &mut u16, hex: bool, enabled: bool) -> bool {
    let mut v = i64::from(*value);
    let changed = num(ui, id, &mut v, 0, 0xffff, hex, 70.0, enabled);
    if changed {
        *value = v as u16;
    }
    changed
}

pub fn num_u32(
    ui: &mut Ui,
    id: impl Hash + Debug,
    value: &mut u32,
    max: i64,
    hex: bool,
    enabled: bool,
) -> bool {
    let mut v = i64::from(*value);
    let changed = num(ui, id, &mut v, 0, max, hex, 90.0, enabled);
    if changed {
        *value = v as u32;
    }
    changed
}

pub fn num_u8(
    ui: &mut Ui,
    id: impl Hash + Debug,
    value: &mut u8,
    min: i64,
    max: i64,
    hex: bool,
    enabled: bool,
) -> bool {
    let mut v = i64::from(*value);
    let changed = num(ui, id, &mut v, min, max, hex, 60.0, enabled);
    if changed {
        *value = v as u8;
    }
    changed
}

pub fn num_i16(ui: &mut Ui, id: impl Hash + Debug, value: &mut i16, hex: bool, enabled: bool) -> bool {
    let mut v = i64::from(*value);
    let changed = num(ui, id, &mut v, -0x8000, 0x7fff, hex, 70.0, enabled);
    if changed {
        *value = v as i16;
    }
    changed
}

/// A single-line text field with a maximum length in characters.
pub fn text(ui: &mut Ui, value: &mut String, max_len: usize, width: f32, enabled: bool) -> Response {
    let r = ui.add_enabled(enabled, TextEdit::singleline(value).desired_width(width));
    if value.chars().count() > max_len {
        *value = value.chars().take(max_len).collect();
    }
    r
}

/// A multi-line field, the `<textarea class="wide">` of the forms.
pub fn multiline(ui: &mut Ui, value: &mut String, rows: usize, enabled: bool) -> Response {
    ui.add_enabled(
        enabled,
        TextEdit::multiline(value)
            .desired_rows(rows)
            .desired_width(f32::INFINITY)
            .font(egui::TextStyle::Monospace),
    )
}

pub fn check(ui: &mut Ui, label: &str, value: &mut bool, enabled: bool) -> bool {
    ui.add_enabled(enabled, egui::Checkbox::new(value, label)).changed()
}

/// A dropdown over `options`, selecting by index.
pub fn combo<T: PartialEq + Copy>(
    ui: &mut Ui,
    id: impl Hash + Debug,
    value: &mut T,
    options: &[(T, String)],
    width: f32,
    enabled: bool,
) -> bool {
    let current = options.iter().find(|(v, _)| v == value).map(|(_, s)| s.clone()).unwrap_or_default();
    let mut changed = false;
    ui.add_enabled_ui(enabled, |ui| {
        egui::ComboBox::from_id_salt(id).selected_text(current).width(width).show_ui(ui, |ui| {
            for (v, label) in options {
                if ui.selectable_label(*v == *value, label).clicked() {
                    *value = *v;
                    changed = true;
                }
            }
        });
    });
    changed
}

/// A small chip, the `.chip` of the editor's info column.
pub fn chip(ui: &mut Ui, text: &str, colour: Color32) {
    let galley = ui.painter().layout_no_wrap(text.to_string(), egui::FontId::proportional(11.0), colour);
    let (rect, _) = ui.allocate_exact_size(galley.size() + egui::vec2(10.0, 4.0), egui::Sense::hover());
    ui.painter().rect_filled(rect, egui::CornerRadius::same(4), colour.gamma_multiply(0.18));
    ui.painter().galley(rect.center() - galley.size() / 2.0, galley, colour);
}
