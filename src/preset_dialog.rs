//! Confirmation before an imported Lightroom preset is deleted.

use eframe::egui::{self, Color32, Vec2};

use crate::app::StudioApp;
use crate::theme;

/// Shows "Delete preset?" while a deletion waits for an answer.
pub fn confirm_delete(app: &mut StudioApp, ctx: &egui::Context) {
    let Some(preset) = app.preset_to_delete.clone() else { return };
    let mut answer: Option<bool> = None;
    let modal = egui::Modal::new(egui::Id::unique("delete_preset")).show(ctx, |ui| {
        ui.set_width(380.0);
        ui.label(egui::RichText::new("Delete preset?").size(16.0).strong());
        ui.label(
            egui::RichText::new(format!(
                "\u{201c}{}\u{201d} will be removed from Animagraph. The file you imported it from is not touched, \
                 so you can import it again later.",
                preset.name
            ))
            .color(theme::TEXT_DIM),
        );
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            let delete = egui::Button::new(egui::RichText::new("Delete").color(Color32::WHITE).strong())
                .fill(theme::DANGER)
                .min_size(Vec2::new(90.0, 30.0));
            if ui.add(delete).clicked() {
                answer = Some(true);
            }
            if ui.add(egui::Button::new("Cancel").min_size(Vec2::new(90.0, 30.0))).clicked() {
                answer = Some(false);
            }
        });
    });
    ctx.move_to_top(modal.backdrop_response.layer_id);
    ctx.move_to_top(modal.response.layer_id);
    if modal.should_close() {
        answer = answer.or(Some(false));
    }
    match answer {
        Some(true) => {
            app.status = match crate::presets::remove(&preset) {
                Ok(()) => format!("Deleted the {} preset.", preset.name),
                Err(e) => e,
            };
            app.lr_presets = crate::presets::list();
            app.lr_choice = None;
            app.preset_to_delete = None;
        }
        Some(false) => app.preset_to_delete = None,
        None => {}
    }
}
