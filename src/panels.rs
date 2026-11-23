//! Top bar, right-hand settings panel and the results strip.

use eframe::egui::{self, Color32, CornerRadius, Pos2, Rect, Sense, Stroke, Vec2};

use crate::app::{StudioApp, Tool, View};
use crate::export::Format;
use crate::icons::{self, Icon};
use crate::theme;
use crate::app::defaults;

/// Clickable card title with a chevron; clicking folds or unfolds the card.
pub(crate) fn fold_title(ui: &mut egui::Ui, app: &mut StudioApp, key: &str, title: &str) {
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

pub(crate) fn is_open(app: &StudioApp, key: &str) -> bool {
    !app.prefs.collapsed.contains(key)
}

/// Every panel the View menu can show or hide: (key, name).
pub const PANELS: [(&str, &str); 19] = [
    ("scope", "Vectorscope"),
    ("region", "Moving region / selected area"),
    ("motion", "Motion"),
    ("loop", "Loop"),
    ("video", "Video"),
    ("crop", "Rotate, straighten & crop"),
    ("border", "Border"),
    ("caption", "Caption"),
    ("look", "Look"),
    ("lr", "Lightroom presets"),
    ("film", "Film Simulation"),
    ("adj_light", "Light"),
    ("adj_color", "Color"),
    ("adj_effects", "Effects"),
    ("adj_curve", "Tone curve"),
    ("adj_hsl", "HSL / Color"),
    ("adj_grade", "Color grading"),
    ("masks", "Masks"),
    ("export", "Export"),
];

/// Small "back to default" button shown after a setting; grayed out when
/// the setting already has its default value.
pub(crate) fn reset_icon(ui: &mut egui::Ui, is_default: bool) -> bool {
    let b = egui::Button::image(Icon::Reset.image(13.0, theme::TEXT_DIM))
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
        // Layout: several finished photos on one picture.
        let can_layout = app.layout.is_some() || app.files.len() >= 2;
        let on = app.layout_open;
        let layout_btn = icons::button(
            Icon::Layout,
            egui::RichText::new("Layout").size(13.5).color(if on { Color32::WHITE } else { theme::TEXT }),
            if on { Color32::WHITE } else { theme::TEXT },
        )
        .fill(if on { theme::ACCENT } else { Color32::TRANSPARENT });
        if ui
            .add_enabled(can_layout, layout_btn)
            .on_hover_text(if on {
                "Back to editing single files (the layout is kept)"
            } else {
                "Place several finished photos on one picture"
            })
            .on_disabled_hover_text("Open at least two files to make a layout")
            .clicked()
        {
            if on {
                app.close_layout();
            } else if app.layout.is_some() {
                app.open_layout();
            } else {
                app.new_layout();
            }
        }
        ui.add_space(4.0);
        ui.add(Icon::Logo.image(26.0, Color32::WHITE));
        ui.label(egui::RichText::new("Animagraph").size(15.0).strong());
        if let Some(s) = &app.scene {
            ui.label(egui::RichText::new(format!("·  {}", s.name)).color(theme::TEXT_DIM));
        }

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            // Fold the right-hand settings bar (mirror of the Files panel button).
            let controls_on = app.panel_visible("controls");
            let controls_btn =
                egui::Button::image(Icon::SidebarRight.image(18.0, if controls_on { Color32::WHITE } else { theme::TEXT_DIM }))
                    .fill(if controls_on { theme::ACCENT_SOFT } else { Color32::TRANSPARENT })
                    .min_size(Vec2::splat(30.0));
            if ui.add(controls_btn).on_hover_text("Show or hide the settings bar on the right").clicked() {
                app.set_panel_visible("controls", !controls_on);
            }
            let can_export = if app.layout_open { app.layout.is_some() && !app.busy() } else { app.can_export() };
            let several = if app.layout_open { None } else { crate::export_panel::selected_to_save(app) };
            let words = several.map_or("Export".to_string(), |n| format!("Export {}", crate::batch_export::photos(n)));
            let hint = crate::export_panel::export_hint(app, several);
            let export = icons::button(Icon::Download, egui::RichText::new(words).color(Color32::WHITE).strong(), Color32::WHITE)
                .fill(if can_export { theme::ACCENT } else { theme::ACCENT_SOFT })
                .min_size(Vec2::new(96.0, 32.0));
            if ui
                .add_enabled(can_export, export)
                .on_hover_text(hint)
                .on_disabled_hover_text("Animate a loop first, or choose a still image format in Export")
                .clicked()
            {
                if app.layout_open {
                    app.export_layout();
                } else {
                    app.start_export();
                }
            }
            if ui
                .add(icons::button(Icon::Reset, "Reset", theme::TEXT).min_size(Vec2::new(0.0, 32.0)))
                .on_hover_text(
                    "Speed, lengths, look, film, border and other settings go back to their defaults. \
                     Painted areas, arrows and crops are kept.",
                )
                .clicked()
            {
                app.reset_settings();
            }
            // Undo and redo (laid out right to left, so redo comes first).
            let can_undo = app.scene.as_ref().is_some_and(|s| s.can_undo());
            let can_redo = app.scene.as_ref().is_some_and(|s| s.can_redo());
            let top_icon = |icon: Icon| {
                egui::Button::image(icon.image(16.0, theme::TEXT)).min_size(Vec2::new(36.0, 32.0))
            };
            if ui
                .add_enabled(can_redo, top_icon(Icon::Redo))
                .on_hover_text("Redo (Cmd/Ctrl+Shift+Z)")
                .clicked()
            {
                let ctx = ui.ctx().clone();
                app.redo(&ctx);
            }
            if ui
                .add_enabled(can_undo, top_icon(Icon::Undo))
                .on_hover_text("Undo (Cmd/Ctrl+Z): painting, arrows, pins, crop, rotation and colour changes (Edit, curves, presets, film)")
                .clicked()
            {
                let ctx = ui.ctx().clone();
                app.undo(&ctx);
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

fn view_toggle(ui: &mut egui::Ui, app: &mut StudioApp, key: &str, name: &str) {
    let mut on = app.panel_visible(key);
    if ui.checkbox(&mut on, name).changed() {
        app.set_panel_visible(key, on);
    }
}

/// View menu: show or hide each panel, fold or unfold them all.
fn view_menu(app: &mut StudioApp, ui: &mut egui::Ui) {
    // Stay open while toggling items; close on a click outside the menu or Esc.
    let config = egui::containers::menu::MenuConfig::new()
        .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside);
    egui::containers::menu::MenuButton::new(egui::RichText::new("View").size(13.5)).config(config).ui(ui, |ui| {
        ui.set_min_width(230.0);
        view_toggle(ui, app, "files", "Files panel");
        view_toggle(ui, app, "results", "Results strip");
        view_toggle(ui, app, "vectorscope", "Vectorscope");
        view_toggle(ui, app, "controls", "Settings bar (right)");
        ui.separator();
        ui.label(egui::RichText::new("Settings bar sections").size(11.0).color(theme::TEXT_DIM));
        view_toggle(ui, app, "edit", "Edit");
        view_toggle(ui, app, "framing", "Framing");
        view_toggle(ui, app, "styles", "Styles");
        view_toggle(ui, app, "animate", "Animate");
        view_toggle(ui, app, "export", "Export");
        ui.separator();
        still_view_choice(ui, app);
        ui.separator();
        if ui
            .button("Assistant access (MCP)…")
            .on_hover_text("Let an assistant such as Claude edit and save photos with Animagraph")
            .clicked()
        {
            app.mcp_window_open = true;
        }
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

/// How sharp the still photo looks on the canvas while editing. Exports
/// always use the size chosen in Export.
fn still_view_choice(ui: &mut egui::Ui, app: &mut StudioApp) {
    ui.label(egui::RichText::new("Still photo quality").size(11.0).color(theme::TEXT_DIM));
    for (size, label) in crate::settings::STILL_VIEW_CHOICES {
        ui.radio_value(&mut app.prefs.still_view, size, label).on_hover_text(if size == crate::scene::WORK_SIZE {
            "Quickest to update. Can look soft on a large screen."
        } else {
            "Sharper on screen. While you drag a slider or hover a style it updates at 720 px, \
             then sharpens when you let go."
        });
    }
    let shown = app.still_view_pixels().map(|[w, h]| format!("Shown now at {w} × {h} px. Exports use the Export size."));
    ui.label(
        egui::RichText::new(shown.unwrap_or_else(|| "Exports use the Export size.".into()))
            .size(11.0)
            .color(theme::TEXT_DIM),
    );
}

// ----- Side panel ----------------------------------------------------------

pub fn side_panel(app: &mut StudioApp, ui: &mut egui::Ui) {
    if app.layout_open {
        crate::layout_view::side_panel(app, ui);
        return;
    }
    // The vectorscope stays at the top while the drawers scroll.
    if app.panel_visible("vectorscope") && app.scene.is_some() {
        ui.scope(|ui| {
            ui.set_max_width(ui.available_width().min(312.0));
            vectorscope_card(app, ui);
        });
        ui.add_space(10.0);
    }
    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        ui.spacing_mut().item_spacing.y = 10.0;
        // Keep everything inside the bar's width.
        ui.set_max_width(ui.available_width().min(312.0));
        let is_video = app.scene.as_ref().is_some_and(|s| s.is_video());

        // Edit: light and colour.
        if app.panel_visible("edit") {
            drawer(
                app,
                ui,
                "drawer_edit",
                Icon::Image,
                "Edit",
                "Adjust light, color, curves and effects. Applies to saved images, loops and videos.",
                |app, ui| {
                    let n = app.apply_targets().len();
                    let b = icons::button(Icon::Image, format!("Apply edits to {}", app.apply_target_words()), theme::TEXT);
                    if ui
                        .add_enabled(n > 0, b)
                        .on_hover_text(
                            "Copies this file's colour edits (light, colour, curves, effects, presets and film) to the \
                             other open files, or only to the files selected in the Files panel. Framing is copied \
                             with its own button in Framing.",
                        )
                        .on_disabled_hover_text(
                            "Open more than one file to use this. Cmd/Ctrl-click files in the Files panel to pick some.",
                        )
                        .clicked()
                    {
                        app.apply_edits_to_targets();
                    }
                    crate::panels::adjust_card(app, ui);
                    crate::mask_panel::masks_card(app, ui);
                },
            );
        }

        // Framing: rotation, straightening, crop, border and caption.
        if app.panel_visible("framing") {
            drawer(
                app,
                ui,
                "drawer_framing",
                Icon::Crop,
                "Framing",
                "Rotate, straighten and crop, then add a border or caption.",
                |app, ui| {
                    if app.scene.is_none() {
                        ui.label(
                            egui::RichText::new("Open a photo or video to use these settings.")
                                .size(12.0)
                                .color(theme::TEXT_DIM),
                        );
                    }
                    let n = app.apply_targets().len();
                    let b = icons::button(Icon::Crop, format!("Apply framing to {}", app.apply_target_words()), theme::TEXT);
                    if ui
                        .add_enabled(n > 0, b)
                        .on_hover_text(
                            "Copies this file's crop, straightening, border and caption to the other open files, or \
                             only to the files selected in the Files panel. Quarter turns are not copied. A crop with \
                             a fixed shape keeps that shape on pictures of another size.",
                        )
                        .on_disabled_hover_text(
                            "Open more than one file to use this. Cmd/Ctrl-click files in the Files panel to pick some.",
                        )
                        .clicked()
                    {
                        app.apply_framing_to_targets();
                    }
                    crate::panels::crop_card(app, ui);
                    crate::panels::border_card(app, ui);
                    crate::panels::caption_card(app, ui);
                },
            );
        }

        // Styles: looks, Lightroom presets and film simulations, as previews.
        if app.panel_visible("styles") {
            drawer(
                app,
                ui,
                "drawer_styles",
                Icon::Film,
                "Styles",
                "Hover a tile to preview it on the picture, click to apply. Fine-tune it in Edit.",
                |app, ui| {
                    crate::panels::look_card(app, ui);
                    crate::panels::lightroom_card(app, ui);
                    crate::panels::film_card(app, ui);
                },
            );
        }

        // Animate: what moves, how, and the button that makes the loop.
        let (title, intro) = if is_video {
            ("Freeze", "Select an area, choose whether it freezes or plays, then press Make loop.")
        } else {
            ("Animate", "Paint the area to animate, draw arrows in the direction of motion, then press Animate.")
        };
        if app.panel_visible("animate") {
            drawer(app, ui, "drawer_animate", Icon::Sparkle, title, intro, |app, ui| {
                region_card(app, ui);
                if is_video {
                    crate::panels::video_card(app, ui);
                } else {
                    motion_card(app, ui);
                    loop_card(app, ui);
                }
                generate_block(app, ui);
            });
        }

        // Export: saving.
        if app.panel_visible("export") {
            drawer(
                app,
                ui,
                "drawer_export",
                Icon::Download,
                "Export",
                "Save the selected loop, or a still image.",
                |app, ui| export_card(app, ui),
            );
        }

        ui.add_space(8.0);
    });
}

/// Vectorscope of the picture on the canvas: hue by angle, saturation by
/// distance from the centre, with the 75% colour targets and the skin line.
fn vectorscope_card(app: &mut StudioApp, ui: &mut egui::Ui) {
    theme::card().show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal(|ui| {
            fold_title(ui, app, "scope", "Vectorscope");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                for z in [2.0f32, 1.0] {
                    let sel = app.scope_zoom == z;
                    if ui
                        .selectable_label(sel, egui::RichText::new(format!("{z:.0}×")).size(12.0))
                        .on_hover_text(if z > 1.0 {
                            "Enlarge the middle, for pictures with soft colours"
                        } else {
                            "Whole range: the circle's edge is full saturation"
                        })
                        .clicked()
                    {
                        app.scope_zoom = z;
                    }
                }
            });
        });
        if !is_open(app, "scope") {
            return;
        }
        let ctx = ui.ctx().clone();
        app.refresh_scope(&ctx);
        let side = ui.available_width().min(200.0);
        let (rect, resp) = ui.allocate_exact_size(Vec2::new(ui.available_width(), side), Sense::hover());
        let rect = Rect::from_center_size(rect.center(), Vec2::splat(side));
        let painter = ui.painter_at(rect.expand(2.0));
        let to_screen = |p: [f32; 2]| {
            let s = side / crate::scope::SIZE as f32;
            rect.min + Vec2::new(p[0] * s, p[1] * s)
        };
        let zoom = app.scope_zoom;
        let center = rect.center();
        let radius = side / 2.0 - 4.0 * side / crate::scope::SIZE as f32;
        // Graticule.
        painter.circle_filled(center, radius, Color32::from_rgb(14, 14, 18));
        painter.circle_stroke(center, radius, Stroke::new(1.0, Color32::from_white_alpha(50)));
        painter.circle_stroke(center, radius * 0.5, Stroke::new(1.0, Color32::from_white_alpha(22)));
        let faint = Stroke::new(1.0, Color32::from_white_alpha(22));
        painter.line_segment([center - Vec2::new(radius, 0.0), center + Vec2::new(radius, 0.0)], faint);
        painter.line_segment([center - Vec2::new(0.0, radius), center + Vec2::new(0.0, radius)], faint);
        let [sx, sy] = crate::scope::skin_direction();
        painter.line_segment(
            [center, center + Vec2::new(sx, -sy) * radius],
            Stroke::new(1.0, Color32::from_rgba_unmultiplied(230, 170, 130, 110)),
        );
        // Trace.
        if let Some(t) = &app.scope_tex {
            painter.image(t.id(), rect, Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), Color32::WHITE);
        }
        // Colour targets (75%).
        for (label, c) in crate::scope::targets() {
            let p = to_screen(crate::scope::to_scope(c, zoom));
            if !rect.contains(p) || (p - center).length() > radius + 1.0 {
                continue;
            }
            let box_r = Rect::from_center_size(p, Vec2::splat(9.0));
            painter.rect_stroke(box_r, CornerRadius::ZERO, Stroke::new(1.0, Color32::from_white_alpha(120)), egui::StrokeKind::Middle);
            let dir = (p - center).normalized();
            painter.text(
                p + dir * 12.0,
                egui::Align2::CENTER_CENTER,
                label,
                egui::FontId::proportional(9.5),
                Color32::from_white_alpha(150),
            );
        }
        resp.on_hover_text(
            "Each dot is a colour in the picture: the angle is the hue, the distance from the centre \
             the saturation. Boxes mark strong red, magenta, blue, cyan, green and yellow; the orange \
             line is where skin tones fall. Grey and white sit in the middle.",
        );
    });
}

/// A category drawer: a large clickable header that folds its contents.
fn drawer(
    app: &mut StudioApp,
    ui: &mut egui::Ui,
    key: &str,
    icon: Icon,
    title: &str,
    intro: &str,
    body: impl FnOnce(&mut StudioApp, &mut egui::Ui),
) {
    let open = !app.prefs.collapsed.contains(key);
    let width = ui.available_width();
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(width, 40.0), Sense::click());
    let hovered = resp.hovered();
    let painter = ui.painter();
    painter.rect_filled(
        rect,
        CornerRadius::same(10),
        if hovered { Color32::from_rgb(40, 40, 50) } else { Color32::from_rgb(34, 34, 43) },
    );
    painter.rect_stroke(rect, CornerRadius::same(10), Stroke::new(1.0, theme::BORDER), egui::StrokeKind::Inside);
    let chevron = if open { Icon::ChevronDown } else { Icon::ChevronRight };
    let c = rect.left_center();
    chevron.image(14.0, theme::TEXT_DIM).paint_at(ui, Rect::from_center_size(c + Vec2::new(16.0, 0.0), Vec2::splat(14.0)));
    icon.image(18.0, theme::ACCENT).paint_at(ui, Rect::from_center_size(c + Vec2::new(40.0, 0.0), Vec2::splat(18.0)));
    ui.painter().text(
        c + Vec2::new(58.0, 0.0),
        egui::Align2::LEFT_CENTER,
        title,
        egui::FontId::proportional(15.5),
        theme::TEXT,
    );
    if resp.on_hover_text(if open { "Fold" } else { "Unfold" }).clicked() {
        if open {
            app.prefs.collapsed.insert(key.to_string());
        } else {
            app.prefs.collapsed.remove(key);
        }
    }
    if open {
        ui.label(egui::RichText::new(intro).size(12.5).color(theme::TEXT_DIM));
        body(app, ui);
        ui.add_space(6.0);
    }
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
            // Separate buttons: arrows and pins are often cleared on their own.
            let (n_arrows, n_pins) = app.scene.as_ref().map_or((0, 0), |s| (s.arrows.len(), s.pins.len()));
            ui.horizontal(|ui| {
                let b = icons::button(Icon::Trash, "Remove arrows", theme::TEXT);
                if ui.add_enabled(n_arrows > 0, b).on_hover_text("Remove every motion arrow (undo with Cmd+Z)").clicked() {
                    if let Some(s) = &mut app.scene {
                        s.checkpoint();
                        s.arrows.clear();
                    }
                }
                let b = icons::button(Icon::Trash, "Remove pins", theme::TEXT);
                if ui.add_enabled(n_pins > 0, b).on_hover_text("Remove every pin (undo with Cmd+Z)").clicked() {
                    if let Some(s) = &mut app.scene {
                        s.checkpoint();
                        s.pins.clear();
                    }
                }
            });
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
        ui.horizontal(|ui| {
            ui.label("Format");
            egui::ComboBox::from_id_salt("format")
                .selected_text(app.export_format.label())
                .width(150.0)
                .show_ui(ui, |ui| {
                    ui.label(egui::RichText::new("STILL IMAGE").size(11.0).strong().color(theme::TEXT_DIM));
                    for f in [Format::Png, Format::Jpeg, Format::Heif] {
                        ui.selectable_value(&mut app.export_format, f, f.label())
                            .on_hover_text("Single still image with look, crop, caption and border");
                    }
                    ui.separator();
                    ui.label(egui::RichText::new("ANIMATION").size(11.0).strong().color(theme::TEXT_DIM));
                    ui.selectable_value(&mut app.export_format, Format::Gif, Format::Gif.label());
                    ui.add_enabled_ui(app.ffmpeg, |ui| {
                        for f in [Format::Mp4, Format::Mov] {
                            ui.selectable_value(&mut app.export_format, f, f.label())
                                .on_disabled_hover_text("Saving video needs ffmpeg on this computer");
                        }
                    });
                    ui.selectable_value(&mut app.export_format, Format::PngFrames, Format::PngFrames.label());
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
        crate::export_panel::size_picker(app, ui);
        let can = app.can_export();
        let still = app.export_format.is_still();
        let several = crate::export_panel::selected_to_save(app);
        let words = match several {
            Some(n) => format!("Save {}…", crate::batch_export::photos(n)),
            None if still => "Save image…".into(),
            None => "Save selected loop…".into(),
        };
        ui.horizontal(|ui| {
            if ui
                .add_enabled(can, egui::Button::new(words))
                .on_hover_text(crate::export_panel::export_hint(app, several))
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
        if still {
            crate::export_panel::save_all(app, ui);
        }
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

        // Apple Log: convert to normal colours, or keep the flat log picture.
        if video.apple_log {
            let mut convert = video.log_converted;
            if ui
                .checkbox(&mut convert, "Convert Apple Log to normal colours")
                .on_hover_text(
                    "This clip was shot in Apple Log, which looks flat and grey as shot. \
                     On: shown and saved in normal colours. Off: kept as shot, for your own grading \
                     with Edit or a preset. Your choice is used for Log clips you open later too.",
                )
                .changed()
            {
                let ctx = ui.ctx().clone();
                app.set_log_conversion(&ctx, convert);
            }
        }

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
            .add(egui::Slider::new(&mut still, 0..=n - 1).text("Frozen frame").custom_formatter(move |v, _| format!("{:.2}s", v as f32 / fps)))
            .on_hover_text("The single frame of the video that the still parts of the picture show");
        if r.changed() {
            let ctx = ui.ctx().clone();
            app.set_still(&ctx, still);
            app.view = View::Edit;
        }

        with_reset!(ui, app.video_speed, defaults::VIDEO_SPEED, egui::Slider::new(&mut app.video_speed, 0.25..=2.0).text("Speed").suffix("×"));
        ui.checkbox(&mut app.stabilize, "Stabilize handheld clip")
            .on_hover_text("Aligns each frame to the frozen frame and zooms in slightly to hide the edges. Applied when the loop is made.");

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
    theme::card().show(ui, |ui| {
        ui.set_width(ui.available_width());
        fold_title(ui, app, "look", "Look");
        if !is_open(app, "look") {
            return;
        }
        let tiles: Vec<Tile> = crate::finish::Look::presets()
            .into_iter()
            .map(|(name, look)| Tile {
                key: format!("look:{name}"),
                name: name.to_string(),
                note: String::new(),
                look,
                thumb_look: look,
                removable: false,
            })
            .collect();
        if let Some(TileAction::Apply(i)) = tile_grid(app, ui, &tiles) {
            app.look = tiles[i].look;
        }
    });
}

/// Imported Lightroom presets as preview tiles, with Import.
pub fn lightroom_card(app: &mut StudioApp, ui: &mut egui::Ui) {
    theme::card().show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal(|ui| {
            fold_title(ui, app, "lr", "Lightroom presets");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .add(icons::button(Icon::Plus, egui::RichText::new("Import").size(12.0), theme::TEXT))
                    .on_hover_text("Import Lightroom presets: .xmp files, or .dng preset files from Lightroom mobile. They are kept in the app.")
                    .clicked()
                {
                    import_presets(app);
                }
            });
        });
        if !is_open(app, "lr") {
            return;
        }
        if app.lr_presets.is_empty() {
            ui.label(
                egui::RichText::new("No presets yet. Import .xmp or .dng preset files; they are kept in the app.")
                    .size(12.0)
                    .color(theme::TEXT_DIM),
            );
            return;
        }
        let tiles: Vec<Tile> = app
            .lr_presets
            .iter()
            .map(|p| Tile {
                key: format!("lr:{}", p.path.display()),
                name: p.name.clone(),
                note: "Delete with the bin button or by right-clicking".into(),
                look: p.look,
                thumb_look: p.look,
                removable: true,
            })
            .collect();
        match tile_grid(app, ui, &tiles) {
            Some(TileAction::Apply(i)) => {
                app.lr_choice = Some(i);
                app.look = tiles[i].look;
                app.status = format!("Applied the {} preset. Fine-tune it in Edit.", tiles[i].name);
            }
            // Asks first (see preset_dialog).
            Some(TileAction::Remove(i)) => app.preset_to_delete = app.lr_presets.get(i).cloned(),
            None => {}
        }
    });
}

fn import_presets(app: &mut StudioApp) {
    let Some(files) = rfd::FileDialog::new()
        .set_title("Import Lightroom presets")
        .add_filter("Lightroom presets", &["xmp", "dng"])
        .pick_files()
    else {
        return;
    };
    let mut ok = 0;
    let mut failed = Vec::new();
    for f in files {
        match crate::presets::import(&f) {
            Ok(_) => ok += 1,
            Err(e) => failed.push(e),
        }
    }
    app.lr_presets = crate::presets::list();
    app.status = format!("Imported {ok} preset{}.", if ok == 1 { "" } else { "s" });
    if !failed.is_empty() {
        app.error = Some(failed.join("\n"));
    }
}

// ----- Preview tiles ----------------------------------------------------------

/// One preview tile: a style shown on the current picture.
struct Tile {
    key: String,
    name: String,
    note: String,
    /// What a click applies (and hover previews).
    look: crate::finish::Look,
    /// What the thumbnail shows.
    thumb_look: crate::finish::Look,
    removable: bool,
}

enum TileAction {
    Apply(usize),
    Remove(usize),
}

/// Two columns of tiles. Thumbnails are made a few per frame and kept until
/// the picture changes. Hovering previews the style on the canvas.
fn tile_grid(app: &mut StudioApp, ui: &mut egui::Ui, tiles: &[Tile]) -> Option<TileAction> {
    let Some(scene) = &app.scene else {
        ui.label(egui::RichText::new("Open a photo or video to see previews.").size(12.0).color(theme::TEXT_DIM));
        return None;
    };
    let [iw, ih] = scene.work_size();
    let scene_key = format!("{}:{iw}x{ih}:{}", scene.id, scene.still_index);
    if app.style_thumbs_scene != scene_key {
        app.style_thumbs.clear();
        app.style_thumb_src = Some(crate::scene::downscale(&scene.work, 220));
        app.style_thumbs_scene = scene_key;
    }
    let mut budget = 6;
    let mut waiting = false;
    for t in tiles {
        if app.style_thumbs.contains_key(&t.key) {
            continue;
        }
        if budget == 0 {
            waiting = true;
            break;
        }
        budget -= 1;
        if let Some(src) = &app.style_thumb_src {
            let mut img = src.clone();
            crate::finish::apply_look(&mut img, &t.thumb_look);
            let c = egui::ColorImage::from_rgba_unmultiplied([img.width() as usize, img.height() as usize], img.as_raw());
            let tex = ui.ctx().load_texture(format!("style:{}", t.key), c, egui::TextureOptions::LINEAR);
            app.style_thumbs.insert(t.key.clone(), tex);
        }
    }
    if waiting {
        ui.ctx().request_repaint();
    }

    let gap = 8.0;
    let tile_w = ((ui.available_width() - gap) / 2.0).floor();
    let img_h = (tile_w * 0.62).round();
    let mut action = None;
    for (row, pair) in tiles.chunks(2).enumerate() {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = gap;
            for (col, t) in pair.iter().enumerate() {
                let i = row * 2 + col;
                let (rect, resp) = ui.allocate_exact_size(Vec2::new(tile_w, img_h + 22.0), Sense::click());
                let selected = app.look == t.look;
                let img_rect = Rect::from_min_size(rect.min, Vec2::new(tile_w, img_h));
                ui.painter().rect_filled(img_rect, CornerRadius::same(6), theme::BG);
                if let Some(tex) = app.style_thumbs.get(&t.key) {
                    // Fit inside the frame without stretching; bars fill the rest.
                    let [tw, th] = tex.size();
                    let k = (img_rect.width() / tw as f32).min(img_rect.height() / th as f32);
                    let fitted = Rect::from_center_size(img_rect.center(), Vec2::new(tw as f32 * k, th as f32 * k));
                    ui.painter().image(tex.id(), fitted, Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), Color32::WHITE);
                }
                ui.painter().rect_stroke(
                    img_rect,
                    CornerRadius::same(6),
                    Stroke::new(
                        if selected { 2.5 } else { 1.0 },
                        if selected {
                            theme::ACCENT
                        } else if resp.hovered() {
                            theme::TEXT_DIM
                        } else {
                            theme::BORDER
                        },
                    ),
                    egui::StrokeKind::Inside,
                );
                // Name, cut to the tile width.
                let mut job = egui::text::LayoutJob::simple_singleline(
                    t.name.clone(),
                    egui::FontId::proportional(12.5),
                    if selected { Color32::WHITE } else { theme::TEXT },
                );
                job.wrap = egui::text::TextWrapping::truncate_at_width(tile_w - 4.0);
                let galley = ui.painter().layout_job(job);
                ui.painter().galley(Pos2::new(rect.left() + 2.0, rect.bottom() - 18.0), galley, theme::TEXT);

                if resp.hovered() {
                    app.hover_look = Some((t.look, t.name.clone()));
                }
                // Bin button on imported presets, shown on hover.
                let mut bin_clicked = false;
                if t.removable {
                    let btn_rect = Rect::from_min_size(img_rect.right_top() + Vec2::new(-30.0, 4.0), Vec2::splat(26.0));
                    if resp.hovered() || ui.rect_contains_pointer(btn_rect) {
                        let btn = egui::Button::image(Icon::Trash.image(14.0, Color32::WHITE))
                            .fill(Color32::from_black_alpha(170))
                            .stroke(Stroke::NONE)
                            .corner_radius(CornerRadius::same(6));
                        bin_clicked = theme::overlay(ui, btn_rect, btn).on_hover_text("Delete this preset…").clicked();
                    }
                }
                let tip = if t.note.is_empty() { t.name.clone() } else { format!("{}\n{}", t.name, t.note) };
                let resp = resp.on_hover_text(tip);
                if bin_clicked {
                    action = Some(TileAction::Remove(i));
                } else if resp.clicked() {
                    action = Some(TileAction::Apply(i));
                }
                if t.removable {
                    resp.context_menu(|ui| {
                        if ui.button("Delete preset…").clicked() {
                            action = Some(TileAction::Remove(i));
                            ui.close();
                        }
                    });
                }
            }
        });
    }
    action
}

/// The fine adjustment sliders (light, color, tone curve, grain).
/// The adjustment controls, each section its own folding card in Edit.
pub fn adjust_card(app: &mut StudioApp, ui: &mut egui::Ui) {
    edit_section(app, ui, "adj_light", "Light", &mut |app: &mut StudioApp, ui: &mut egui::Ui| {
        let l = &mut app.look;
        with_reset!(ui, l.exposure, 0.0, egui::Slider::new(&mut l.exposure, -2.0..=2.0).text("Exposure").fixed_decimals(2));
        with_reset!(ui, l.contrast, 0.0, pct(&mut l.contrast, -0.5, 0.5, "Contrast", 200.0));
        with_reset!(ui, l.highlights, 0.0, pct(&mut l.highlights, -1.0, 1.0, "Highlights", 100.0));
        with_reset!(ui, l.shadows, 0.0, pct(&mut l.shadows, -1.0, 1.0, "Shadows", 100.0));
        with_reset!(ui, l.whites, 0.0, pct(&mut l.whites, -1.0, 1.0, "Whites", 100.0));
        with_reset!(ui, l.blacks, 0.0, pct(&mut l.blacks, -1.0, 1.0, "Blacks", 100.0));
    });

    edit_section(app, ui, "adj_color", "Color", &mut |app: &mut StudioApp, ui: &mut egui::Ui| {
        let l = &mut app.look;
        with_reset!(ui, l.warmth, 0.0, pct(&mut l.warmth, -1.0, 1.0, "Temperature", 100.0));
        with_reset!(ui, l.tint, 0.0, pct(&mut l.tint, -1.0, 1.0, "Tint", 100.0));
        with_reset!(ui, l.vibrance, 0.0, pct(&mut l.vibrance, -1.0, 1.0, "Vibrance", 100.0));
        with_reset!(ui, l.saturation, 0.0, pct(&mut l.saturation, -1.0, 1.0, "Saturation", 100.0));
    });

    edit_section(app, ui, "adj_effects", "Effects", &mut |app: &mut StudioApp, ui: &mut egui::Ui| {
        use crate::finish::{DEFAULT_GRAIN_ROUGH, DEFAULT_GRAIN_SIZE, DEFAULT_VIGNETTE_FEATHER, DEFAULT_VIGNETTE_MID};
        let l = &mut app.look;
        with_reset!(ui, l.texture, 0.0, pct(&mut l.texture, -1.0, 1.0, "Texture", 100.0));
        with_reset!(ui, l.clarity, 0.0, pct(&mut l.clarity, -1.0, 1.0, "Clarity", 100.0));
        with_reset!(ui, l.dehaze, 0.0, pct(&mut l.dehaze, -1.0, 1.0, "Dehaze", 100.0));
        with_reset!(ui, l.sharpen, 0.0, pct(&mut l.sharpen, 0.0, 1.0, "Sharpening", 100.0));
        with_reset!(ui, l.fade, 0.0, pct(&mut l.fade, 0.0, 1.0, "Fade", 100.0));

        sub_heading(ui, "Vignette");
        with_reset!(ui, l.vignette, 0.0, pct(&mut l.vignette, -1.0, 1.0, "Amount", 100.0));
        ui.label(egui::RichText::new("Positive darkens the edges, negative lightens them.").size(11.0).color(theme::TEXT_DIM));
        ui.add_enabled_ui(l.vignette != 0.0, |ui| {
            with_reset!(ui, l.vignette_mid, DEFAULT_VIGNETTE_MID, pct(&mut l.vignette_mid, 0.0, 1.0, "Midpoint", 100.0));
            with_reset!(ui, l.vignette_feather, DEFAULT_VIGNETTE_FEATHER, pct(&mut l.vignette_feather, 0.0, 1.0, "Feather", 100.0));
            with_reset!(ui, l.vignette_round, 0.0, pct(&mut l.vignette_round, -1.0, 1.0, "Roundness", 100.0));
        });

        sub_heading(ui, "Grain");
        with_reset!(ui, l.grain, 0.0, pct(&mut l.grain, 0.0, 1.0, "Amount", 100.0));
        ui.add_enabled_ui(l.grain > 0.0, |ui| {
            with_reset!(ui, l.grain_size, DEFAULT_GRAIN_SIZE, egui::Slider::new(&mut l.grain_size, 1.0..=4.0).text("Size").fixed_decimals(1));
            with_reset!(ui, l.grain_rough, DEFAULT_GRAIN_ROUGH, pct(&mut l.grain_rough, 0.0, 1.0, "Roughness", 100.0));
        });
    });

    edit_section(app, ui, "adj_curve", "Tone curve", &mut |app: &mut StudioApp, ui: &mut egui::Ui| {
        ui.horizontal(|ui| {
            for (i, name) in ["RGB", "Red", "Green", "Blue"].iter().enumerate() {
                if ui.selectable_label(app.curve_channel == i, *name).clicked() {
                    app.curve_channel = i;
                }
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let ch = app.curve_channel;
                if ui
                    .add_enabled(!app.look.curves[ch].is_identity(), egui::Button::new("Reset").small())
                    .on_hover_text("Straighten this curve")
                    .clicked()
                {
                    app.look.curves[ch] = crate::develop::Curve::default();
                }
            });
        });
        let ch = app.curve_channel;
        let color = [theme::TEXT, Color32::from_rgb(235, 90, 90), Color32::from_rgb(90, 200, 110), Color32::from_rgb(90, 140, 240)][ch];
        curve_editor(ui, &mut app.look.curves[ch], color);
        ui.label(
            egui::RichText::new("Drag points. Click to add, right-click to remove.")
                .size(11.0)
                .color(theme::TEXT_DIM),
        );
        let l = &mut app.look;
        with_reset!(ui, l.s_curve, 0.0, pct(&mut l.s_curve, -1.0, 1.0, "S-curve", 100.0));
        with_reset!(ui, l.pivot, crate::finish::DEFAULT_PIVOT, egui::Slider::new(&mut l.pivot, 0.1..=0.9).text("Pivot").fixed_decimals(2));
    });

    edit_section(app, ui, "adj_hsl", "HSL / Color", &mut |app: &mut StudioApp, ui: &mut egui::Ui| {
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing.x = 6.0;
            for (i, c) in crate::develop::HSL_SWATCH.iter().enumerate() {
                let sel = app.hsl_band == i;
                let (rect, resp) = ui.allocate_exact_size(Vec2::splat(22.0), Sense::click());
                ui.painter().circle_filled(rect.center(), 9.0, Color32::from_rgb(c[0], c[1], c[2]));
                if sel {
                    ui.painter().circle_stroke(rect.center(), 11.0, Stroke::new(2.0, Color32::WHITE));
                }
                if resp.on_hover_text(crate::develop::HSL_NAMES[i]).clicked() {
                    app.hsl_band = i;
                }
            }
        });
        let b = app.hsl_band;
        ui.label(egui::RichText::new(crate::develop::HSL_NAMES[b]).size(12.0).color(theme::TEXT_DIM));
        let band = &mut app.look.hsl[b];
        with_reset!(ui, band[0], 0.0, pct(&mut band[0], -1.0, 1.0, "Hue", 100.0));
        with_reset!(ui, band[1], 0.0, pct(&mut band[1], -1.0, 1.0, "Saturation", 100.0));
        with_reset!(ui, band[2], 0.0, pct(&mut band[2], -1.0, 1.0, "Luminance", 100.0));
    });

    edit_section(app, ui, "adj_grade", "Color grading", &mut |app: &mut StudioApp, ui: &mut egui::Ui| {
        const RANGES: [&str; 4] = ["Shadows", "Midtones", "Highlights", "Global"];
        ui.label(
            egui::RichText::new(
                "Tint the dark, middle and bright parts of the picture, or all of it with Global. \
                 Click or drag on a wheel to pick the tint; the center means no tint.",
            )
            .size(11.0)
            .color(theme::TEXT_DIM),
        );
        ui.add_space(4.0);
        let gap = 8.0;
        let w = ((ui.available_width() - 2.0 * gap) / 3.0).min(96.0).floor();
        // One wheel with its name below; picking on it selects the range.
        let wheel = |app: &mut StudioApp, ui: &mut egui::Ui, i: usize| {
            ui.allocate_ui_with_layout(Vec2::new(w, w + 20.0), egui::Layout::top_down(egui::Align::Center), |ui| {
                let sel = app.grade_range == i;
                let g = &mut app.look.grade[i];
                let resp = grade_wheel(ui, w, g, sel)
                    .on_hover_text("Click or drag to pick the tint. Double-click to remove it.");
                let (rect, label) = ui.allocate_exact_size(Vec2::new(w, 16.0), Sense::click());
                let color = if sel { theme::TEXT } else { theme::TEXT_DIM };
                let text = ui.painter().layout_no_wrap(RANGES[i].to_string(), egui::FontId::proportional(12.0), color);
                let left = rect.center().x - (text.size().x + 14.0) * 0.5;
                ui.painter().circle_filled(Pos2::new(left + 5.0, rect.center().y), 5.0, tint_color(g));
                ui.painter().galley(Pos2::new(left + 14.0, rect.center().y - text.size().y * 0.5), text, color);
                if resp.is_pointer_button_down_on() || resp.clicked() || label.clicked() {
                    app.grade_range = i;
                }
            });
        };
        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing.x = gap;
            for i in 0..3 {
                wheel(app, ui, i);
            }
        });
        ui.horizontal_top(|ui| {
            ui.add_space(w + gap);
            wheel(app, ui, 3);
        });
        ui.add_space(4.0);

        let r = app.grade_range;
        ui.horizontal(|ui| {
            let (rect, _) = ui.allocate_exact_size(Vec2::splat(14.0), Sense::hover());
            ui.painter().circle_filled(rect.center(), 6.0, tint_color(&app.look.grade[r]));
            ui.label(egui::RichText::new(RANGES[r]).size(12.0).color(theme::TEXT));
        });
        let g = &mut app.look.grade[r];
        with_reset!(ui, g[0], 0.0, egui::Slider::new(&mut g[0], 0.0..=360.0).text("Hue").suffix("°").fixed_decimals(0));
        with_reset!(ui, g[1], 0.0, pct(&mut g[1], 0.0, 1.0, "Saturation", 100.0));
        with_reset!(ui, g[2], 0.0, pct(&mut g[2], -1.0, 1.0, "Luminance", 100.0));
        ui.add_space(4.0);
        let l = &mut app.look;
        with_reset!(ui, l.grade_blend, crate::finish::DEFAULT_BLEND, pct(&mut l.grade_blend, 0.0, 1.0, "Blending", 100.0));
        with_reset!(ui, l.grade_balance, 0.0, pct(&mut l.grade_balance, -1.0, 1.0, "Balance", 100.0));
    });
}

/// A small dim heading for a group of sliders inside a card.
fn sub_heading(ui: &mut egui::Ui, text: &str) {
    ui.add_space(4.0);
    ui.label(egui::RichText::new(text).size(12.0).color(theme::TEXT_DIM));
}

/// The tint a color grading range adds: grey when it adds none.
fn tint_color(g: &[f32]) -> Color32 {
    let rgb = crate::develop::hsl_to_rgb([g[0], g[1].clamp(0.0, 1.0), 0.5]);
    Color32::from_rgb((rgb[0] * 255.0) as u8, (rgb[1] * 255.0) as u8, (rgb[2] * 255.0) as u8)
}

/// A color wheel for one grading range. The angle sets the hue in degrees
/// (red to the right, counter-clockwise), the distance from the center sets
/// the saturation (0 to 1). Double-click removes the tint.
fn grade_wheel(ui: &mut egui::Ui, size: f32, g: &mut [f32; 3], selected: bool) -> egui::Response {
    let [hue, sat, _] = g;
    let (rect, mut resp) = ui.allocate_exact_size(Vec2::splat(size), Sense::click_and_drag());
    let c = rect.center();
    let r = size * 0.5 - 4.0;
    if resp.double_clicked() {
        *hue = 0.0;
        *sat = 0.0;
        resp.mark_changed();
    } else if resp.is_pointer_button_down_on() {
        if let Some(p) = resp.interact_pointer_pos() {
            let d = p - c;
            let dist = d.length();
            *sat = (dist / r).clamp(0.0, 1.0);
            if dist > 0.5 {
                *hue = (-d.y).atan2(d.x).to_degrees().rem_euclid(360.0);
            }
            resp.mark_changed();
        }
    }

    // Wheel: rings from grey in the center to full color at the rim.
    let color = |h: f32, s: f32| tint_color(&[h, s]);
    let (segments, rings) = (72usize, 4usize);
    let mut mesh = egui::Mesh::default();
    for k in 0..=rings {
        let s = k as f32 / rings as f32;
        for j in 0..segments {
            let h = j as f32 * 360.0 / segments as f32;
            let a = h.to_radians();
            let pos = c + Vec2::new(a.cos(), -a.sin()) * (s * r);
            mesh.colored_vertex(pos, color(h, s));
        }
    }
    for k in 0..rings {
        for j in 0..segments {
            let n = segments as u32;
            let (k, j) = (k as u32, j as u32);
            let a = k * n + j;
            let b = k * n + (j + 1) % n;
            let cc = (k + 1) * n + j;
            let d = (k + 1) * n + (j + 1) % n;
            mesh.add_triangle(a, cc, d);
            mesh.add_triangle(a, d, b);
        }
    }
    let painter = ui.painter();
    painter.add(egui::Shape::mesh(mesh));
    let hovered = resp.hovered() || resp.dragged();
    painter.circle_stroke(c, r, Stroke::new(1.0, if hovered { theme::TEXT_DIM } else { theme::BORDER }));
    if selected {
        painter.circle_stroke(c, r + 2.5, Stroke::new(2.0, theme::ACCENT));
    }

    // Marker at the chosen tint.
    let a = hue.to_radians();
    let m = c + Vec2::new(a.cos(), -a.sin()) * (*sat * r);
    painter.circle_filled(m, 5.0, tint_color(&[*hue, *sat]));
    painter.circle_stroke(m, 5.0, Stroke::new(1.5, Color32::WHITE));
    painter.circle_stroke(m, 6.5, Stroke::new(1.0, Color32::from_black_alpha(160)));
    resp
}

/// One folding card in Edit, greyed out until a file is open.
fn edit_section(
    app: &mut StudioApp,
    ui: &mut egui::Ui,
    key: &str,
    title: &str,
    body: &mut dyn FnMut(&mut StudioApp, &mut egui::Ui),
) {
    let has_scene = app.scene.is_some();
    theme::card().show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal(|ui| fold_title(ui, app, key, title));
        if !is_open(app, key) {
            return;
        }
        ui.add_enabled_ui(has_scene, |ui| body(app, ui));
    });
}

/// A slider shown as whole numbers (value × `scale`), like -100..100.
pub(crate) fn pct<'a>(v: &'a mut f32, lo: f32, hi: f32, text: &str, scale: f64) -> egui::Slider<'a> {
    egui::Slider::new(v, lo..=hi)
        .text(text)
        .custom_formatter(move |x, _| format!("{:.0}", x * scale))
        .custom_parser(move |t| t.trim().parse::<f64>().ok().map(|x| x / scale))
}

/// Point tone-curve editor: drag points, click to add, right-click to remove.
fn curve_editor(ui: &mut egui::Ui, curve: &mut crate::develop::Curve, color: Color32) {
    let w = ui.available_width();
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, w * 0.75), Sense::click_and_drag());
    let p = ui.painter_at(rect);
    p.rect_filled(rect, CornerRadius::same(6), theme::BG);
    let grid = Stroke::new(1.0, Color32::from_white_alpha(18));
    for k in 1..4 {
        let t = k as f32 / 4.0;
        p.line_segment([Pos2::new(rect.left() + t * rect.width(), rect.top()), Pos2::new(rect.left() + t * rect.width(), rect.bottom())], grid);
        p.line_segment([Pos2::new(rect.left(), rect.top() + t * rect.height()), Pos2::new(rect.right(), rect.top() + t * rect.height())], grid);
    }
    p.line_segment([rect.left_bottom(), rect.right_top()], Stroke::new(1.0, Color32::from_white_alpha(30)));
    let to_screen = |q: [f32; 2]| Pos2::new(rect.left() + q[0] * rect.width(), rect.bottom() - q[1] * rect.height());
    let to_curve = |s: Pos2| [((s.x - rect.left()) / rect.width()).clamp(0.0, 1.0), ((rect.bottom() - s.y) / rect.height()).clamp(0.0, 1.0)];

    // Which point is being dragged lives in egui's memory between frames.
    let drag_id = resp.id.with("drag_point");
    let mut dragging: Option<usize> = ui.data(|d| d.get_temp(drag_id));
    let nearest = |pos: Pos2, c: &crate::develop::Curve| {
        c.points()
            .iter()
            .enumerate()
            .map(|(i, q)| (i, to_screen(*q).distance(pos)))
            .filter(|(_, d)| *d < 12.0)
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(i, _)| i)
    };

    if let Some(pos) = resp.interact_pointer_pos() {
        if resp.drag_started() || resp.clicked() {
            dragging = nearest(pos, curve);
            if dragging.is_none() && (curve.n as usize) < crate::develop::MAX_POINTS {
                // Add a point at the pointer.
                let mut pts = curve.points().to_vec();
                pts.push(to_curve(pos));
                *curve = crate::develop::Curve::from_points(&pts);
                dragging = nearest(pos, curve);
            }
        }
        if resp.dragged() {
            if let Some(i) = dragging {
                let n = curve.n as usize;
                let mut q = to_curve(pos);
                // Ends stay at the left and right edges; others stay between neighbors.
                if i == 0 {
                    q[0] = 0.0;
                } else if i == n - 1 {
                    q[0] = 1.0;
                } else {
                    let lo = curve.pts[i - 1][0] + 0.01;
                    let hi = curve.pts[i + 1][0] - 0.01;
                    q[0] = q[0].clamp(lo, hi.max(lo));
                }
                curve.pts[i] = q;
            }
        }
    }
    if resp.drag_stopped() {
        dragging = None;
    }
    if resp.secondary_clicked() {
        if let Some(pos) = resp.interact_pointer_pos() {
            if let Some(i) = nearest(pos, curve) {
                let n = curve.n as usize;
                if i != 0 && i != n - 1 {
                    let pts: Vec<[f32; 2]> = curve.points().iter().enumerate().filter(|(k, _)| *k != i).map(|(_, q)| *q).collect();
                    *curve = crate::develop::Curve::from_points(&pts);
                }
            }
        }
    }
    ui.data_mut(|d| match dragging {
        Some(i) => {
            d.insert_temp(drag_id, i);
        }
        None => d.remove::<usize>(drag_id),
    });

    // The curve and its points.
    let line: Vec<Pos2> = (0..=64).map(|k| {
        let x = k as f32 / 64.0;
        to_screen([x, curve.eval(x)])
    }).collect();
    p.add(egui::Shape::line(line, Stroke::new(2.0, color)));
    for q in curve.points() {
        p.circle_filled(to_screen(*q), 4.5, color);
        p.circle_stroke(to_screen(*q), 4.5, Stroke::new(1.0, theme::BG));
    }
}

// ----- Crop ----------------------------------------------------------------

/// Crop tool button, aspect-ratio choices, the resulting size and Reset.
pub fn crop_card(app: &mut StudioApp, ui: &mut egui::Ui) {
    rotate_row_card(app, ui);
}

/// Straighten: turn by any angle, -45° to 45°. Dragging redraws a quick
/// preview with a grid; letting go finishes it as one undo step.
fn straighten_row(app: &mut StudioApp, ui: &mut egui::Ui) {
    let current = app.scene.as_ref().map_or(0.0, |s| s.angle);
    let mut deg = current;
    let ctx = ui.ctx().clone();
    ui.horizontal(|ui| {
        let r = ui
            .add_enabled(
                !app.busy(),
                egui::Slider::new(&mut deg, -45.0..=45.0).text("Straighten").suffix("°").fixed_decimals(1).step_by(0.1),
            )
            .on_hover_text(
                "Turn the picture by any angle (right is clockwise). It is zoomed in slightly so no \
                 empty corners show. Straighten before painting: painted areas, arrows and the crop stay \
                 where they are on the picture.",
            );
        if r.changed() && (deg - current).abs() > 1e-4 {
            app.set_angle(&ctx, deg, r.dragged());
        }
        if r.drag_stopped() {
            app.finish_angle(&ctx);
        }
        if reset_icon(ui, current == 0.0) {
            app.set_angle(&ctx, 0.0, false);
        }
    });
}

/// Crop and rotate share one card: rotate buttons on top, then crop.
fn rotate_row_card(app: &mut StudioApp, ui: &mut egui::Ui) {
    // Shown (greyed out) even with no file open.
    let has_file = app.scene.is_some();
    let (iw, ih, crop, aspect) = match &app.scene {
        Some(sc) => (sc.full.width(), sc.full.height(), sc.crop, sc.aspect),
        None => (0, 0, crate::crop::CropRect::FULL, crate::crop::Aspect::Free),
    };
    theme::card().show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal(|ui| {
            fold_title(ui, app, "crop", "Rotate, straighten & crop");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .add_enabled(has_file && !crop.is_full(), egui::Button::new("Reset crop").small())
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
        if !has_file {
            ui.disable();
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
        straighten_row(app, ui);
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
        let text = if !has_file {
            "No file open".to_string()
        } else if crop.is_full() {
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
    // Shown (greyed out) even with no file open.
    let has_file = app.scene.is_some();
    theme::card().show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal(|ui| {
            fold_title(ui, app, "border", "Border");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_enabled(has_file, egui::Checkbox::new(&mut app.border.enabled, "Add border"));
            });
        });
        if !is_open(app, "border") {
            return;
        }
        if !has_file {
            ui.disable();
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
            // Photo corners: rounded, and paper corner mounts.
            ui.label(egui::RichText::new("Photo corners").size(12.0).color(theme::TEXT_DIM));
            with_reset!(
                ui,
                b.radius,
                0.0,
                egui::Slider::new(&mut b.radius, 0.0..=0.25)
                    .text("Rounded")
                    .custom_formatter(|v, _| format!("{:.0}%", v * 100.0))
                    .custom_parser(|t| t.trim_end_matches('%').trim().parse::<f64>().ok().map(|v| v / 100.0))
            );
            ui.checkbox(&mut b.mounts, "Corner mounts")
                .on_hover_text("Small paper triangles holding the photo at its corners, as in a photo album");
            if b.mounts {
                with_reset!(
                    ui,
                    b.mount_size,
                    crate::border::DEFAULT_MOUNT_SIZE,
                    egui::Slider::new(&mut b.mount_size, 0.03..=0.25)
                        .text("Mount size")
                        .custom_formatter(|v, _| format!("{:.0}%", v * 100.0))
                        .custom_parser(|t| t.trim_end_matches('%').trim().parse::<f64>().ok().map(|v| v / 100.0))
                );
                ui.horizontal(|ui| {
                    ui.label("Mount color");
                    color_swatches(ui, &mut b.mount_color, &app.palette);
                    ui.color_edit_button_srgb(&mut b.mount_color).on_hover_text("Any color");
                });
            }
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
        if app.border.enabled && app.border.fit == crate::border::BorderFit::Polaroid {
            polaroid_controls(app, ui);
        }
        // The same border on the other files.
        let n = app.apply_targets().len();
        let same = icons::button(Icon::Image, format!("Apply border to {}", app.apply_target_words()), theme::TEXT);
        if ui
            .add_enabled(n > 0, same)
            .on_hover_text(
                "Gives the other open files this border (thickness, colour, style and photo corners), or only the \
                 files selected in the Files panel. Crop and caption are not changed. With the border off, it is \
                 removed from them.",
            )
            .on_disabled_hover_text("Open more than one file to use this")
            .clicked()
        {
            app.apply_border_to_targets();
        }
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

/// Polaroid frame: the bottom strip's depth, set directly or by a target
/// ratio (typed, or one of the instant-film card sizes).
fn polaroid_controls(app: &mut StudioApp, ui: &mut egui::Ui) {
    let Some(sc) = &app.scene else { return };
    let [_, _, cw, ch] = sc.crop.pixels(sc.full.width(), sc.full.height());
    let (w, h) = (cw as f32, ch as f32);
    let b = &mut app.border;
    let shown = b.polaroid_bottom(w, h);

    ui.label(egui::RichText::new("Polaroid bottom strip").size(12.0).color(theme::TEXT_DIM));
    ui.horizontal(|ui| {
        let mut depth = shown;
        let r = ui
            .add(
                egui::Slider::new(&mut depth, crate::border::POLAROID_BOTTOM_RANGE)
                    .text("Bottom")
                    .custom_formatter(|v, _| format!("{v:.1}×"))
                    .custom_parser(|t| t.trim_end_matches('×').trim().parse::<f64>().ok()),
            )
            .on_hover_text("How deep the bottom strip is, in times the top and side edges. Moving it lets go of the target ratio.");
        if r.changed() {
            b.bottom = depth;
            b.target = None;
        }
        let is_default = b.target.is_none() && b.bottom == crate::border::POLAROID_BOTTOM;
        if reset_icon(ui, is_default) {
            b.bottom = crate::border::POLAROID_BOTTOM;
            b.target = None;
        }
    });

    // Target ratio, written as width : height with width 1.
    let [ow, oh, _, _] = b.layout(w, h);
    let mut target = b.target.unwrap_or(if ow > 0.0 { oh / ow } else { 1.0 });
    ui.horizontal(|ui| {
        ui.label("Target ratio  1 :");
        let r = ui
            .add(egui::DragValue::new(&mut target).range(0.5..=3.0).speed(0.005).fixed_decimals(2))
            .on_hover_text(
                "Height for a width of 1 (above 1 is tall, below 1 is wide). The bottom strip grows \
                 or shrinks to reach it. The picture is not cropped.",
            );
        if r.changed() {
            b.target = Some(target);
        }
        let note = if b.target.is_some() { format!("({}, set)", ratio_text(1.0, target)) } else { format!("({})", ratio_text(1.0, target)) };
        ui.label(egui::RichText::new(note).size(11.0).color(if b.target.is_some() { theme::ACCENT } else { theme::TEXT_DIM }));
    });
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = Vec2::new(6.0, 6.0);
        for (name, ratio, tip) in crate::border::INSTANT_CARDS {
            let sel = b.target.is_some_and(|t| (t - ratio).abs() < 1e-4);
            let label = format!("{name} {}", ratio_text(1.0, ratio));
            let btn = egui::Button::new(egui::RichText::new(label).size(12.0).color(if sel { Color32::WHITE } else { theme::TEXT }))
                .fill(if sel { theme::ACCENT } else { theme::PANEL })
                .corner_radius(CornerRadius::same(12));
            if ui.add(btn).on_hover_text(tip).clicked() {
                b.target = Some(ratio);
            }
        }
    });
    if !b.polaroid_target_reached(w, h) {
        ui.label(
            egui::RichText::new(
                "This picture can't reach that ratio with the bottom strip alone. \
                 Change Thickness, or crop the picture closer to the card's shape.",
            )
            .size(12.0)
            .color(Color32::from_rgb(240, 180, 90)),
        );
    }
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
    // Shown (greyed out) even with no file open.
    let has_file = app.scene.is_some();
    theme::card().show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal(|ui| {
            fold_title(ui, app, "caption", "Caption");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_enabled(has_file, egui::Checkbox::new(&mut app.caption.enabled, "Add caption"));
            });
        });
        if !is_open(app, "caption") {
            return;
        }
        if !has_file {
            ui.disable();
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
                ui.label("Where");
                use crate::caption::Area;
                if ui.selectable_label(c.area == Area::Picture, "On picture").clicked() {
                    c.area = Area::Picture;
                }
                let r = ui
                    .add_enabled(app.border.enabled, egui::Button::selectable(c.area == Area::Border, "In border"))
                    .on_hover_text("In the border strip below the picture (or above it with Top)")
                    .on_disabled_hover_text("Turn on a border first (Border card)");
                if r.clicked() && c.area != Area::Border {
                    c.area = Area::Border;
                    // White text with a shadow suits a photo, not a light border.
                    let [r, g, b] = app.border.color;
                    let light = (r as u32 + g as u32 + b as u32) > 3 * 160;
                    if light && c.color == [255, 255, 255] {
                        c.color = [40, 40, 40];
                        c.shadow = false;
                    }
                }
            });
            if c.area == crate::caption::Area::Border && !app.border.enabled {
                ui.label(
                    egui::RichText::new("No border is on, so the caption is shown on the picture.")
                        .size(12.0)
                        .color(theme::TEXT_DIM),
                );
            }
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
pub(crate) fn eyedropper_button(ui: &mut egui::Ui, active: bool) -> egui::Response {
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
pub(crate) fn color_swatches(ui: &mut egui::Ui, current: &mut [u8; 3], palette: &[[u8; 3]]) {
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

// ----- Film simulation -------------------------------------------------------

/// Film styles grouped by kind, with a strength slider. Reset (top bar)
/// removes the film style.
pub fn film_card(app: &mut StudioApp, ui: &mut egui::Ui) {
    let sims = crate::film::all();
    theme::card().show(ui, |ui| {
        ui.set_width(ui.available_width());
        fold_title(ui, app, "film", "Film Simulation");
        if !is_open(app, "film") {
            return;
        }
        // Strength: re-applies the chosen style as it moves.
        let r = with_reset!(
            ui,
            app.film_strength,
            1.0,
            egui::Slider::new(&mut app.film_strength, 0.0..=1.0)
                .text("Strength")
                .custom_formatter(|v, _| format!("{:.0}%", v * 100.0))
                .custom_parser(|t| t.trim_end_matches('%').trim().parse::<f64>().ok().map(|v| v / 100.0))
        );
        if r {
            if let Some(i) = app.film_choice {
                app.look = crate::film::at_strength(&sims[i].look, app.film_strength);
            }
        }
        for group in crate::film::GROUPS {
            let members: Vec<usize> = (0..sims.len()).filter(|&i| sims[i].group == group).collect();
            if members.is_empty() {
                continue;
            }
            ui.add_space(2.0);
            ui.label(egui::RichText::new(group.to_uppercase()).size(11.0).strong().color(theme::TEXT_DIM));
            let tiles: Vec<Tile> = members
                .iter()
                .map(|&i| Tile {
                    key: format!("film:{}", sims[i].name),
                    name: sims[i].name.to_string(),
                    note: sims[i].note.to_string(),
                    look: crate::film::at_strength(&sims[i].look, app.film_strength),
                    thumb_look: sims[i].look,
                    removable: false,
                })
                .collect();
            if let Some(TileAction::Apply(t)) = tile_grid(app, ui, &tiles) {
                let i = members[t];
                app.film_choice = Some(i);
                app.look = crate::film::at_strength(&sims[i].look, app.film_strength);
                app.status = format!("Applied the {} film style.", sims[i].name);
            }
            if app.scene.is_none() {
                break; // one "open a photo" note is enough
            }
        }
    });
}
