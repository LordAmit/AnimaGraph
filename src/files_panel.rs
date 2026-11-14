//! The Files panel: every photo, video or photo sequence opened in this
//! session, with a thumbnail. Click one to work on it, or remove it.

use eframe::egui::{self, Color32, CornerRadius, FontId, Pos2, Rect, Sense, Stroke, Vec2};

use crate::app::StudioApp;
use crate::icons::Icon;
use crate::theme;

pub fn files_panel(app: &mut StudioApp, ui: &mut egui::Ui) {
    ui.horizontal(|ui| {
        theme::section_title(ui, "Files");
        ui.label(
            egui::RichText::new(app.files.len().to_string())
                .size(11.0)
                .color(theme::TEXT_DIM),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let add = egui::Button::image(Icon::Plus.image(16.0, theme::TEXT))
                .fill(theme::PANEL_RAISED)
                .min_size(Vec2::splat(28.0));
            if ui.add(add).on_hover_text("Open more photos or videos").clicked() {
                let ctx = ui.ctx().clone();
                app.open_photo_dialog(&ctx);
            }
        });
    });
    ui.add_space(4.0);

    if app.files.is_empty() {
        ui.add_space(12.0);
        ui.vertical_centered(|ui| {
            ui.add(Icon::Image.image(28.0, theme::TEXT_DIM));
            ui.label(
                egui::RichText::new("Photos and videos you open are listed here.")
                    .size(12.0)
                    .color(theme::TEXT_DIM),
            );
        });
        return;
    }

    let mut select = None;
    let mut remove = None;
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .max_height(ui.available_height() - 40.0)
        .show(ui, |ui| {
            for i in 0..app.files.len() {
                let active = app.active_file == Some(i);
                let (clicked, removed) = file_row(app, ui, i, active);
                if clicked {
                    select = Some(i);
                }
                if removed {
                    remove = Some(i);
                }
            }
        });

    let ctx = ui.ctx().clone();
    if let Some(i) = remove {
        app.remove_file(&ctx, i);
    } else if let Some(i) = select {
        app.select_file(&ctx, i);
    }

    ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
        let b = crate::icons::button(Icon::Trash, egui::RichText::new("Remove all").size(12.0), theme::TEXT_DIM)
            .fill(Color32::TRANSPARENT);
        if ui
            .add(b)
            .on_hover_text("Close every file. Loops already made stay in Results.")
            .clicked()
        {
            app.remove_all_files();
        }
    });
}

/// One row: thumbnail, name, details and a remove button.
/// Returns (row clicked, remove clicked).
fn file_row(app: &StudioApp, ui: &mut egui::Ui, i: usize, active: bool) -> (bool, bool) {
    let entry = &app.files[i];
    let width = ui.available_width();
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(width, 62.0), Sense::click());
    let hovered = resp.hovered();
    let painter = ui.painter();
    let fill = if active {
        theme::ACCENT_SOFT
    } else if hovered {
        Color32::from_rgb(36, 36, 45)
    } else {
        theme::PANEL_RAISED
    };
    painter.rect_filled(rect, CornerRadius::same(10), fill);
    painter.rect_stroke(
        rect,
        CornerRadius::same(10),
        Stroke::new(1.0, if active { theme::ACCENT } else { theme::BORDER }),
        egui::StrokeKind::Inside,
    );

    // Thumbnail, fitted into a 64x46 box.
    let [tw, th] = entry.thumb.size();
    let box_size = Vec2::new(64.0, 46.0);
    let s = (box_size.x / tw as f32).min(box_size.y / th as f32);
    let thumb_box = Rect::from_min_size(rect.min + Vec2::new(8.0, 8.0), box_size);
    let thumb_rect = Rect::from_center_size(thumb_box.center(), Vec2::new(tw as f32 * s, th as f32 * s));
    painter.rect_filled(thumb_box, CornerRadius::same(6), theme::BG);
    painter.image(
        entry.thumb.id(),
        thumb_rect,
        Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
        Color32::WHITE,
    );
    if entry.is_video {
        let badge = Rect::from_min_size(thumb_box.left_bottom() + Vec2::new(3.0, -17.0), Vec2::splat(14.0));
        painter.rect_filled(badge.expand(1.0), CornerRadius::same(3), Color32::from_black_alpha(170));
        egui::Image::new(Icon::Film.source())
            .tint(Color32::WHITE)
            .paint_at(ui, badge);
    }

    // Text.
    let text_x = thumb_box.right() + 8.0;
    let text_w = rect.right() - 34.0 - text_x;
    let name_color = if active { Color32::WHITE } else { theme::TEXT };
    line(painter, Pos2::new(text_x, rect.top() + 16.0), &entry.name, 13.0, name_color, text_w);
    line(painter, Pos2::new(text_x, rect.top() + 32.0), &entry.detail, 10.5, theme::TEXT_DIM, text_w);
    let scene = if active { app.scene.as_ref() } else { entry.scene.as_ref() };
    if let Some(sc) = scene {
        let mut edits = format!("Area {:.0}%", sc.mask_coverage() * 100.0);
        if !sc.is_video() {
            edits.push_str(&format!(" · {} arrows", sc.arrows.len()));
        }
        line(painter, Pos2::new(text_x, rect.top() + 47.0), &edits, 10.5, theme::TEXT_DIM, text_w);
    }

    // Remove button, drawn over the row so it takes its own clicks.
    let btn_rect = Rect::from_center_size(Pos2::new(rect.right() - 18.0, rect.center().y), Vec2::splat(26.0));
    let show_btn = hovered || active || ui.rect_contains_pointer(btn_rect);
    let mut removed = false;
    if show_btn {
        let btn = egui::Button::image(Icon::Trash.image(15.0, theme::TEXT_DIM))
            .fill(Color32::TRANSPARENT)
            .stroke(Stroke::NONE)
            .corner_radius(CornerRadius::same(6));
        let r = theme::overlay(ui, btn_rect, btn)
            .on_hover_text("Remove this file (loops already made stay in Results)");
        removed = r.clicked();
    }
    let mut menu_remove = false;
    resp.context_menu(|ui| {
        if ui.button("Remove").clicked() {
            menu_remove = true;
            ui.close();
        }
    });
    ui.add_space(2.0);
    (resp.clicked() && !removed, removed || menu_remove)
}

/// One line of text, vertically centered on `pos`, cut with "…" at `width`.
fn line(painter: &egui::Painter, pos: Pos2, text: &str, size: f32, color: Color32, width: f32) {
    let mut job = egui::text::LayoutJob::simple_singleline(text.to_string(), FontId::proportional(size), color);
    job.wrap = egui::text::TextWrapping::truncate_at_width(width.max(10.0));
    let galley = painter.layout_job(job);
    let top_left = Pos2::new(pos.x, pos.y - galley.size().y / 2.0);
    painter.galley(top_left, galley, color);
}
