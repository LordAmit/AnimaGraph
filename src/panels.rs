//! Top bar, right-hand settings panel and the results strip.

use eframe::egui::{self, Color32, CornerRadius, Pos2, Rect, Sense, Stroke, Vec2};

use crate::app::{StudioApp, Tool, View};
use crate::export::Format;
use crate::icons::{self, Icon};
use crate::theme;
use crate::app::defaults;

/// Clickable card title with a chevron; clicking folds or unfolds the card.
fn fold_title(ui: &mut egui::Ui, app: &mut StudioApp, key: &str, title: &str) {
    let open = !app.prefs.collapsed.contains(key);
    let icon = if open { Icon::ChevronDown } else { Icon::ChevronRight };
    let b = egui::Button::image_and_text(
        icon.image(12.0, theme::TEXT_DIM),
        egui::RichText::new(title.to_uppercase()).size(11.0).color(theme::TEXT_DIM).strong(),
    )
    .frame(false);
    if ui.add(b).on_hover_text(if open { "Fold" } else { "Unfold" }).clicked() {
        if open {
            app.prefs.collapsed.insert(key.to_string());
        } else {
            app.prefs.collapsed.remove(key);
        }
    }
}

fn is_open(app: &StudioApp, key: &str) -> bool {
    !app.prefs.collapsed.contains(key)
}

/// Every panel the View menu can show or hide: (key, name).
pub const PANELS: [(&str, &str); 9] = [
    ("region", "Moving region / selected area"),
    ("motion", "Motion"),
    ("loop", "Loop"),
    ("video", "Video"),
    ("crop", "Rotate & crop"),
    ("border", "Border"),
    ("caption", "Caption"),
    ("look", "Look"),
    ("export", "Export"),
];

/// Small "back to default" button shown after a setting; grayed out when
/// the setting already has its default value.
fn reset_icon(ui: &mut egui::Ui, is_default: bool) -> bool {
    let b = egui::Button::image(Icon::Undo.image(13.0, theme::TEXT_DIM))
        .fill(Color32::TRANSPARENT)
        .stroke(Stroke::NONE)
        .min_size(Vec2::splat(22.0));
    ui.add_enabled(!is_default, b).on_hover_text("Reset to default").clicked()
}

/// A slider followed by its reset button.
macro_rules! with_reset {
    ($ui:expr, $val:expr, $def:expr, $slider:expr) => {{
        let mut changed = false;
        $ui.horizontal(|ui| {
            changed = ui.add($slider).changed();
            if reset_icon(ui, $val == $def) {
                $val = $def;
                changed = true;
            }
        });
        changed
    }};
}

// ----- Top bar -------------------------------------------------------------

pub fn top_bar(app: &mut StudioApp, ui: &mut egui::Ui) {
    ui.horizontal_centered(|ui| {
        let files_on = app.panel_visible("files");
        let files_btn = egui::Button::image(Icon::Sidebar.image(18.0, if files_on { Color32::WHITE } else { theme::TEXT_DIM }))
            .fill(if files_on { theme::ACCENT_SOFT } else { Color32::TRANSPARENT })
            .min_size(Vec2::splat(30.0));
        if ui.add(files_btn).on_hover_text("Show or hide the Files panel").clicked() {
            app.set_panel_visible("files", !files_on);
        }
        view_menu(app, ui);
        ui.add_space(4.0);
        ui.add(Icon::Logo.image(26.0, Color32::WHITE));
        ui.label(egui::RichText::new("Animagraph").size(15.0).strong());
        if let Some(s) = &app.scene {
            ui.label(egui::RichText::new(format!("·  {}", s.name)).color(theme::TEXT_DIM));
        }

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let can_export = app.can_export();
            let export = icons::button(Icon::Download, egui::RichText::new("Export").color(Color32::WHITE).strong(), Color32::WHITE)
                .fill(if can_export { theme::ACCENT } else { theme::ACCENT_SOFT })
                .min_size(Vec2::new(96.0, 32.0));
            if ui
                .add_enabled(can_export, export)
                .on_disabled_hover_text("Animate a loop first, or choose a still image format in Export")
                .clicked()
            {
                app.start_export();
            }
            if ui.add(icons::button(Icon::Folder, "Open photo or video", theme::TEXT).min_size(Vec2::new(0.0, 32.0))).clicked() {
                let ctx = ui.ctx().clone();
                app.open_photo_dialog(&ctx);
            }
            ui.add_space(12.0);
            ui.scope(|ui| {
                ui.set_max_width((ui.available_width() - 260.0).clamp(120.0, 520.0));
                ui.add(
                    egui::Label::new(egui::RichText::new(&app.status).color(theme::TEXT_DIM).size(12.0))
                        .truncate(),
                )
                .on_hover_text(&app.status);
            });
        });
    });
}

/// Settings-bar panels grouped as they appear in the View menu.
pub const MOTION_PANELS: [(&str, &str); 4] = [
    ("region", "Moving region / selected area"),
    ("motion", "Motion"),
    ("loop", "Loop"),
    ("video", "Video"),
];
pub const IMAGE_PANELS: [(&str, &str); 4] = [
    ("crop", "Rotate & crop"),
    ("border", "Border"),
    ("caption", "Caption"),
    ("look", "Look"),
];

fn view_toggle(ui: &mut egui::Ui, app: &mut StudioApp, key: &str, name: &str) {
    let mut on = app.panel_visible(key);
    if ui.checkbox(&mut on, name).changed() {
        app.set_panel_visible(key, on);
    }
}

/// A View submenu: one button that shows or hides the whole group, then
/// a checkbox per panel.
fn view_group(ui: &mut egui::Ui, app: &mut StudioApp, title: &str, panels: &[(&str, &str)]) {
    ui.menu_button(title, |ui| {
        ui.set_min_width(240.0);
        let all_on = panels.iter().all(|(k, _)| app.panel_visible(k));
        let label = if all_on { "Hide all" } else { "Show all" };
        let b = egui::Button::new(egui::RichText::new(label).strong()).min_size(Vec2::new(ui.available_width(), 26.0));
        if ui.add(b).on_hover_text(format!("Show or hide every {title} panel at once")).clicked() {
            for (k, _) in panels {
                app.set_panel_visible(k, !all_on);
            }
        }
        ui.separator();
        for (key, name) in panels {
            view_toggle(ui, app, key, name);
        }
    });
}

/// View menu: show or hide each panel, fold or unfold them all.
fn view_menu(app: &mut StudioApp, ui: &mut egui::Ui) {
    ui.menu_button(egui::RichText::new("View").size(13.5), |ui| {
        ui.set_min_width(230.0);
        view_toggle(ui, app, "files", "Files panel");
        view_toggle(ui, app, "results", "Results strip");
        view_toggle(ui, app, "controls", "Settings bar (right)");
        ui.separator();
        view_group(ui, app, "Motion", &MOTION_PANELS);
        view_group(ui, app, "Image", &IMAGE_PANELS);
        view_toggle(ui, app, "export", "Export");
        ui.separator();
        if ui.button("Show everything").clicked() {
            app.prefs.hidden.clear();
        }
        if ui.button("Unfold all panels").clicked() {
            app.prefs.collapsed.clear();
        }
        if ui.button("Fold all panels").clicked() {
            for (key, _) in PANELS {
                app.prefs.collapsed.insert(key.to_string());
            }
        }
    });
}

// ----- Side panel ----------------------------------------------------------

pub fn side_panel(app: &mut StudioApp, ui: &mut egui::Ui) {
    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        ui.spacing_mut().item_spacing.y = 10.0;
        // Keep everything inside the bar's width.
        ui.set_max_width(ui.available_width().min(312.0));
        let is_video = app.scene.as_ref().is_some_and(|s| s.is_video());
        let (title, sub) = if is_video {
            ("Freeze part of a video", "Select an area, choose whether it freezes or plays, then press Make loop.")
        } else {
            ("Animate a photo", "Paint the area to animate, draw arrows in the direction of motion, then press Animate.")
        };
        ui.label(egui::RichText::new(title).size(18.0).strong());
        ui.label(egui::RichText::new(sub).color(theme::TEXT_DIM));
        ui.add_space(2.0);

        let is_video = app.scene.as_ref().is_some_and(|s| s.is_video());
        if app.panel_visible("region") {
            region_card(app, ui);
        }
        if is_video {
            if app.panel_visible("video") {
                crate::panels::video_card(app, ui);
            }
        } else {
            if app.panel_visible("motion") {
                motion_card(app, ui);
            }
            if app.panel_visible("loop") {
                loop_card(app, ui);
            }
        }

        if app.panel_visible("crop") {
            crate::panels::crop_card(app, ui);
        }
        if app.panel_visible("border") {
            crate::panels::border_card(app, ui);
        }
        if app.panel_visible("caption") {
            crate::panels::caption_card(app, ui);
        }
        if app.panel_visible("look") {
            crate::panels::look_card(app, ui);
        }
        generate_block(app, ui);
        if app.panel_visible("export") {
            export_card(app, ui);
        }
        ui.add_space(4.0);
        if ui
            .add(icons::button(Icon::Undo, "Reset all settings", theme::TEXT_DIM).fill(Color32::TRANSPARENT))
            .on_hover_text("Speed, lengths, look, border and other settings go back to their defaults. Painted areas, arrows and crops are kept.")
            .clicked()
        {
            app.reset_settings();
        }
        ui.add_space(8.0);
    });
}

fn tool_chip(ui: &mut egui::Ui, app: &mut StudioApp, tool: Tool, icon: Icon, label: &str) {
    let sel = app.tool == tool && app.view == View::Edit;
    let color = if sel { Color32::WHITE } else { theme::TEXT };
    let b = icons::button(icon, egui::RichText::new(label).color(color), color)
        .fill(if sel { theme::ACCENT } else { theme::PANEL })
        .min_size(Vec2::new(0.0, 28.0));
    if ui.add(b).clicked() {
        app.pick_tool(tool);
    }
}

fn region_card(app: &mut StudioApp, ui: &mut egui::Ui) {
    let has_scene = app.scene.is_some();
    let is_video = app.scene.as_ref().is_some_and(|s| s.is_video());
    theme::card().show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal(|ui| {
            fold_title(ui, app, "region", if is_video { "1  Selected area" } else { "1  Moving region" });
            if let Some(s) = &app.scene {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(
                        egui::RichText::new(format!("{:.0}% of {}", s.mask_coverage() * 100.0, if s.is_video() { "frame" } else { "photo" }))
                            .size(11.0)
                            .color(theme::TEXT_DIM),
                    );
                });
            }
        });
        if !is_open(app, "region") {
            return;
        }
        ui.add_enabled_ui(has_scene, |ui| {
            if is_video {
                ui.horizontal(|ui| {
                    let w = (ui.available_width() - 8.0) / 2.0;
                    for (freeze, icon, label, tip) in [
                        (true, Icon::Snow, "Area freezes", "The selected area holds still; the rest of the video plays"),
                        (false, Icon::Play, "Area plays", "Only the selected area plays; the rest holds still"),
                    ] {
                        let sel = app.freeze_painted == freeze;
                        let color = if sel { Color32::WHITE } else { theme::TEXT_DIM };
                        let b = icons::button(icon, egui::RichText::new(label).color(color), color)
                        .fill(if sel { theme::ACCENT_SOFT } else { theme::PANEL })
                        .stroke(Stroke::new(1.0, if sel { theme::ACCENT } else { theme::BORDER }))
                        .min_size(Vec2::new(w, 30.0));
                        if ui.add(b).on_hover_text(tip).clicked() {
                            app.freeze_painted = freeze;
                        }
                    }
                });
            }
            ui.horizontal(|ui| {
                tool_chip(ui, app, Tool::Rect, Icon::Box, "Box");
                tool_chip(ui, app, Tool::Brush, Icon::Brush, "Paint");
                tool_chip(ui, app, Tool::Eraser, Icon::Eraser, "Erase");
                ui.checkbox(&mut app.show_mask, "Show");
            });
            ui.label(
                egui::RichText::new("Drag a box to add an area; hold Alt/Option while dragging to remove one.")
                    .size(12.0)
                    .color(theme::TEXT_DIM),
            );
            with_reset!(ui, app.brush_radius, defaults::BRUSH_RADIUS, egui::Slider::new(&mut app.brush_radius, 3.0..=120.0).text("Brush size").integer());
            with_reset!(ui, app.brush_hardness, defaults::BRUSH_HARDNESS, egui::Slider::new(&mut app.brush_hardness, 0.0..=1.0).text("Hardness"));
            ui.horizontal_wrapped(|ui| {
                if !is_video && ui
                    .add(icons::button(Icon::Sparkle, "Auto-detect sky & water", theme::ARROW))
                    .on_hover_text("A quick color-based guess. Touch it up with the brush.")
                    .clicked()
                {
                    if let Some(s) = &mut app.scene {
                        s.checkpoint();
                        let m = crate::automask::guess_sky_and_water(&s.work);
                        s.set_mask(m);
                        app.status = "Auto-detected sky and water. Check the pink area and fix it with the brush.".into();
                    }
                }
                if ui.button("Fill").clicked() {
                    if let Some(s) = &mut app.scene {
                        s.checkpoint();
                        s.fill_mask(1.0);
                    }
                }
                if ui.button("Invert").clicked() {
                    if let Some(s) = &mut app.scene {
                        s.checkpoint();
                        s.invert_mask();
                    }
                }
                if ui.button("Clear").clicked() {
                    if let Some(s) = &mut app.scene {
                        s.checkpoint();
                        s.fill_mask(0.0);
                    }
                }
            });
        });
    });
}

fn motion_card(app: &mut StudioApp, ui: &mut egui::Ui) {
    let has_scene = app.scene.is_some();
    theme::card().show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal(|ui| {
            fold_title(ui, app, "motion", "2  Motion");
            if let Some(s) = &app.scene {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(
                        egui::RichText::new(format!("{} arrows · {} pins", s.arrows.len(), s.pins.len()))
                            .size(11.0)
                            .color(theme::TEXT_DIM),
                    );
                });
            }
        });
        if !is_open(app, "motion") {
            return;
        }
        ui.add_enabled_ui(has_scene, |ui| {
            ui.horizontal(|ui| {
                tool_chip(ui, app, Tool::Arrow, Icon::Arrow, "Arrow");
                tool_chip(ui, app, Tool::Pin, Icon::Pin, "Pin");
                ui.checkbox(&mut app.show_hints, "Show");
            });
            ui.label(
                egui::RichText::new("Drag to draw an arrow in the direction of motion. Pins keep a point still. Right-click removes one.")
                    .size(12.0)
                    .color(theme::TEXT_DIM),
            );
            with_reset!(ui, app.speed, defaults::SPEED, egui::Slider::new(&mut app.speed, 0.1..=3.0).text("Speed").suffix("×"));

            // Arrow length.
            ui.horizontal(|ui| {
                ui.checkbox(&mut app.arrow_fixed, "Same length arrows")
                    .on_hover_text("New arrows use the length below. Dragging sets only the direction.");
            });
            with_reset!(
                ui,
                app.arrow_len,
                defaults::ARROW_LEN,
                egui::Slider::new(&mut app.arrow_len, 0.02..=0.5)
                    .text("Length")
                    .custom_formatter(|v, _| format!("{:.0}%", v * 100.0))
                    .custom_parser(|t| t.trim_end_matches('%').trim().parse::<f64>().ok().map(|v| v / 100.0))
            );
            if ui
                .button("Make all arrows this length")
                .on_hover_text("Keeps each arrow's start and direction")
                .clicked()
            {
                if let Some(s) = &mut app.scene {
                    if !s.arrows.is_empty() {
                        s.checkpoint();
                        let [w, h] = s.work_size();
                        for a in s.arrows.iter_mut() {
                            if let Some(end) = crate::canvas::fixed_arrow_end(a.from, a.to, app.arrow_len, w, h) {
                                a.to = end;
                            }
                        }
                    }
                }
            }
            ui.horizontal(|ui| {
                ui.add(
                    egui::DragValue::new(&mut app.auto_angle)
                        .range(-180.0..=180.0)
                        .suffix("°")
                        .speed(1.0),
                )
                .on_hover_text("Direction for quick arrows: 0° right, 90° down, 180° left, -90° up");
                if ui
                    .button("Fill region with arrows")
                    .on_hover_text("Adds a grid of arrows in this direction over the painted area")
                    .clicked()
                {
                    if let Some(s) = &mut app.scene {
                        let [w, h] = s.work_size();
                        let arrows = crate::motion::auto_arrows(&s.mask, w, h, app.auto_angle, app.arrow_len);
                        if arrows.is_empty() {
                            app.status = "Paint the moving region first, then fill it with arrows.".into();
                        } else {
                            s.checkpoint();
                            s.arrows.extend(arrows);
                        }
                    }
                }
            });
            if ui.button("Clear arrows and pins").clicked() {
                if let Some(s) = &mut app.scene {
                    s.checkpoint();
                    s.arrows.clear();
                    s.pins.clear();
                }
            }
        });
    });
}

fn loop_card(app: &mut StudioApp, ui: &mut egui::Ui) {
    theme::card().show(ui, |ui| {
        ui.set_width(ui.available_width());
        fold_title(ui, app, "loop", "3  Loop");
        if !is_open(app, "loop") {
            return;
        }
        with_reset!(ui, app.loop_seconds, defaults::LOOP_SECONDS, egui::Slider::new(&mut app.loop_seconds, 1.0..=8.0).text("Length").suffix(" s").step_by(0.5));
        ui.horizontal(|ui| {
            ui.label("Frame rate");
            for fps in [12u32, 24, 30] {
                let sel = app.fps == fps;
                if ui.selectable_label(sel, format!("{fps} fps")).clicked() {
                    app.fps = fps;
                }
            }
        });
        let frames = (app.loop_seconds * app.fps as f32).round() as usize;
        ui.label(egui::RichText::new(format!("{frames} frames, plays back seamlessly")).size(12.0).color(theme::TEXT_DIM));
        crate::panels::loop_style(app, ui);
    });
}

fn progress_row(app: &mut StudioApp, ui: &mut egui::Ui) {
    if let Some(t) = &app.task {
        let frac = t.progress.fraction();
        let label = t.label.clone();
        ui.horizontal(|ui| {
            ui.add(
                egui::ProgressBar::new(frac)
                    .text(format!("{label}… {:.0}%", frac * 100.0))
                    .desired_width(ui.available_width() - 76.0),
            );
            if ui.button("Cancel").clicked() {
                app.cancel_task();
            }
        });
    }
}

fn generate_block(app: &mut StudioApp, ui: &mut egui::Ui) {
    let is_video = app.scene.as_ref().is_some_and(|s| s.is_video());
    let ready = app
        .scene
        .as_ref()
        .is_some_and(|s| s.is_video() || (!s.arrows.is_empty() && s.mask_coverage() > 0.0));
    let enabled = ready && !app.busy();
    ui.add_space(2.0);
    let (icon, text) = match (app.busy(), is_video) {
        (true, _) => (None, "Working…"),
        (false, true) => (Some(Icon::Loop), "Make loop"),
        (false, false) => (Some(Icon::Sparkle), "Animate"),
    };
    let r = theme::primary_button(ui, icon, text, enabled);
    let r = if !ready {
        r.on_disabled_hover_text("Paint a moving region and draw at least one arrow")
    } else {
        r
    };
    if r.clicked() {
        let ctx = ui.ctx().clone();
        app.start_preview(&ctx);
    }
    progress_row(app, ui);
}

fn export_card(app: &mut StudioApp, ui: &mut egui::Ui) {
    theme::card().show(ui, |ui| {
        ui.set_width(ui.available_width());
        fold_title(ui, app, "export", "4  Export");
        if !is_open(app, "export") {
            return;
        }
        ui.horizontal(|ui| {
            ui.label("Format");
            egui::ComboBox::from_id_salt("format")
                .selected_text(app.export_format.label())
                .width(150.0)
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut app.export_format, Format::Gif, Format::Gif.label());
                    ui.selectable_value(&mut app.export_format, Format::PngFrames, Format::PngFrames.label());
                    ui.add_enabled_ui(app.ffmpeg, |ui| {
                        for f in [Format::Mp4, Format::Mov] {
                            ui.selectable_value(&mut app.export_format, f, f.label())
                                .on_disabled_hover_text("Saving video needs ffmpeg on this computer");
                        }
                    });
                    ui.separator();
                    for f in [Format::Png, Format::Jpeg] {
                        ui.selectable_value(&mut app.export_format, f, f.label())
                            .on_hover_text("Single still image with look, crop, caption and border");
                    }
                });
        });
        if app.export_format.is_video() {
            with_reset!(ui, app.video_repeats, defaults::REPEATS, egui::Slider::new(&mut app.video_repeats, 1..=10).text("Plays").suffix("×"));
            if let Some(c) = app.active_clip.and_then(|i| app.clips.get(i)) {
                ui.label(
                    egui::RichText::new(format!("Video length {:.1}s (H.264)", c.seconds() * app.video_repeats as f32))
                        .size(12.0)
                        .color(theme::TEXT_DIM),
                );
            }
        }
        if let Some(sc) = &app.scene {
            let [_, _, w, h] = sc.crop.pixels(sc.full.width(), sc.full.height());
            let text = if sc.crop.is_full() {
                "Crop: none (whole picture)".to_string()
            } else {
                format!("Crop: {} · {w} × {h} px", sc.aspect.label())
            };
            ui.label(egui::RichText::new(text).size(12.0).color(theme::TEXT_DIM));
        }
        ui.horizontal(|ui| {
            ui.label("Size");
            let label = |s: u32| match s {
                0 => "Original".to_string(),
                n => format!("{n} px"),
            };
            egui::ComboBox::from_id_salt("size")
                .selected_text(label(app.export_size))
                .width(150.0)
                .show_ui(ui, |ui| {
                    for s in [crate::scene::WORK_SIZE, 1024, 1600, 2048, 0] {
                        ui.selectable_value(&mut app.export_size, s, label(s));
                    }
                });
        });
        if app.export_size == 0 || app.export_size > 1600 {
            ui.label(
                egui::RichText::new("Large sizes take longer and use more memory.")
                    .size(12.0)
                    .color(theme::TEXT_DIM),
            );
        }
        // Pictures are never enlarged: say so when the source is smaller.
        if let Some(sc) = &app.scene {
            let longest = sc.full.width().max(sc.full.height());
            if app.export_size > longest {
                let what = if sc.is_video() { "This video's frames are" } else { "This picture is" };
                ui.label(
                    egui::RichText::new(format!("{what} {longest} px on the long side, so it is saved at {longest} px."))
                        .size(12.0)
                        .color(theme::TEXT_DIM),
                );
            }
        }
        let can = app.can_export();
        let still = app.export_format.is_still();
        ui.horizontal(|ui| {
            if ui
                .add_enabled(can, egui::Button::new(if still { "Save image…" } else { "Save selected loop…" }))
                .on_disabled_hover_text(if still { "Open a photo first" } else { "Animate a loop first" })
                .clicked()
            {
                app.start_export();
            }
            if let Some(p) = app.last_export.clone() {
                if ui.button("Show file").clicked() {
                    crate::export::reveal(&p);
                }
            }
        });
    });
}

// ----- Results strip -------------------------------------------------------

pub fn results_strip(app: &mut StudioApp, ui: &mut egui::Ui) {
    ui.horizontal(|ui| {
        theme::section_title(ui, "Results");
        ui.label(
            egui::RichText::new(if app.clips.is_empty() {
                "Loops you make appear here."
            } else {
                "Click a loop to play it. Kept until you close the app; export to save."
            })
            .size(11.0)
            .color(theme::TEXT_DIM),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let small = |icon: Icon, text: &str| {
                icons::button(icon, egui::RichText::new(text).size(12.0), theme::TEXT).min_size(Vec2::new(0.0, 24.0))
            };
            if ui
                .add_enabled(!app.clips.is_empty(), small(Icon::Trash, "Delete all"))
                .on_hover_text("Remove every loop from Results. Exported files are not touched.")
                .clicked()
            {
                app.delete_all_clips();
            }
            if ui
                .add_enabled(app.active_clip.is_some(), small(Icon::Trash, "Delete"))
                .on_hover_text("Remove the selected loop (Delete or Backspace). Exported files are not touched.")
                .clicked()
            {
                if let Some(i) = app.active_clip {
                    app.delete_clip(i);
                }
            }
            if !app.last_deleted.is_empty() {
                let n = app.last_deleted.len();
                let label = if n == 1 {
                    format!("Restore Loop {}", app.last_deleted[0].1.number)
                } else {
                    format!("Restore {n} loops")
                };
                if ui.add(small(Icon::Undo, &label)).on_hover_text("Restore the deleted loops").clicked() {
                    app.restore_deleted();
                }
            }
        });
    });
    egui::ScrollArea::horizontal().auto_shrink([false, true]).show(ui, |ui| {
        ui.horizontal(|ui| {
            if app.clips.is_empty() {
                for _ in 0..4 {
                    let (rect, _) = ui.allocate_exact_size(Vec2::new(112.0, 63.0), Sense::hover());
                    ui.painter().rect_stroke(
                        rect,
                        CornerRadius::same(8),
                        Stroke::new(1.0, theme::BORDER),
                        egui::StrokeKind::Inside,
                    );
                }
                return;
            }
            let t = ui.input(|i| i.time);
            let mut remove = None;
            for (i, clip) in app.clips.iter().enumerate().rev() {
                let first = &clip.textures[0];
                let [w, h] = first.size();
                let thumb_h = 63.0;
                let thumb_w = (w as f32 / h as f32 * thumb_h).clamp(60.0, 140.0);
                let (rect, resp) = ui.allocate_exact_size(Vec2::new(thumb_w, thumb_h), Sense::click());
                // Hovered thumbnails play.
                let idx = if resp.hovered() {
                    ui.ctx().request_repaint();
                    ((t * clip.recipe.fps as f64) as usize) % clip.textures.len()
                } else {
                    0
                };
                let painter = ui.painter();
                painter.image(
                    clip.textures[idx].id(),
                    rect,
                    Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                    Color32::WHITE,
                );
                let selected = app.active_clip == Some(i);
                painter.rect_stroke(
                    rect,
                    CornerRadius::same(6),
                    Stroke::new(if selected { 2.5 } else { 1.0 }, if selected { theme::ACCENT } else { theme::BORDER }),
                    egui::StrokeKind::Inside,
                );
                let badge = Rect::from_min_size(rect.left_bottom() + Vec2::new(4.0, -18.0), Vec2::new(rect.width() - 8.0, 14.0));
                painter.text(
                    badge.left_center(),
                    egui::Align2::LEFT_CENTER,
                    format!("Loop {} · {:.1}s", clip.number, clip.seconds()),
                    egui::FontId::proportional(10.5),
                    Color32::WHITE,
                );
                // Trash button on the hovered or selected thumbnail.
                if resp.hovered() || selected || ui.rect_contains_pointer(rect) {
                    let btn_rect = Rect::from_min_size(rect.right_top() + Vec2::new(-26.0, 4.0), Vec2::splat(22.0));
                    let btn = egui::Button::image(Icon::Trash.image(13.0, Color32::WHITE))
                        .fill(Color32::from_black_alpha(170))
                        .stroke(Stroke::NONE)
                        .corner_radius(CornerRadius::same(6));
                    if theme::overlay(ui, btn_rect, btn).on_hover_text("Delete this loop").clicked() {
                        remove = Some(i);
                    }
                }
                if resp.clicked() && remove.is_none() {
                    app.active_clip = Some(i);
                    app.view = View::Preview;
                    app.play_pos = 0.0;
                    app.playing = true;
                }
                resp.context_menu(|ui| {
                    if ui.button("Delete").clicked() {
                        remove = Some(i);
                        ui.close();
                    }
                });
            }
            if let Some(i) = remove {
                app.delete_clip(i);
            }
        });
    });
}

// ----- Video clip ------------------------------------------------------------

pub fn video_card(app: &mut StudioApp, ui: &mut egui::Ui) {
    let Some(scene) = &app.scene else { return };
    let Some(video) = scene.video.clone() else { return };
    let n = video.frames.len();
    let fps = video.fps;
    let mut still = scene.still_index;
    let [mut start, mut end] = app.trim_range();
    let t = |f: usize| f as f32 / fps;
    theme::card().show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal(|ui| {
            fold_title(ui, app, "video", "2  Video");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(
                    egui::RichText::new(format!("{n} frames · {:.1}s · {:.0} fps", video.seconds(), fps))
                        .size(11.0)
                        .color(theme::TEXT_DIM),
                );
            });
        });
        if !is_open(app, "video") {
            return;
        }
        ui.horizontal(|ui| {
            ui.checkbox(&mut app.live, "Live preview")
                .on_hover_text("Plays the clip on the canvas while you edit");
            let icon = if app.playing { Icon::Pause } else { Icon::Play };
            if ui
                .add(egui::Button::image(icon.image(14.0, theme::TEXT)).min_size(Vec2::splat(26.0)))
                .on_hover_text("Play / pause (Space)")
                .clicked()
            {
                app.playing = !app.playing;
            }
        });

        // Trim.
        ui.label(egui::RichText::new("Trim").strong());
        let r1 = ui.add(egui::Slider::new(&mut start, 0..=n - 2).text("Start").custom_formatter(move |v, _| format!("{:.2}s", v as f32 / fps)));
        let r2 = ui.add(egui::Slider::new(&mut end, 1..=n - 1).text("End").custom_formatter(move |v, _| format!("{:.2}s", v as f32 / fps)));
        if r1.changed() || r2.changed() {
            if r1.changed() && start >= end {
                end = (start + 1).min(n - 1);
            }
            if r2.changed() && end <= start {
                start = end.saturating_sub(1);
            }
            if let Some(s) = &mut app.scene {
                s.trim = [start, end];
            }
        }
        ui.label(
            egui::RichText::new(format!("Using {:.2}s to {:.2}s", t(start), t(end + 1)))
                .size(12.0)
                .color(theme::TEXT_DIM),
        );

        // Still frame.
        let r = ui
            .add(egui::Slider::new(&mut still, 0..=n - 1).text("Still frame").custom_formatter(move |v, _| format!("{:.2}s", v as f32 / fps)))
            .on_hover_text("The frame shown in the frozen area");
        if r.changed() {
            let ctx = ui.ctx().clone();
            app.set_still(&ctx, still);
            app.view = View::Edit;
        }

        with_reset!(ui, app.video_speed, defaults::VIDEO_SPEED, egui::Slider::new(&mut app.video_speed, 0.25..=2.0).text("Speed").suffix("×"));
        ui.checkbox(&mut app.stabilize, "Stabilize handheld clip")
            .on_hover_text("Aligns each frame to the still frame and zooms in slightly to hide the edges. Applied when the loop is made.");

        // Loop style.
        loop_style(app, ui);
        ui.add_enabled_ui(!app.bounce, |ui| {
            let used = end - start + 1;
            let max_fade = (used as f32 / fps / 2.0).max(0.1);
            with_reset!(
                ui,
                app.crossfade_seconds,
                defaults::CROSSFADE,
                egui::Slider::new(&mut app.crossfade_seconds, 0.0..=max_fade.min(1.5)).text("Blend").suffix(" s")
            );
        });
        let used = end - start + 1;
        let blend = if app.bounce { 0 } else { ((app.crossfade_seconds * fps).round() as usize).min(used / 2) };
        let frames = if app.bounce { 2 * used - 2 } else { used - blend };
        let len = frames as f32 / (fps * app.video_speed);
        ui.label(egui::RichText::new(format!("Loop length {len:.1}s")).size(12.0).color(theme::TEXT_DIM));
        ui.separator();
        with_reset!(ui, app.video_seconds, defaults::VIDEO_SECONDS, egui::Slider::new(&mut app.video_seconds, 1.0..=10.0).text("Read").suffix(" s").step_by(0.5));
        ui.label(
            egui::RichText::new("Length read from the next video you open. Short clips (about 3 s) work best.")
                .size(11.0)
                .color(theme::TEXT_DIM),
        );
    });
}

/// Loop (start again) or Bounce (play forward, then backward).
fn loop_style(app: &mut StudioApp, ui: &mut egui::Ui) {
    ui.horizontal(|ui| {
        let w = (ui.available_width() - 8.0) / 2.0;
        for (bounce, icon, label, tip) in [
            (false, Icon::Loop, "Loop", "Plays to the end and starts again"),
            (true, Icon::Bounce, "Bounce", "Plays forward, then backward"),
        ] {
            let sel = app.bounce == bounce;
            let color = if sel { Color32::WHITE } else { theme::TEXT_DIM };
            let b = icons::button(icon, egui::RichText::new(label).color(color), color)
                .fill(if sel { theme::ACCENT_SOFT } else { theme::PANEL })
                .stroke(Stroke::new(1.0, if sel { theme::ACCENT } else { theme::BORDER }))
                .min_size(Vec2::new(w, 28.0));
            if ui.add(b).on_hover_text(tip).clicked() {
                app.bounce = bounce;
            }
        }
    });
}

/// Color look presets and fine adjustments.
pub fn look_card(app: &mut StudioApp, ui: &mut egui::Ui) {
    let has_scene = app.scene.is_some();
    theme::card().show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal(|ui| {
            fold_title(ui, app, "look", "Look");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.add_enabled(!app.look.is_neutral(), egui::Button::new("Reset").small()).clicked() {
                    app.look = crate::finish::Look::default();
                }
            });
        });
        if !is_open(app, "look") {
            return;
        }
        ui.add_enabled_ui(has_scene, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = Vec2::new(6.0, 6.0);
                for (name, preset) in crate::finish::Look::presets() {
                    let sel = app.look == preset;
                    let b = egui::Button::new(egui::RichText::new(name).size(12.0).color(if sel {
                        Color32::WHITE
                    } else {
                        theme::TEXT
                    }))
                    .fill(if sel { theme::ACCENT } else { theme::PANEL })
                    .corner_radius(CornerRadius::same(12));
                    if ui.add(b).clicked() {
                        app.look = preset;
                    }
                }
            });
            let l = &mut app.look;
            with_reset!(ui, l.exposure, 0.0, egui::Slider::new(&mut l.exposure, -1.0..=1.0).text("Exposure"));
            with_reset!(ui, l.contrast, 0.0, egui::Slider::new(&mut l.contrast, -0.5..=0.5).text("Contrast"));
            with_reset!(ui, l.saturation, 0.0, egui::Slider::new(&mut l.saturation, -1.0..=1.0).text("Saturation"));
            with_reset!(ui, l.warmth, 0.0, egui::Slider::new(&mut l.warmth, -1.0..=1.0).text("Warmth"));
            with_reset!(ui, l.fade, 0.0, egui::Slider::new(&mut l.fade, 0.0..=1.0).text("Fade").fixed_decimals(2));
            with_reset!(ui, l.vignette, 0.0, egui::Slider::new(&mut l.vignette, 0.0..=1.0).text("Vignette").fixed_decimals(2));
            ui.label(egui::RichText::new("Tone curve").size(12.0).color(theme::TEXT_DIM));
            with_reset!(ui, l.s_curve, 0.0, egui::Slider::new(&mut l.s_curve, -1.0..=1.0).text("S-curve").fixed_decimals(2))
                ;
            with_reset!(
                ui,
                l.pivot,
                crate::finish::DEFAULT_PIVOT,
                egui::Slider::new(&mut l.pivot, 0.1..=0.9).text("Pivot").fixed_decimals(2)
            );
            with_reset!(ui, l.shadows, 0.0, egui::Slider::new(&mut l.shadows, -1.0..=1.0).text("Shadows").fixed_decimals(2));
            with_reset!(ui, l.highlights, 0.0, egui::Slider::new(&mut l.highlights, -1.0..=1.0).text("Highlights").fixed_decimals(2));
            ui.label(egui::RichText::new("Grain").size(12.0).color(theme::TEXT_DIM));
            with_reset!(ui, l.grain, 0.0, egui::Slider::new(&mut l.grain, 0.0..=1.0).text("Amount").fixed_decimals(2));
            with_reset!(
                ui,
                l.grain_size,
                crate::finish::DEFAULT_GRAIN_SIZE,
                egui::Slider::new(&mut l.grain_size, 1.0..=4.0).text("Size").fixed_decimals(1)
            );
        });
    });
}

// ----- Crop ----------------------------------------------------------------

/// Crop tool button, aspect-ratio choices, the resulting size and Reset.
pub fn crop_card(app: &mut StudioApp, ui: &mut egui::Ui) {
    rotate_row_card(app, ui);
}

/// Crop and rotate share one card: rotate buttons on top, then crop.
fn rotate_row_card(app: &mut StudioApp, ui: &mut egui::Ui) {
    let Some(sc) = &app.scene else { return };
    let (iw, ih) = (sc.full.width(), sc.full.height());
    let crop = sc.crop;
    let aspect = sc.aspect;
    theme::card().show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal(|ui| {
            fold_title(ui, app, "crop", "Rotate & crop");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .add_enabled(!crop.is_full(), egui::Button::new("Reset crop").small())
                    .on_hover_text("Keep the whole picture")
                    .clicked()
                {
                    if let Some(s) = &mut app.scene {
                        s.checkpoint();
                    }
                    app.set_crop(crate::crop::CropRect::FULL);
                    if let Some(s) = &mut app.scene {
                        s.aspect = crate::crop::Aspect::Free;
                        s.crop_applied = false;
                    }
                }
            });
        });
        if !is_open(app, "crop") {
            return;
        }
        ui.horizontal(|ui| {
            let busy = app.busy();
            for (icon, label, tip, cw) in [
                (Icon::RotateLeft, "Left", "Rotate a quarter turn to the left ( [ )", false),
                (Icon::RotateRight, "Right", "Rotate a quarter turn to the right ( ] )", true),
            ] {
                let b = icons::button(icon, label, theme::TEXT).min_size(Vec2::new(0.0, 28.0));
                if ui
                    .add_enabled(!busy, b)
                    .on_hover_text(tip)
                    .on_disabled_hover_text("Wait for the current job to finish")
                    .clicked()
                {
                    let ctx = ui.ctx().clone();
                    app.rotate_current(&ctx, cw);
                }
            }
        });
        let active = app.tool == Tool::Crop && app.view == View::Edit;
        let color = if active { Color32::WHITE } else { theme::TEXT };
        let b = icons::button(Icon::Crop, egui::RichText::new(if active { "Cropping" } else { "Crop tool" }).color(color), color)
            .fill(if active { theme::ACCENT } else { theme::PANEL })
            .min_size(Vec2::new(0.0, 28.0));
        ui.horizontal(|ui| {
            if ui.add(b).on_hover_text("Drag on the picture to crop (C)").clicked() {
                app.pick_tool(Tool::Crop);
            }
            if active {
                let done = egui::Button::new(egui::RichText::new("Done").color(Color32::WHITE).strong())
                    .fill(theme::ACCENT)
                    .min_size(Vec2::new(70.0, 28.0));
                if ui.add(done).on_hover_text("Confirm the crop and hide the cut-away part (Enter)").clicked() {
                    app.finish_crop();
                }
            }
        });
        ui.label(egui::RichText::new("Aspect ratio").size(12.0).color(theme::TEXT_DIM));
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = Vec2::new(6.0, 6.0);
            for a in crate::crop::Aspect::ALL {
                let sel = aspect == a;
                let b = egui::Button::new(egui::RichText::new(a.label()).size(12.0).color(if sel {
                    Color32::WHITE
                } else {
                    theme::TEXT
                }))
                .fill(if sel { theme::ACCENT } else { theme::PANEL })
                .corner_radius(CornerRadius::same(12));
                if ui.add(b).on_hover_text(a.hint()).clicked() {
                    app.set_aspect(a);
                }
            }
        });
        let [_, _, w, h] = crop.pixels(iw, ih);
        let text = if crop.is_full() {
            format!("Whole picture · {iw} × {ih} px")
        } else {
            format!("{w} × {h} px of {iw} × {ih}")
        };
        ui.label(egui::RichText::new(text).size(12.0).color(theme::TEXT_DIM));
        ui.label(
            egui::RichText::new("Applied when you export. Double-click the picture with the crop tool to reset.")
                .size(11.0)
                .color(theme::TEXT_DIM),
        );
    });
}

// ----- Border --------------------------------------------------------------

/// Border around the output: on/off, thickness, color and shape.
pub fn border_card(app: &mut StudioApp, ui: &mut egui::Ui) {
    if app.scene.is_none() {
        return;
    }
    theme::card().show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal(|ui| {
            fold_title(ui, app, "border", "Border");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.checkbox(&mut app.border.enabled, "Add border");
            });
        });
        if !is_open(app, "border") {
            return;
        }
        let b = &mut app.border;
        ui.add_enabled_ui(b.enabled, |ui| {
            with_reset!(
                ui,
                b.size,
                crate::border::DEFAULT_SIZE,
                egui::Slider::new(&mut b.size, 0.0..=0.25)
                    .text("Thickness")
                    .custom_formatter(|v, _| format!("{:.0}%", v * 100.0))
                    .custom_parser(|t| t.trim_end_matches('%').trim().parse::<f64>().ok().map(|v| v / 100.0))
            );
            ui.horizontal(|ui| {
                ui.label("Color");
                color_swatches(ui, &mut b.color, &app.palette);
                // Plain color picker: a border has no opacity or blending.
                ui.color_edit_button_srgb(&mut b.color).on_hover_text("Any color");
                let target = crate::app::PickTarget::Border;
                let active = app.picking == Some(target);
                if eyedropper_button(ui, active).clicked() {
                    app.picking = if active { None } else { Some(target) };
                    app.status = "Click the picture to pick a color (Esc to cancel).".into();
                }
            });
            ui.label(egui::RichText::new("Output shape").size(12.0).color(theme::TEXT_DIM));
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = Vec2::new(6.0, 6.0);
                let mut choices = vec![
                    (crate::border::BorderFit::Even, "Same thickness on every side"),
                    (crate::border::BorderFit::KeepShape, "Border keeps the picture's aspect ratio"),
                    (crate::border::BorderFit::Polaroid, "Cream frame with a wide bottom edge. Sets thickness and color."),
                ];
                for a in crate::border::RATIO_CHOICES {
                    choices.push((crate::border::BorderFit::Ratio(a), "Adds border to reach this aspect ratio. The picture is not resized."));
                }
                for (fit, tip) in choices {
                    let sel = b.fit == fit;
                    let btn = egui::Button::new(egui::RichText::new(fit.label()).size(12.0).color(if sel {
                        Color32::WHITE
                    } else {
                        theme::TEXT
                    }))
                    .fill(if sel { theme::ACCENT } else { theme::PANEL })
                    .corner_radius(CornerRadius::same(12));
                    if ui.add(btn).on_hover_text(tip).clicked() {
                        if fit == crate::border::BorderFit::Polaroid && b.fit != fit {
                            // Polaroid also sets its cream color and thickness.
                            *b = crate::border::Border::polaroid();
                        }
                        b.fit = fit;
                    }
                }
            });
        });
        // Resulting output size.
        if let Some(sc) = &app.scene {
            let [_, _, cw, ch] = sc.crop.pixels(sc.full.width(), sc.full.height());
            let [ow, oh, _, _] = app.border.layout(cw as f32, ch as f32);
            let text = if app.border.enabled {
                format!("Output {:.0} × {:.0} px · ratio {}", ow, oh, ratio_text(ow, oh))
            } else {
                format!("Output {cw} × {ch} px, no border")
            };
            ui.label(egui::RichText::new(text).size(12.0).color(theme::TEXT_DIM));
        }
    });
}

/// "16:9" for common shapes, otherwise "1.47:1".
pub fn ratio_text(w: f32, h: f32) -> String {
    if h <= 0.0 {
        return "-".into();
    }
    let r = w / h;
    for (a, b) in [(1, 1), (4, 5), (5, 4), (3, 4), (4, 3), (2, 3), (3, 2), (9, 16), (16, 9), (21, 9)] {
        if (r - a as f32 / b as f32).abs() < 0.01 {
            return format!("{a}:{b}");
        }
    }
    if r >= 1.0 { format!("{r:.2}:1") } else { format!("1:{:.2}", 1.0 / r) }
}

// ----- Caption ---------------------------------------------------------------

/// Caption text, font (built-in, added or installed), size, color, place.
pub fn caption_card(app: &mut StudioApp, ui: &mut egui::Ui) {
    if app.scene.is_none() {
        return;
    }
    theme::card().show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal(|ui| {
            fold_title(ui, app, "caption", "Caption");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.checkbox(&mut app.caption.enabled, "Add caption");
            });
        });
        if !is_open(app, "caption") {
            return;
        }
        let enabled = app.caption.enabled;
        ui.add_enabled_ui(enabled, |ui| {
            ui.add(
                egui::TextEdit::multiline(&mut app.caption.text)
                    .desired_rows(2)
                    .desired_width(f32::INFINITY)
                    .hint_text(crate::caption::Caption::PLACEHOLDER),
            );
            font_picker(app, ui);
            let c = &mut app.caption;
            with_reset!(
                ui,
                c.size,
                crate::caption::DEFAULT_SIZE,
                egui::Slider::new(&mut c.size, 0.02..=0.25)
                    .text("Size")
                    .custom_formatter(|v, _| format!("{:.0}%", v * 100.0))
                    .custom_parser(|t| t.trim_end_matches('%').trim().parse::<f64>().ok().map(|v| v / 100.0))
            );
            with_reset!(
                ui,
                c.margin,
                crate::caption::DEFAULT_MARGIN,
                egui::Slider::new(&mut c.margin, 0.0..=0.4)
                    .text("Margin")
                    .custom_formatter(|v, _| format!("{:.0}%", v * 100.0))
                    .custom_parser(|t| t.trim_end_matches('%').trim().parse::<f64>().ok().map(|v| v / 100.0))
            );
            with_reset!(ui, c.opacity, 1.0, egui::Slider::new(&mut c.opacity, 0.1..=1.0).text("Opacity").fixed_decimals(2));
            ui.horizontal(|ui| {
                ui.label("Color");
                color_swatches(ui, &mut c.color, &app.palette);
                // Plain color picker; caption opacity has its own slider.
                ui.color_edit_button_srgb(&mut c.color).on_hover_text("Any color");
                let target = crate::app::PickTarget::Caption;
                let active = app.picking == Some(target);
                if eyedropper_button(ui, active).clicked() {
                    app.picking = if active { None } else { Some(target) };
                    app.status = "Click the picture to pick a color (Esc to cancel).".into();
                }
            });
            ui.checkbox(&mut c.shadow, "Text shadow");
            ui.horizontal(|ui| {
                ui.label("Place");
                use crate::caption::Place;
                for (p, name) in [(Place::Top, "Top"), (Place::Middle, "Middle"), (Place::Bottom, "Bottom")] {
                    if ui.selectable_label(c.place == p, name).clicked() {
                        c.place = p;
                    }
                }
            });
            ui.horizontal(|ui| {
                ui.label("Align");
                use crate::caption::Align;
                for (a, name) in [(Align::Left, "Left"), (Align::Center, "Center"), (Align::Right, "Right")] {
                    if ui.selectable_label(c.align == a, name).clicked() {
                        c.align = a;
                    }
                }
            });
        });
    });
}

/// Font list (built-in, added, installed) with search, plus adding a font file.
fn font_picker(app: &mut StudioApp, ui: &mut egui::Ui) {
    use crate::fonts::{FontEntry, FontSource};
    ui.horizontal(|ui| {
        ui.label("Font");
        let current = app.caption.font.name.clone();
        egui::ComboBox::from_id_salt("caption_font")
            .selected_text(current)
            .width(200.0)
            // Long font names are cut with "…" so the bar never grows.
            .truncate()
            .height(420.0)
            // Stay open while typing in the search box; close after a pick.
            .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
            .show_ui(ui, |ui| {
                ui.set_min_width(280.0);
                ui.add(egui::TextEdit::singleline(&mut app.font_filter).hint_text("Search fonts").desired_width(270.0));
                let filter = app.font_filter.to_lowercase();
                let matches = |f: &FontEntry| filter.is_empty() || f.name.to_lowercase().contains(&filter);
                let mut pick: Option<FontEntry> = None;
                let mut remove: Option<FontEntry> = None;

                // One list of rows: section titles and fonts.
                enum Row {
                    Title(&'static str),
                    Font(FontEntry, bool),
                }
                let mut rows = Vec::new();
                let builtin = FontEntry::builtin();
                if matches(&builtin) {
                    rows.push(Row::Font(builtin, false));
                }
                let added: Vec<FontEntry> = app.fonts_added.iter().filter(|f| matches(f)).cloned().collect();
                if !added.is_empty() {
                    rows.push(Row::Title("Added by you"));
                    rows.extend(added.into_iter().map(|f| Row::Font(f, true)));
                }
                let installed: Vec<FontEntry> = app.fonts_installed.iter().filter(|f| matches(f)).cloned().collect();
                if !installed.is_empty() {
                    rows.push(Row::Title("Installed on this computer"));
                    rows.extend(installed.into_iter().map(|f| Row::Font(f, false)));
                }

                let ppp = ui.ctx().pixels_per_point();
                let row_h = 32.0;
                let mut budget = 6; // previews drawn per frame, so scrolling stays smooth
                let mut waiting = false;
                egui::ScrollArea::vertical().max_height(330.0).auto_shrink([false, true]).show_rows(
                    ui,
                    row_h,
                    rows.len(),
                    |ui, range| {
                        for row in &rows[range] {
                            match row {
                                Row::Title(t) => {
                                    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), row_h), Sense::hover());
                                    ui.painter().text(
                                        rect.left_bottom() + Vec2::new(4.0, -6.0),
                                        egui::Align2::LEFT_BOTTOM,
                                        *t,
                                        egui::FontId::proportional(11.0),
                                        theme::TEXT_DIM,
                                    );
                                }
                                Row::Font(f, is_added) => {
                                    let key = f.path.as_ref().map(|p| p.to_string_lossy().to_string()).unwrap_or_default();
                                    if !app.font_previews.contains_key(&key) {
                                        if budget > 0 {
                                            budget -= 1;
                                            let tex = crate::fonts::render_sample(f, &f.name, 20.0 * ppp).map(|img| {
                                                let c = egui::ColorImage::from_rgba_unmultiplied(
                                                    [img.width() as usize, img.height() as usize],
                                                    img.as_raw(),
                                                );
                                                ui.ctx().load_texture(format!("fontprev:{key}"), c, egui::TextureOptions::LINEAR)
                                            });
                                            app.font_previews.insert(key.clone(), tex);
                                        } else {
                                            waiting = true;
                                        }
                                    }
                                    let (rect, resp) =
                                        ui.allocate_exact_size(Vec2::new(ui.available_width(), row_h - 2.0), Sense::click());
                                    let selected = app.caption.font == *f;
                                    if selected || resp.hovered() {
                                        ui.painter().rect_filled(
                                            rect,
                                            CornerRadius::same(6),
                                            if selected { theme::ACCENT_SOFT } else { theme::PANEL_RAISED },
                                        );
                                    }
                                    let text_right = if *is_added { rect.right() - 64.0 } else { rect.right() - 6.0 };
                                    let area = Rect::from_min_max(rect.min + Vec2::new(6.0, 0.0), Pos2::new(text_right, rect.max.y));
                                    match app.font_previews.get(&key).and_then(|t| t.as_ref()) {
                                        Some(tex) => {
                                            let [tw, th] = tex.size();
                                            let size = Vec2::new(tw as f32 / ppp, th as f32 / ppp);
                                            let r = Rect::from_min_size(Pos2::new(area.left(), area.center().y - size.y / 2.0), size);
                                            // Cut long names at the row edge.
                                            let shown = r.intersect(area);
                                            let uv = Rect::from_min_max(
                                                Pos2::ZERO,
                                                Pos2::new((shown.width() / size.x).min(1.0), 1.0),
                                            );
                                            let tint = if selected { Color32::WHITE } else { theme::TEXT };
                                            ui.painter().image(tex.id(), Rect::from_min_size(shown.min, Vec2::new(shown.width(), size.y)), uv, tint);
                                        }
                                        None => {
                                            ui.painter().with_clip_rect(area).text(
                                                Pos2::new(area.left(), area.center().y),
                                                egui::Align2::LEFT_CENTER,
                                                &f.name,
                                                egui::FontId::proportional(13.5),
                                                theme::TEXT,
                                            );
                                        }
                                    }
                                    let resp = resp.on_hover_text(&f.name);
                                    let mut removed = false;
                                    if *is_added {
                                        let btn_rect = Rect::from_min_max(Pos2::new(rect.right() - 60.0, rect.top() + 4.0), rect.max - Vec2::new(4.0, 4.0));
                                        removed = theme::overlay(ui, btn_rect, egui::Button::new(egui::RichText::new("Remove").size(11.0)))
                                            .on_hover_text("Delete this font from the app's font folder")
                                            .clicked();
                                        if removed {
                                            remove = Some(f.clone());
                                        }
                                    }
                                    if resp.clicked() && !removed {
                                        pick = Some(f.clone());
                                    }
                                }
                            }
                        }
                    },
                );
                if waiting {
                    ui.ctx().request_repaint();
                }
                if let Some(f) = pick {
                    app.caption.font = f;
                    ui.close();
                }
                if let Some(f) = remove {
                    match crate::fonts::remove_added_font(&f) {
                        Ok(()) => {
                            if app.caption.font == f {
                                app.caption.font = FontEntry::builtin();
                            }
                            app.fonts_added = crate::fonts::added_fonts();
                        }
                        Err(e) => app.status = e,
                    }
                }
            });
    });
    ui.horizontal(|ui| {
        if ui
            .add(icons::button(Icon::Plus, "Add font", theme::TEXT))
            .on_hover_text("Add a .ttf, .otf or .ttc file. It is copied into the app and kept for next time.")
            .clicked()
        {
            if let Some(path) = rfd::FileDialog::new()
                .set_title("Add a font")
                .add_filter("Fonts", &["ttf", "otf", "ttc"])
                .pick_file()
            {
                match crate::fonts::add_font(&path) {
                    Ok(entry) => {
                        app.fonts_added = crate::fonts::added_fonts();
                        app.status = format!("Added font {}. Saved in {}", entry.name, crate::fonts::fonts_dir().display());
                        app.caption.font = entry;
                    }
                    Err(e) => app.error = Some(e),
                }
            }
        }
        if ui
            .small_button("Fonts folder")
            .on_hover_text(format!("Added fonts are kept in {}", crate::fonts::fonts_dir().display()))
            .clicked()
        {
            let dir = crate::fonts::fonts_dir();
            let _ = std::fs::create_dir_all(&dir);
            crate::export::reveal(&dir);
        }
    });
    let source = match app.caption.font.source {
        FontSource::BuiltIn => "Built-in font",
        FontSource::Added => "Added font, kept in the app",
        FontSource::Installed => "Installed on this computer",
    };
    ui.label(egui::RichText::new(source).size(11.0).color(theme::TEXT_DIM));
}

/// Eyedropper toggle shown next to a color picker.
fn eyedropper_button(ui: &mut egui::Ui, active: bool) -> egui::Response {
    let tint = if active { Color32::WHITE } else { theme::TEXT };
    let b = egui::Button::image(Icon::Eyedropper.image(15.0, tint))
        .fill(if active { theme::ACCENT } else { theme::PANEL })
        .min_size(Vec2::splat(24.0));
    ui.add(b).on_hover_text(if active {
        "Picking: click the picture (Esc to cancel)"
    } else {
        "Pick a color from the picture"
    })
}

/// Five color swatches: white, the photo's three main colors, black.
fn color_swatches(ui: &mut egui::Ui, current: &mut [u8; 3], palette: &[[u8; 3]]) {
    ui.spacing_mut().item_spacing.x = 6.0;
    let mut swatches: Vec<([u8; 3], String)> = vec![([255, 255, 255], "White".into())];
    for i in 0..3 {
        // Mid grays stand in until the photo's colors are known.
        let c = palette.get(i).copied().unwrap_or([96 + 32 * i as u8; 3]);
        swatches.push((c, format!("From the photo #{:02X}{:02X}{:02X}", c[0], c[1], c[2])));
    }
    swatches.push(([0, 0, 0], "Black".into()));
    for (c, name) in swatches {
        let sel = *current == c;
        let (rect, resp) = ui.allocate_exact_size(Vec2::splat(20.0), Sense::click());
        ui.painter().rect_filled(rect, CornerRadius::same(5), Color32::from_rgb(c[0], c[1], c[2]));
        ui.painter().rect_stroke(
            rect,
            CornerRadius::same(5),
            Stroke::new(if sel { 2.0 } else { 1.0 }, if sel { theme::ACCENT } else { theme::BORDER }),
            egui::StrokeKind::Outside,
        );
        if resp.on_hover_text(name).clicked() {
            *current = c;
        }
    }
}
