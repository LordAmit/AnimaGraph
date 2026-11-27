//! Parts of the Export card: choosing the saved size (with Instagram and
//! custom sizes), and saving every open photo at once.

use eframe::egui;

use crate::app::StudioApp;
use crate::export_size::{CUSTOM_RANGE, ExportSize, Side, natural_size};
use crate::theme;

pub fn size_picker(app: &mut StudioApp, ui: &mut egui::Ui) {
    ui.horizontal(|ui| {
        ui.label("Size");
        let shown = if app.export_custom { "Custom".into() } else { app.export_size.label() };
        egui::ComboBox::from_id_salt("size").selected_text(shown).width(190.0).show_ui(ui, |ui| {
            let presets = [
                ExportSize::INSTAGRAM,
                ExportSize::long(crate::scene::WORK_SIZE),
                ExportSize::long(1024),
                ExportSize::long(1600),
                ExportSize::long(2048),
                ExportSize::ORIGINAL,
            ];
            for s in presets {
                if ui.selectable_label(!app.export_custom && app.export_size == s, s.label()).clicked() {
                    app.export_size = s;
                    app.export_custom = false;
                }
            }
            if ui.selectable_label(app.export_custom, "Custom…").clicked() {
                app.export_custom = true;
                if app.export_size.px == 0 {
                    app.export_size = ExportSize::long(2048);
                }
            }
        });
    });
    if app.export_custom {
        ui.horizontal(|ui| {
            ui.add(egui::DragValue::new(&mut app.export_size.px).range(CUSTOM_RANGE).suffix(" px").speed(4.0))
                .on_hover_text("Click to type a size, or drag");
            for side in Side::ALL {
                ui.selectable_value(&mut app.export_size.side, side, side.label());
            }
        });
    }
    let note = |ui: &mut egui::Ui, text: String| {
        ui.label(egui::RichText::new(text).size(12.0).color(theme::TEXT_DIM));
    };
    if let Some(sc) = &app.scene {
        // The whole video is saved from the file, at its own size.
        let source = match app.video_origin() {
            Some(o) if app.export_format.is_video() && app.video_whole => o.size,
            _ => sc.full.dimensions(),
        };
        let natural = natural_size(source, sc.crop, &app.border);
        let (w, h) = app.export_size.output(natural);
        note(ui, format!("Saves {w} × {h} px, crop and border included"));
        if app.export_size.too_big(natural) {
            note(ui, "The picture is smaller than this size; pictures are not enlarged.".into());
        }
    }
    if app.export_size.px == 0 || app.export_size.px > 1600 {
        note(ui, "Large sizes take longer and use more memory.".into());
    }
}

/// "Save all photos", or progress and Stop while they are being saved.
pub fn save_all(app: &mut StudioApp, ui: &mut egui::Ui) {
    if app.batch.is_some() {
        ui.horizontal(|ui| {
            ui.spinner();
            if let Some(b) = &app.batch {
                ui.label(egui::RichText::new(b.label()).size(12.0).color(theme::TEXT_DIM));
            }
            if ui.button("Stop").on_hover_text("Stop after the photo being saved now").clicked() {
                app.stop_batch();
            }
        });
        return;
    }
    let n = app.photo_ids().len();
    if n == 0 {
        return;
    }
    let still = app.export_format.is_still();
    let button = egui::Button::new(format!("Save all {n} photos…"));
    if ui
        .add_enabled(n > 1 && still && !app.busy(), button)
        .on_hover_text(
            "Saves every open photo into a folder you choose, each with its own edits, in this format and size. \
             Existing files are kept: a number is added to the name.",
        )
        .on_disabled_hover_text(if still {
            "Open at least two photos"
        } else {
            "Choose a still image format (PNG, JPEG or HEIC)"
        })
        .clicked()
    {
        app.export_photos(app.photo_ids());
    }
}

/// How many photos Export saves together, when photos are selected in Files.
pub fn selected_to_save(app: &StudioApp) -> Option<usize> {
    if !app.export_format.is_still() {
        return None;
    }
    app.selected_photo_ids().map(|ids| ids.len())
}

/// What the Export button saves, and at which size.
pub fn export_hint(app: &StudioApp, several: Option<usize>) -> String {
    if app.layout_open {
        return "Saves the layout, at the format and size chosen in the layout settings".into();
    }
    let (format, size) = (app.export_format.label(), app.export_size.label());
    match several {
        Some(n) => format!(
            "Saves {} (the open one and those selected in Files) into a folder you choose, each with its own \
             edits. {format}, {size}. Videos are skipped; existing files are kept.",
            crate::batch_export::photos(n)
        ),
        None => format!("{format}, {size}: the format and size chosen in the Export drawer"),
    }
}

/// For a video file: save the whole edited video, or the selected loop.
pub fn video_choice(app: &mut StudioApp, ui: &mut egui::Ui) {
    let is_video = app.scene.as_ref().is_some_and(|s| s.is_video());
    if !is_video {
        return;
    }
    if app.video_origin().is_none() {
        if app.export_format.is_video() {
            note(ui, "Only loops can be saved from GIFs and photo sequences.");
        }
        return;
    }
    if !app.export_format.is_video() {
        note(ui, "To save the whole video with your edits, choose MP4 or MOV video.");
        return;
    }
    ui.horizontal(|ui| {
        ui.label("Save");
        ui.selectable_value(&mut app.video_whole, true, "Edited video").on_hover_text(
            "The whole video, every frame with your edits (Edit, crop, straighten, border, caption), \
             at the size below and with its sound. No loop is made.",
        );
        ui.selectable_value(&mut app.video_whole, false, "Loop").on_hover_text("The loop selected in Results");
    });
    if app.video_whole {
        note(ui, "Whole length: Trim and Speed apply to loops only.");
    }
}

fn note(ui: &mut egui::Ui, text: &str) {
    ui.label(egui::RichText::new(text).size(12.0).color(theme::TEXT_DIM));
}
