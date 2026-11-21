//! The Masks card in the Edit drawer: add masks (brush, linear, radial,
//! sky), pick one to edit on the canvas, narrow it to a brightness or colour
//! range, and set its own adjustments.

use eframe::egui::{self, Color32, Vec2};

use crate::app::{PickTarget, StudioApp};
use crate::icons::{self, Icon};
use crate::mask::{Mask, RangeKind, Shape};
use crate::panels::{eyedropper_button, fold_title, is_open, pct, reset_icon};
use crate::theme;

pub fn masks_card(app: &mut StudioApp, ui: &mut egui::Ui) {
    let has_file = app.scene.is_some();
    // A colour picked on the canvas for the colour range.
    if let Some(c) = app.mask_pick.take() {
        if let Some(m) = selected(app) {
            m.range.hue = crate::mask::hue_sat(c[0] as f32 / 255.0, c[1] as f32 / 255.0, c[2] as f32 / 255.0).0;
            m.range.kind = RangeKind::Color;
        }
    }
    theme::card().show(ui, |ui| {
        ui.set_width(ui.available_width());
        let count = app.scene.as_ref().map_or(0, |s| s.edits.masks.len());
        ui.horizontal(|ui| {
            fold_title(ui, app, "masks", "Masks");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if count > 0 {
                    ui.label(egui::RichText::new(format!("{count}")).size(11.0).color(theme::TEXT_DIM));
                }
            });
        });
        if !is_open(app, "masks") {
            return;
        }
        if !has_file {
            ui.disable();
        }
        ui.label(
            egui::RichText::new("Change only part of the photo. Add a mask, shape it on the picture, then set its adjustments.")
                .size(12.0)
                .color(theme::TEXT_DIM),
        );
        // Add.
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = Vec2::new(6.0, 6.0);
            let add = |ui: &mut egui::Ui, label: &str, tip: &str| {
                ui.add(icons::button(Icon::Plus, egui::RichText::new(label).size(12.5), theme::TEXT)).on_hover_text(tip).clicked()
            };
            let mut new: Option<Mask> = None;
            let n = count + 1;
            if add(ui, "Brush", "Paint where the change goes") {
                let (w, h) = app.scene.as_ref().map_or((1, 1), |s| (s.full.width(), s.full.height()));
                new = Some(Mask::brush(format!("Brush {n}"), w, h));
            }
            if add(ui, "Linear", "A gradient: full effect on one side, fading out (skies, foregrounds)") {
                new = Some(Mask::linear(format!("Linear {n}")));
            }
            if add(ui, "Radial", "An oval: full effect inside, fading out at its edge (a subject, a light)") {
                new = Some(Mask::radial(format!("Radial {n}")));
            }
            if add(ui, "Sky", "The sky, estimated from the picture; touch it up with the brush") {
                if let Some(s) = &app.scene {
                    new = Some(Mask::sky(format!("Sky {n}"), &s.full));
                }
            }
            if let (Some(m), Some(s)) = (new, app.scene.as_mut()) {
                s.edits.masks.push(m);
                app.mask_edit = Some(s.edits.masks.len() - 1);
                app.mask_overlay = true;
                app.view = crate::app::View::Edit;
                app.pick_tool(crate::app::Tool::Select);
                app.status = "New mask: shape it on the picture, then set its adjustments below.".into();
            }
        });

        // List.
        let mut remove = None;
        let mut select = None;
        if let Some(s) = app.scene.as_mut() {
            for (i, m) in s.edits.masks.iter_mut().enumerate() {
                ui.horizontal(|ui| {
                    ui.checkbox(&mut m.visible, "").on_hover_text("Show or hide this mask's changes");
                    let sel = app.mask_edit == Some(i);
                    let label = format!("{}  ·  {}", m.name, m.kind_name());
                    if ui.selectable_label(sel, label).on_hover_text("Edit this mask").clicked() {
                        select = Some(if sel { None } else { Some(i) });
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let bin = egui::Button::image(Icon::Trash.image(13.0, theme::TEXT_DIM)).fill(Color32::TRANSPARENT);
                        if ui.add(bin).on_hover_text("Delete this mask (undo brings it back)").clicked() {
                            remove = Some(i);
                        }
                    });
                });
            }
            if let Some(i) = remove {
                s.edits.masks.remove(i);
                app.mask_edit = None;
            }
        }
        if let Some(sel) = select {
            app.mask_edit = sel;
            if sel.is_some() {
                app.view = crate::app::View::Edit;
                app.pick_tool(crate::app::Tool::Select);
            }
        }

        // The selected mask.
        if app.mask_edit.is_some() {
            ui.separator();
            selected_mask(app, ui);
        }
    });
}

fn selected(app: &mut StudioApp) -> Option<&mut Mask> {
    let i = app.mask_edit?;
    app.scene.as_mut()?.edits.masks.get_mut(i)
}

fn selected_mask(app: &mut StudioApp, ui: &mut egui::Ui) {
    let (mut erase, mut overlay) = (app.mask_erase, app.mask_overlay);
    let picking = app.picking == Some(PickTarget::MaskColor);
    let (mut brush_size, mut hardness) = (app.brush_radius, app.brush_hardness);
    let mut pick_clicked = false;
    let mut done = false;
    let Some(m) = selected(app) else { return };
    ui.horizontal(|ui| {
        ui.label("Name");
        ui.add(egui::TextEdit::singleline(&mut m.name).desired_width(150.0));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.add(egui::Button::new(egui::RichText::new("Done").color(Color32::WHITE)).fill(theme::ACCENT)).clicked() {
                done = true;
            }
        });
    });
    match &mut m.shape {
        Shape::Brush { .. } => {
            ui.horizontal(|ui| {
                ui.selectable_value(&mut erase, false, "Paint");
                ui.selectable_value(&mut erase, true, "Erase").on_hover_text("Or hold Alt/Option while painting");
                if ui.small_button("Fill").clicked() {
                    m.fill(1.0);
                }
                if ui.small_button("Clear").clicked() {
                    m.fill(0.0);
                }
            });
            ui.add(egui::Slider::new(&mut brush_size, 3.0..=200.0).text("Brush size").integer());
            ui.add(egui::Slider::new(&mut hardness, 0.0..=1.0).text("Hardness"));
        }
        Shape::Linear { .. } => {
            ui.label(egui::RichText::new("Drag on the picture to draw the gradient: full effect where you start, none where you let go. Drag the dots to adjust.").size(11.5).color(theme::TEXT_DIM));
        }
        Shape::Radial { feather, .. } => {
            ui.add(pct(feather, 0.01, 1.0, "Feather", 100.0)).on_hover_text("How much of the oval fades out");
            ui.label(egui::RichText::new("Drag on the picture to draw the oval; drag the dots to move or resize it.").size(11.5).color(theme::TEXT_DIM));
        }
    }
    ui.horizontal(|ui| {
        ui.checkbox(&mut m.invert, "Invert").on_hover_text("Change everything outside the mask instead");
        ui.checkbox(&mut overlay, "Show overlay").on_hover_text("Red tint where the mask applies");
    });

    // Range.
    ui.horizontal(|ui| {
        ui.label("Range");
        for (k, name) in [(RangeKind::Off, "All"), (RangeKind::Luminance, "Brightness"), (RangeKind::Color, "Colour")] {
            ui.selectable_value(&mut m.range.kind, k, name);
        }
    });
    match m.range.kind {
        RangeKind::Off => {}
        RangeKind::Luminance => {
            ui.add(pct(&mut m.range.lum[0], 0.0, 1.0, "From", 100.0)).on_hover_text("Darkest tone included (0 black)");
            ui.add(pct(&mut m.range.lum[1], 0.0, 1.0, "To", 100.0)).on_hover_text("Brightest tone included (100 white)");
            ui.add(pct(&mut m.range.smooth, 0.0, 1.0, "Smoothness", 100.0));
        }
        RangeKind::Color => {
            ui.horizontal(|ui| {
                ui.add(egui::Slider::new(&mut m.range.hue, 0.0..=360.0).text("Hue").suffix("°").integer());
                if eyedropper_button(ui, picking).on_hover_text("Pick the colour from the picture").clicked() {
                    pick_clicked = true;
                }
            });
            ui.add(egui::Slider::new(&mut m.range.hue_width, 10.0..=180.0).text("Width").suffix("°").integer());
            ui.add(pct(&mut m.range.smooth, 0.0, 1.0, "Smoothness", 100.0));
        }
    }

    // Adjustments, in the same units as the Edit sliders.
    ui.label(egui::RichText::new("Adjustments").strong());
    let l = &mut m.local;
    ui.horizontal(|ui| {
        let was_zero = l.exposure == 0.0;
        ui.add(egui::Slider::new(&mut l.exposure, -2.0..=2.0).text("Exposure").fixed_decimals(2));
        if reset_icon(ui, was_zero) {
            l.exposure = 0.0;
        }
    });
    macro_rules! adj {
        ($field:ident, $lo:expr, $hi:expr, $name:expr, $scale:expr) => {{
            ui.horizontal(|ui| {
                let was_zero = l.$field == 0.0;
                ui.add(pct(&mut l.$field, $lo, $hi, $name, $scale));
                if reset_icon(ui, was_zero) {
                    l.$field = 0.0;
                }
            });
        }};
    }
    adj!(contrast, -0.5, 0.5, "Contrast", 200.0);
    adj!(highlights, -1.0, 1.0, "Highlights", 100.0);
    adj!(shadows, -1.0, 1.0, "Shadows", 100.0);
    adj!(whites, -1.0, 1.0, "Whites", 100.0);
    adj!(blacks, -1.0, 1.0, "Blacks", 100.0);
    adj!(temperature, -1.0, 1.0, "Temperature", 100.0);
    adj!(tint, -1.0, 1.0, "Tint", 100.0);
    adj!(saturation, -1.0, 1.0, "Saturation", 100.0);
    adj!(clarity, -1.0, 1.0, "Clarity", 100.0);
    adj!(texture, -1.0, 1.0, "Texture", 100.0);
    adj!(dehaze, -1.0, 1.0, "Dehaze", 100.0);

    app.mask_erase = erase;
    app.mask_overlay = overlay;
    app.brush_radius = brush_size;
    app.brush_hardness = hardness;
    if pick_clicked {
        app.picking = if picking { None } else { Some(PickTarget::MaskColor) };
        app.status = "Click the picture to pick the colour for this mask (Esc to cancel).".into();
    }
    if done {
        app.mask_edit = None;
    }
}
