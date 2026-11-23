//! The layout screen: the layout on the canvas (click a box to select it,
//! drag to move its photo, scroll to zoom, double-click to reset) and its
//! settings in the side panel (template, shape, gap, margin, colour, saving).

use eframe::egui::{self, Color32, CornerRadius, FontId, Pos2, Rect, Sense, Stroke, Vec2};

use crate::app::StudioApp;
use crate::export::Format;
use crate::icons::{self, Icon};
use crate::layout::{self, Template};
use crate::slider::SliderDigits;
use crate::theme;

/// A file being dragged from the Files panel (its index there).
pub struct FileDrag(pub usize);

/// The layout on the canvas, with box selection, moving and zooming.
pub fn canvas(app: &mut StudioApp, ui: &mut egui::Ui) {
    let area = ui.max_rect();
    let painter = ui.painter_at(area);
    painter.rect_filled(area, CornerRadius::ZERO, theme::BG);
    crate::canvas::draw_dot_grid(&painter, area);
    let ctx = ui.ctx().clone();
    app.refresh_layout_texture(&ctx);
    let Some(l) = &app.layout else { return };

    // Fit the picture into the canvas.
    let frame = area.shrink2(Vec2::new(48.0, 56.0));
    let ratio = l.ratio();
    let size = if frame.width() / frame.height() > ratio {
        Vec2::new(frame.height() * ratio, frame.height())
    } else {
        Vec2::new(frame.width(), frame.width() / ratio)
    };
    let out_rect = Rect::from_center_size(frame.center(), size);
    painter.rect_filled(out_rect.expand(6.0), CornerRadius::same(6), Color32::from_black_alpha(90));
    if let Some(t) = &app.layout_tex {
        painter.image(t.id(), out_rect, Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), Color32::WHITE);
    }
    let boxes: Vec<Rect> = l
        .rects(out_rect.width(), out_rect.height())
        .iter()
        .map(|r| Rect::from_min_size(out_rect.min + Vec2::new(r[0], r[1]), Vec2::new(r[2], r[3])))
        .collect();

    let response = ui.interact(area, egui::Id::unique("layout_canvas"), Sense::click_and_drag());
    let pointer = response.hover_pos();
    let under = |p: Pos2| boxes.iter().position(|b| b.contains(p));
    let hovered = pointer.and_then(under);

    // Select a box.
    if response.clicked() {
        app.layout_cell = response.interact_pointer_pos().and_then(under);
    }
    // Drag a photo: inside its box it moves; over another box it swaps
    // with that box's photo when let go.
    if response.drag_started() {
        app.layout_drag = response.interact_pointer_pos().and_then(under);
        app.layout_cell = app.layout_drag.or(app.layout_cell);
        app.layout_drag_saved = app.layout_drag.and_then(|i| app.layout.as_ref().map(|l| l.cells[i]));
    }
    let drag_target = app.layout_drag.and_then(|i| {
        let p = response.interact_pointer_pos().or(pointer)?;
        under(p).filter(|&j| j != i)
    });
    if let Some(i) = app.layout_drag {
        if drag_target.is_some() || !response.interact_pointer_pos().or(pointer).is_some_and(|p| boxes[i].contains(p)) {
            // Outside its box: the photo stays as it was.
            if let (Some(saved), Some(l)) = (app.layout_drag_saved, app.layout.as_mut()) {
                l.cells[i] = saved;
            }
        } else {
            let delta = response.drag_delta();
            move_photo(app, i, boxes[i], |cell, iw, ih, s| {
                cell.center[0] -= delta.x / s / iw;
                cell.center[1] -= delta.y / s / ih;
            });
        }
    }
    if response.drag_stopped() {
        if let (Some(i), Some(j)) = (app.layout_drag, drag_target) {
            app.swap_layout_cells(i, j);
        }
        app.layout_drag = None;
        app.layout_drag_saved = None;
    }
    // A file dragged in from the Files panel.
    let file_drag = egui::DragAndDrop::payload::<FileDrag>(ui.ctx()).map(|p| p.0);
    let file_target = file_drag.and_then(|_| ui.ctx().pointer_latest_pos()).and_then(under);
    if file_drag.is_some() && ui.input(|inp| inp.pointer.any_released()) {
        if let (Some(f), Some(j)) = (egui::DragAndDrop::take_payload::<FileDrag>(ui.ctx()).map(|p| p.0), file_target) {
            app.place_in_layout(j, f);
            app.layout_cell = Some(j);
        }
    }
    // Zoom the photo under the pointer.
    if let Some(i) = hovered {
        let (scroll, pinch) = ui.input(|inp| (inp.smooth_scroll_delta.y, inp.zoom_delta()));
        let factor = pinch * (scroll * 0.002).exp();
        if (factor - 1.0).abs() > 1e-4 {
            move_photo(app, i, boxes[i], |cell, _, _, _| cell.zoom *= factor);
        }
    }
    // Double-click: fill the box again, centered.
    if response.double_clicked() {
        if let Some(i) = response.interact_pointer_pos().and_then(under) {
            if let Some(c) = app.layout.as_mut().and_then(|l| l.cells.get_mut(i)) {
                c.zoom = 1.0;
                c.center = [0.5, 0.5];
            }
        }
    }

    // Box outlines and empty-box hints.
    let Some(l) = &app.layout else { return };
    for (i, b) in boxes.iter().enumerate() {
        let empty = l.cells.get(i).is_none_or(|c| c.source.is_none());
        if empty {
            painter.rect_stroke(*b, CornerRadius::ZERO, Stroke::new(1.0, Color32::from_gray(150)), egui::StrokeKind::Inside);
            painter.text(
                b.center(),
                egui::Align2::CENTER_CENTER,
                "Empty box\ndrag a file here",
                FontId::proportional(12.0),
                Color32::from_gray(120),
            );
        }
        if app.layout_cell == Some(i) {
            painter.rect_stroke(*b, CornerRadius::ZERO, Stroke::new(2.0, theme::ACCENT), egui::StrokeKind::Inside);
        } else if hovered == Some(i) {
            painter.rect_stroke(*b, CornerRadius::ZERO, Stroke::new(1.0, theme::ACCENT.gamma_multiply(0.6)), egui::StrokeKind::Inside);
        }
    }
    // Where a dragged photo or file would land.
    let landing = drag_target.or(if file_drag.is_some() { file_target } else { None });
    if let Some(j) = landing {
        painter.rect_filled(boxes[j], CornerRadius::ZERO, theme::ACCENT.gamma_multiply(0.25));
        painter.rect_stroke(boxes[j], CornerRadius::ZERO, Stroke::new(3.0, theme::ACCENT), egui::StrokeKind::Inside);
        let text = if drag_target.is_some() { format!("Swap with box {}", j + 1) } else { format!("Put in box {}", j + 1) };
        painter.text(boxes[j].center(), egui::Align2::CENTER_CENTER, text, FontId::proportional(14.0), Color32::WHITE);
    }
    if let Some(name) = file_drag.and_then(|f| app.files.get(f)).map(|e| e.name.clone()) {
        if let Some(p) = ui.ctx().pointer_latest_pos() {
            let g = painter.layout_no_wrap(name, FontId::proportional(12.0), Color32::WHITE);
            let r = Rect::from_min_size(p + Vec2::new(14.0, 10.0), g.size() + Vec2::new(12.0, 6.0));
            painter.rect_filled(r, CornerRadius::same(6), theme::ACCENT.gamma_multiply(0.9));
            painter.galley(r.min + Vec2::new(6.0, 3.0), g, Color32::WHITE);
        }
    }
    if hovered.is_some() {
        ui.ctx().set_cursor_icon(if app.layout_drag.is_some() { egui::CursorIcon::Grabbing } else { egui::CursorIcon::Grab });
    }

    // Size of the saved picture.
    let (w, h) = l.size(app.layout_size);
    let text = format!("Layout · {w} × {h} px · {}", crate::panels::ratio_text(w as f32, h as f32));
    let galley = painter.layout_no_wrap(text, FontId::proportional(11.5), Color32::WHITE);
    let pos = out_rect.center_bottom() + Vec2::new(0.0, 14.0);
    let bg = Rect::from_center_size(pos, galley.size() + Vec2::new(14.0, 6.0));
    painter.rect_filled(bg, CornerRadius::same(6), Color32::from_black_alpha(170));
    painter.galley(bg.center() - galley.size() / 2.0, galley, Color32::WHITE);
}

/// Change a box's zoom or position, then keep the photo covering the box.
/// `f` gets the cell, the photo's size and the photo-to-screen scale.
fn move_photo(app: &mut StudioApp, i: usize, b: Rect, f: impl FnOnce(&mut layout::Cell, f32, f32, f32)) {
    let Some(l) = &mut app.layout else { return };
    let Some(cell) = l.cells.get_mut(i) else { return };
    let Some(img) = cell.source.and_then(|id| app.layout_photos.get(&id)) else { return };
    let (iw, ih) = (img.width() as f32, img.height() as f32);
    let s = layout::cell_scale(cell, iw, ih, b.width(), b.height());
    f(cell, iw, ih, s);
    layout::clamp_center(cell, iw, ih, b.width(), b.height());
}

/// Layout settings in the side panel.
pub fn side_panel(app: &mut StudioApp, ui: &mut egui::Ui) {
    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        ui.spacing_mut().item_spacing.y = 10.0;
        ui.set_max_width(ui.available_width().min(312.0));
        ui.horizontal(|ui| {
            ui.add(Icon::Image.image(18.0, theme::ACCENT));
            ui.label(egui::RichText::new("Layout").size(15.5).color(theme::TEXT));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .add(icons::button(Icon::Edit, "Back to editing", theme::TEXT))
                    .on_hover_text("Edit single files again. The layout is kept; open it from the Files panel.")
                    .clicked()
                {
                    app.close_layout();
                }
            });
        });
        ui.label(
            egui::RichText::new(
                "Each file appears as you finished it: colour edits, crop, border and caption. Drag a photo to \
                 move it in its box, or onto another box to swap them; scroll to zoom; double-click to fill the \
                 box again. Drag a file from the left onto a box to put it there (or select a box, then click \
                 the file).",
            )
            .size(12.0)
            .color(theme::TEXT_DIM),
        );
        let Some(l) = app.layout.as_mut() else { return };

        theme::card().show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(egui::RichText::new("Template").size(12.0).color(theme::TEXT_DIM));
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = Vec2::new(6.0, 6.0);
                for t in Template::ALL {
                    if chip(ui, t.label(), l.template == t).clicked() && l.template != t {
                        l.set_template(t);
                    }
                }
            });
            ui.label(egui::RichText::new("Shape").size(12.0).color(theme::TEXT_DIM));
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = Vec2::new(6.0, 6.0);
                for a in layout::SHAPES {
                    if chip(ui, a.label(), l.aspect == a).on_hover_text(a.hint()).clicked() {
                        l.aspect = a;
                    }
                }
            });
            fn pct<'a>(v: &'a mut f32, max: f32, name: &str) -> egui::Slider<'a> {
                egui::Slider::new(v, 0.0..=max)
                    .text(name.to_string())
                    .percent()
            }
            ui.add(pct(&mut l.gap, 0.1, "Gap")).on_hover_text("Space between photos, in % of the shorter side");
            ui.add(pct(&mut l.margin, 0.15, "Margin")).on_hover_text("Space around the edge, in % of the shorter side");
            ui.horizontal(|ui| {
                ui.label("Color");
                crate::panels::color_swatches(ui, &mut l.color, &app.palette);
                ui.color_edit_button_srgb(&mut l.color).on_hover_text("Any color");
            });
        });

        // The selected box.
        theme::card().show(ui, |ui| {
            ui.set_width(ui.available_width());
            let Some(i) = app.layout_cell.filter(|&i| i < l.cells.len()) else {
                ui.label(egui::RichText::new("Click a box on the picture to adjust it.").size(12.0).color(theme::TEXT_DIM));
                return;
            };
            let source = l.cells[i].source;
            let name = source.map(|id| app_file_name(&app.files, &app.scene, id)).unwrap_or_else(|| "Empty".into());
            ui.label(egui::RichText::new(format!("Box {} · {name}", i + 1)).strong());
            if let Some(img) = source.and_then(|id| app.layout_photos.get(&id)) {
                let canvas_box = l.rects(1000.0 * l.ratio().min(1.0), 1000.0 / l.ratio().max(1.0))[i];
                let (iw, ih) = (img.width() as f32, img.height() as f32);
                let min = layout::min_zoom(iw, ih, canvas_box[2], canvas_box[3]);
                let cell = &mut l.cells[i];
                ui.add(egui::Slider::new(&mut cell.zoom, min..=layout::ZOOM_MAX).text("Zoom").two_digits())
                    .on_hover_text("1 fills the box. Lower shows the whole photo with the background around it.");
                layout::clamp_center(cell, iw, ih, canvas_box[2], canvas_box[3]);
                ui.horizontal(|ui| {
                    if ui.button("Fill box").on_hover_text("Zoom 1, centered (or double-click the box)").clicked() {
                        cell.zoom = 1.0;
                        cell.center = [0.5, 0.5];
                    }
                    if ui.button("Show whole photo").clicked() {
                        cell.zoom = min;
                        cell.center = [0.5, 0.5];
                    }
                    if ui.button("Empty").on_hover_text("Take the photo out of this box").clicked() {
                        *cell = layout::Cell::new(None);
                    }
                });
            }
            ui.label(
                egui::RichText::new("Click a file on the left, or drag it onto a box, to put it there.")
                    .size(11.5)
                    .color(theme::TEXT_DIM),
            );
        });

        // Saving.
        theme::card().show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.label("Format");
                egui::ComboBox::from_id_salt("layout_format")
                    .selected_text(app.layout_format.label())
                    .width(170.0)
                    .show_ui(ui, |ui| {
                        for f in [Format::Png, Format::Jpeg, Format::Heif] {
                            ui.selectable_value(&mut app.layout_format, f, f.label());
                        }
                    });
            });
            ui.horizontal(|ui| {
                ui.label("Size");
                egui::ComboBox::from_id_salt("layout_size")
                    .selected_text(format!("{} px", app.layout_size))
                    .width(120.0)
                    .show_ui(ui, |ui| {
                        for s in [1080, 2048, 3000, 4096, 6000] {
                            ui.selectable_value(&mut app.layout_size, s, format!("{s} px"));
                        }
                    });
                ui.label(egui::RichText::new("long side").size(12.0).color(theme::TEXT_DIM));
            });
            ui.horizontal(|ui| {
                let save = icons::button(Icon::Download, egui::RichText::new("Save layout…").color(Color32::WHITE).strong(), Color32::WHITE)
                    .fill(theme::ACCENT)
                    .min_size(Vec2::new(0.0, 30.0));
                if ui.add_enabled(!app.busy(), save).clicked() {
                    app.export_layout();
                }
                if let Some(p) = app.last_export.clone() {
                    if ui.button("Show file").clicked() {
                        crate::export::reveal(&p);
                    }
                }
            });
            if ui
                .small_button("Refresh photos")
                .on_hover_text("Use the latest edits of each file (done each time the layout opens)")
                .clicked()
            {
                app.open_layout();
            }
        });
    });
}

/// File name for a scene id (looking at the open file too).
fn app_file_name(files: &[crate::app::FileEntry], current: &Option<crate::scene::Scene>, id: u64) -> String {
    current
        .iter()
        .chain(files.iter().filter_map(|f| f.scene.as_ref()))
        .find(|s| s.id == id)
        .map(|s| s.name.clone())
        .unwrap_or_else(|| "(closed)".into())
}

/// A rounded choice button, highlighted when chosen.
fn chip(ui: &mut egui::Ui, label: &str, selected: bool) -> egui::Response {
    let b = egui::Button::new(egui::RichText::new(label).size(12.0).color(if selected { Color32::WHITE } else { theme::TEXT }))
        .fill(if selected { theme::ACCENT } else { theme::PANEL })
        .corner_radius(CornerRadius::same(12));
    ui.add(b)
}
