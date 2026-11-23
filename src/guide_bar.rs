//! The composition guide buttons of the crop bar.

use eframe::egui::{self, Color32, CornerRadius, Stroke};

use crate::app::StudioApp;
use crate::composition::Guide;
use crate::theme;

const GAP: f32 = 4.0;
const TEXT: f32 = 12.0;

/// One button per guide, in as many rows as `width` needs. Clicking the
/// chosen guide again shows its next flip.
pub fn buttons(app: &mut StudioApp, ui: &mut egui::Ui, width: f32) {
    ui.spacing_mut().item_spacing.y = GAP;
    let padding = ui.spacing().button_padding;
    ui.spacing_mut().button_padding = egui::vec2(8.0, 4.0);
    for row in rows(ui, width) {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = GAP;
            for g in row {
                button(app, ui, g);
            }
        });
    }
    ui.spacing_mut().button_padding = padding;
}

fn button(app: &mut StudioApp, ui: &mut egui::Ui, g: Guide) {
    let on = app.crop_guide == g;
    let text = egui::RichText::new(g.label()).size(TEXT).color(if on { Color32::WHITE } else { theme::TEXT_DIM });
    let b = egui::Button::new(text)
        .fill(if on { theme::ACCENT } else { Color32::TRANSPARENT })
        .stroke(Stroke::NONE)
        .corner_radius(CornerRadius::same(8));
    if ui.add(b).on_hover_text(g.hint()).clicked() {
        app.crop_guide_flip = if on { (app.crop_guide_flip + 1) % g.flips() } else { 0 };
        app.crop_guide = g;
    }
}

/// The guides split over rows, filled one after the other.
fn rows(ui: &egui::Ui, width: f32) -> Vec<Vec<Guide>> {
    let pad = ui.spacing().button_padding.x * 2.0;
    let mut rows: Vec<Vec<Guide>> = vec![Vec::new()];
    let mut used = 0.0;
    for g in Guide::ALL {
        let font = egui::FontId::proportional(TEXT);
        let w = ui.painter().layout_no_wrap(g.label().into(), font, Color32::WHITE).size().x + pad + GAP;
        if used + w > width && used > 0.0 {
            rows.push(Vec::new());
            used = 0.0;
        }
        used += w;
        rows.last_mut().expect("there is a row").push(g);
    }
    rows
}
