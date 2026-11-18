//! The central canvas: photo with mask overlay and motion hints, painting
//! and arrow tools, zoom and pan, the floating tool rail and the playback bar.

use eframe::egui::{self, Align2, Color32, CornerRadius, FontId, Pos2, Rect, Sense, Stroke, Vec2};

use crate::app::{Compare, CropDrag, PickTarget, StudioApp, Tool, View};
use crate::crop::{Aspect, CropRect};
use crate::icons::Icon;
use crate::scene::Arrow;
use crate::theme;

pub fn canvas(app: &mut StudioApp, ui: &mut egui::Ui) {
    let (response, painter) = ui.allocate_painter(ui.available_size(), Sense::click_and_drag());
    let area = response.rect;
    draw_dot_grid(&painter, area);

    if app.scene.is_none() {
        empty_state(app, ui, area);
        return;
    }
    // Videos play live under the area being edited.
    let showing_clip = app.view == View::Preview && app.active_clip.is_some();
    let is_live = !showing_clip && {
        let dt = ui.input(|i| i.stable_dt).min(0.1);
        let ctx = ui.ctx().clone();
        app.update_live(&ctx, dt)
    };
    let Some(scene) = &app.scene else { return };

    // What part of the picture is shown: everything while cropping (or with
    // no confirmed crop), otherwise only the kept part.
    let [iw, ih] = scene.work_size();
    let crop_tool = app.view == View::Edit && app.tool == Tool::Crop;
    // Holding \ shows the original for a moment.
    let typing = ui
        .ctx()
        .memory(|m| m.focused())
        .is_some_and(|id| egui::text_edit::TextEditState::load(ui.ctx(), id).is_some());
    let hold_original = !typing && ui.input(|i| i.key_down(egui::Key::Backslash));
    let compare = if crop_tool {
        Compare::Edited
    } else if hold_original {
        Compare::Original
    } else {
        app.compare
    };
    let showing_clip_crop = match (app.view, app.active_clip) {
        (View::Preview, Some(i)) => app.clips.get(i).map(|c| c.recipe.crop),
        _ => None,
    };
    let view_region = if crop_tool || !scene.crop_applied || compare == Compare::Original {
        CropRect::FULL
    } else {
        showing_clip_crop.unwrap_or(scene.crop)
    };
    let view_px = Vec2::new(iw as f32 * view_region.w, ih as f32 * view_region.h);
    // The border preview wraps the shown part.
    let border = if crop_tool || compare == Compare::Original {
        crate::border::Border { enabled: false, ..app.border }
    } else {
        app.border
    };
    let [out_w, out_h, off_x, off_y] = border.layout(view_px.x, view_px.y);

    // Fit the output into the canvas, then apply zoom and pan.
    // Keep clear of the tool rail (left), the view switch (top) and the playback bar (bottom).
    let frame = Rect::from_min_max(area.min + Vec2::new(90.0, 72.0), area.max - Vec2::new(24.0, 84.0));
    let fit = (frame.width() / out_w).min(frame.height() / out_h).max(0.01);
    let scale = fit * app.zoom;
    let out_rect = Rect::from_center_size(frame.center() + app.pan, Vec2::new(out_w, out_h) * scale);
    let img_rect = Rect::from_min_size(out_rect.min + Vec2::new(off_x, off_y) * scale, view_px * scale);
    // Where the whole picture would be; every 0..1 position maps through this.
    let full_rect = Rect::from_min_size(
        img_rect.min - Vec2::new(view_region.x * iw as f32, view_region.y * ih as f32) * scale,
        Vec2::new(iw as f32, ih as f32) * scale,
    );
    let to_norm = |p: Pos2| -> [f32; 2] {
        [(p.x - full_rect.min.x) / full_rect.width(), (p.y - full_rect.min.y) / full_rect.height()]
    };
    let to_screen = |n: [f32; 2]| -> Pos2 {
        Pos2::new(full_rect.min.x + n[0] * full_rect.width(), full_rect.min.y + n[1] * full_rect.height())
    };

    // Zoom with scroll wheel / pinch around the pointer.
    if response.hovered() {
        let (scroll, pinch) = ui.input(|i| (i.smooth_scroll_delta.y, i.zoom_delta()));
        let factor = pinch * (scroll * 0.0015).exp();
        if (factor - 1.0).abs() > 1e-4 {
            let new_zoom = (app.zoom * factor).clamp(0.2, 8.0);
            if let Some(p) = response.hover_pos() {
                let rel = p - (frame.center() + app.pan);
                app.pan += rel * (1.0 - new_zoom / app.zoom);
            }
            app.zoom = new_zoom;
        }
    }

    // Shadow, border, image.
    painter.rect_filled(out_rect.expand(6.0), CornerRadius::same(6), Color32::from_black_alpha(90));
    if border.enabled {
        let [r, g, b] = border.color;
        painter.rect_filled(out_rect, CornerRadius::ZERO, Color32::from_rgb(r, g, b));
    }
    let uv = Rect::from_min_size(Pos2::new(view_region.x, view_region.y), Vec2::new(view_region.w, view_region.h));
    // While a preset is hovered, show the still with that preset instead of the loop.
    let preview_tex = match (app.view, app.active_clip) {
        _ if app.look_preview.is_some() => None,
        (View::Preview, Some(i)) => app.clips.get(i).map(|clip| {
            let n = clip.textures.len().max(1);
            let idx = (app.play_pos as usize).min(n - 1);
            clip.textures[idx].id()
        }),
        _ => None,
    };
    let raw_id = app.raw_tex.as_ref().map(|t| t.id());
    match (compare, preview_tex) {
        (Compare::Original, _) => {
            if let Some(id) = raw_id {
                painter.image(id, img_rect, uv, Color32::WHITE);
            }
        }
        (_, Some(id)) => {
            painter.image(id, img_rect, uv, Color32::WHITE);
        }
        (_, None) => {
            let base = if is_live { app.live_tex.as_ref() } else { app.photo_tex.as_ref() };
            if let Some(t) = base {
                painter.image(t.id(), img_rect, uv, Color32::WHITE);
            }
            if app.show_mask && !crop_tool && compare == Compare::Edited {
                if let Some(t) = &app.mask_tex {
                    painter.image(t.id(), img_rect, uv, Color32::WHITE);
                }
            }
        }
    }
    // Caption preview over the kept part of the picture.
    if !crop_tool && compare != Compare::Original {
        if let Some(t) = &app.caption_tex {
            let uv = Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0));
            if app.caption_on_border {
                // In the border: drawn over the whole output.
                if border.enabled {
                    painter.image(t.id(), out_rect, uv, Color32::WHITE);
                }
            } else {
                let c = scene.crop;
                let r = Rect::from_min_max(to_screen([c.x, c.y]), to_screen([c.x + c.w, c.y + c.h]));
                painter.image(t.id(), r, uv, Color32::WHITE);
            }
        }
    }
    painter.rect_stroke(out_rect, CornerRadius::ZERO, Stroke::new(1.0, theme::BORDER), egui::StrokeKind::Outside);
    // Output size and shape while a border is on.
    if border.enabled {
        let [cw, ch] = {
            let [_, _, w, h] = scene.crop.pixels(scene.full.width(), scene.full.height());
            [w as f32, h as f32]
        };
        let [ow, oh, _, _] = border.layout(cw, ch);
        let text = format!("{:.0} × {:.0} px · {}", ow, oh, crate::panels::ratio_text(ow, oh));
        let pos = out_rect.center_bottom() + Vec2::new(0.0, 8.0);
        let galley = painter.layout_no_wrap(text, FontId::proportional(11.5), Color32::WHITE);
        let bg = Rect::from_center_size(pos + Vec2::new(0.0, galley.size().y / 2.0 + 3.0), galley.size() + Vec2::new(14.0, 6.0));
        painter.rect_filled(bg, CornerRadius::same(6), Color32::from_black_alpha(170));
        painter.galley(bg.center() - galley.size() / 2.0, galley, Color32::WHITE);
    }
    // Split view: the original on the left of the divider.
    if compare == Compare::Split {
        let x = img_rect.left() + app.split.clamp(0.0, 1.0) * img_rect.width();
        if let Some(id) = raw_id {
            let left = Rect::from_min_max(img_rect.min, Pos2::new(x, img_rect.max.y));
            painter.with_clip_rect(left.intersect(area)).image(id, img_rect, uv, Color32::WHITE);
        }
        painter.line_segment(
            [Pos2::new(x, img_rect.top()), Pos2::new(x, img_rect.bottom())],
            Stroke::new(2.0, Color32::WHITE),
        );
        let knob = Pos2::new(x, img_rect.center().y);
        painter.circle_filled(knob, 13.0, Color32::WHITE);
        painter.circle_stroke(knob, 13.0, Stroke::new(1.0, Color32::from_black_alpha(80)));
        for d in [-1.0f32, 1.0] {
            let tip = knob + Vec2::new(d * 7.5, 0.0);
            painter.line_segment([tip, tip + Vec2::new(-d * 4.0, -4.0)], Stroke::new(1.8, theme::BG));
            painter.line_segment([tip, tip + Vec2::new(-d * 4.0, 4.0)], Stroke::new(1.8, theme::BG));
        }
        for (text, pos, align) in [
            ("ORIGINAL", img_rect.left_bottom() + Vec2::new(10.0, -10.0), Align2::LEFT_BOTTOM),
            ("EDITED", img_rect.right_bottom() + Vec2::new(-10.0, -10.0), Align2::RIGHT_BOTTOM),
        ] {
            let g = painter.layout_no_wrap(text.into(), FontId::proportional(11.0), Color32::WHITE);
            let r = align.anchor_size(pos, g.size()).expand2(Vec2::new(7.0, 3.0));
            painter.rect_filled(r, CornerRadius::same(5), Color32::from_black_alpha(160));
            painter.galley(r.min + Vec2::new(7.0, 3.0), g, Color32::WHITE);
        }
    }
    if let Some((_, name)) = &app.look_preview {
        let g = painter.layout_no_wrap(format!("PREVIEW · {name}"), FontId::proportional(11.0), Color32::WHITE);
        let r = Align2::LEFT_TOP.anchor_size(img_rect.left_top() + Vec2::new(10.0, 10.0), g.size()).expand2(Vec2::new(7.0, 3.0));
        painter.rect_filled(r, CornerRadius::same(5), theme::ACCENT.gamma_multiply(0.85));
        painter.galley(r.min + Vec2::new(7.0, 3.0), g, Color32::WHITE);
    }
    if compare == Compare::Original {
        let g = painter.layout_no_wrap("ORIGINAL".into(), FontId::proportional(11.0), Color32::WHITE);
        let r = Align2::LEFT_TOP.anchor_size(img_rect.left_top() + Vec2::new(10.0, 10.0), g.size()).expand2(Vec2::new(7.0, 3.0));
        painter.rect_filled(r, CornerRadius::same(5), Color32::from_black_alpha(160));
        painter.galley(r.min + Vec2::new(7.0, 3.0), g, Color32::WHITE);
    }
    // Without a confirmed crop, shade what the crop will cut away.
    if view_region.is_full() && compare != Compare::Original {
        draw_crop_guide(&painter, full_rect, showing_clip_crop.unwrap_or(scene.crop), crop_tool);
    }
    let painter = painter.with_clip_rect(img_rect.intersect(area));
    if is_live {
        let badge = Rect::from_min_size(img_rect.left_top() + Vec2::new(10.0, 10.0), Vec2::new(54.0, 20.0));
        painter.rect_filled(badge, CornerRadius::same(6), Color32::from_black_alpha(160));
        painter.circle_filled(badge.left_center() + Vec2::new(11.0, 0.0), 4.0, if app.playing { theme::DANGER } else { theme::TEXT_DIM });
        painter.text(
            badge.left_center() + Vec2::new(20.0, 0.0),
            Align2::LEFT_CENTER,
            "LIVE",
            FontId::proportional(11.0),
            Color32::WHITE,
        );
    }

    let editing = app.view == View::Edit;
    let is_video = scene.is_video();
    if editing && app.show_hints && !is_video && app.tool != Tool::Crop && compare == Compare::Edited {
        draw_hints(&painter, &scene.arrows, &scene.pins, &to_screen, scale);
    }

    // ----- Interaction -----
    let middle_drag = response.dragged_by(egui::PointerButton::Middle);
    let space_held = ui.input(|i| i.key_down(egui::Key::Space));
    if let Some(target) = app.picking {
        // Eyedropper: show the color under the pointer, click to use it.
        if ui.input(|i| i.key_pressed(egui::Key::Escape)) || response.secondary_clicked() {
            app.picking = None;
        }
        if let Some(p) = response.hover_pos().filter(|p| img_rect.contains(*p)) {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Crosshair);
            let n = to_norm(p);
            let use_original = compare == Compare::Original
                || (compare == Compare::Split && p.x < img_rect.left() + app.split * img_rect.width());
            let source = if use_original { Some(&scene.work) } else { app.shown_still.as_ref() };
            if let Some(img) = source {
                let c = sample_color(img, n);
                // Swatch next to the pointer.
                let center = p + Vec2::new(26.0, -26.0);
                painter.circle_filled(center, 16.0, Color32::from_rgb(c[0], c[1], c[2]));
                painter.circle_stroke(center, 16.0, Stroke::new(2.5, Color32::WHITE));
                painter.circle_stroke(center, 18.0, Stroke::new(1.0, Color32::from_black_alpha(140)));
                painter.circle_stroke(p, 5.0, Stroke::new(1.5, Color32::WHITE));
                if response.clicked() {
                    match target {
                        PickTarget::Border => {
                            app.border.color = c;
                            app.border.enabled = true;
                        }
                        PickTarget::Caption => app.caption.color = c,
                    }
                    app.picking = None;
                    app.status = format!("Picked color #{:02X}{:02X}{:02X}.", c[0], c[1], c[2]);
                }
            }
        }
    } else if app.tool == Tool::Hand || middle_drag || (space_held && response.dragged()) {
        if response.dragged() {
            app.pan += response.drag_delta();
        }
    } else if compare == Compare::Split {
        // Drag anywhere on the picture to move the divider.
        if let Some(p) = response.interact_pointer_pos() {
            if response.dragged() || response.clicked() {
                app.split = ((p.x - img_rect.left()) / img_rect.width()).clamp(0.0, 1.0);
            }
        }
        if response.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
        }
    } else if compare == Compare::Original {
        // Looking only: no editing on the original.
    } else if editing {
        interact_edit(app, &response, &to_norm, full_rect, scale);
        if app.tool == Tool::Crop && (response.drag_stopped() || response.double_clicked()) {
            if let Some(c) = app.scene.as_ref().map(|s| s.crop) {
                app.set_crop(c);
            }
        }
    } else if response.clicked() || response.drag_started() {
        // Clicking the preview with an editing tool returns to editing.
        app.view = View::Edit;
    }

    // Box being dragged.
    if let (Some(start), Some(p)) = (app.rect_start, response.interact_pointer_pos()) {
        let r = Rect::from_two_pos(to_screen(start), p);
        let subtract = ui.input(|i| i.modifiers.alt);
        let color = if subtract { theme::DANGER } else { theme::MASK_TINT };
        painter.rect_filled(r, CornerRadius::ZERO, color.gamma_multiply(0.18));
        painter.rect_stroke(r, CornerRadius::ZERO, Stroke::new(1.5, color), egui::StrokeKind::Middle);
    }

    // Arrow being drawn.
    if let (Some(start), Some(p)) = (app.arrow_start, response.interact_pointer_pos()) {
        let a = to_screen(start);
        let end = if app.arrow_fixed {
            fixed_arrow_end(start, to_norm(p), app.arrow_len, iw, ih).map(to_screen).unwrap_or(a)
        } else {
            p
        };
        painter.arrow(a, end - a, Stroke::new(2.5, theme::ARROW));
    }

    // Brush cursor.
    if editing && matches!(app.tool, Tool::Brush | Tool::Eraser) {
        if let Some(p) = response.hover_pos() {
            let r = app.brush_radius * scale;
            painter.circle_stroke(p, r, Stroke::new(1.5, Color32::from_white_alpha(200)));
            painter.circle_stroke(p, r * app.brush_hardness.max(0.05), Stroke::new(1.0, Color32::from_white_alpha(70)));
            ui.ctx().set_cursor_icon(egui::CursorIcon::Crosshair);
        }
    } else if editing && app.tool == Tool::Rect && response.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::Crosshair);
    } else if app.tool == Tool::Hand && response.hovered() {
        ui.ctx().set_cursor_icon(if response.dragged() {
            egui::CursorIcon::Grabbing
        } else {
            egui::CursorIcon::Grab
        });
    }

    tool_rail(app, ui, area);
    view_switch(app, ui, area);
    compare_switch(app, ui, area);
    if crop_tool {
        crop_bar(app, ui, area);
    }
    if app.view == View::Preview && app.active_clip.is_some() {
        playback_bar(app, ui, area);
    } else if app.scene.as_ref().is_some_and(|s| s.is_video()) {
        video_bar(app, ui, area);
    }
}

fn interact_edit(
    app: &mut StudioApp,
    response: &egui::Response,
    to_norm: &dyn Fn(Pos2) -> [f32; 2],
    img_rect: Rect,
    _scale: f32,
) {
    let Some(scene) = app.scene.as_mut() else { return };
    let primary = response.dragged_by(egui::PointerButton::Primary)
        || response.clicked_by(egui::PointerButton::Primary)
        || response.drag_started_by(egui::PointerButton::Primary);
    let pointer = response.interact_pointer_pos();
    match app.tool {
        Tool::Brush | Tool::Eraser => {
            let erase = app.tool == Tool::Eraser;
            if primary {
                if let Some(p) = pointer {
                    // First dab of a stroke: save one undo step for the whole stroke.
                    if app.stroke_last.is_none() {
                        scene.checkpoint();
                    }
                    let n = to_norm(p);
                    let [w, _] = scene.work_size();
                    // Interpolate dabs so fast strokes stay continuous.
                    let spacing = (app.brush_radius * 0.3).max(1.0) / w as f32;
                    match app.stroke_last {
                        Some(last) => {
                            let d = ((n[0] - last[0]).powi(2) + (n[1] - last[1]).powi(2)).sqrt();
                            let steps = (d / spacing).ceil().max(1.0) as usize;
                            for k in 1..=steps {
                                let t = k as f32 / steps as f32;
                                let q = [last[0] + (n[0] - last[0]) * t, last[1] + (n[1] - last[1]) * t];
                                scene.dab(q, app.brush_radius, app.brush_hardness, erase);
                            }
                        }
                        None => scene.dab(n, app.brush_radius, app.brush_hardness, erase),
                    }
                    app.stroke_last = Some(n);
                }
            }
            let button_down = response.ctx.input(|i| i.pointer.primary_down());
            if response.drag_stopped() || response.clicked() || !button_down {
                app.stroke_last = None;
            }
        }
        Tool::Arrow => {
            if response.drag_started_by(egui::PointerButton::Primary) {
                if let Some(p) = pointer {
                    app.arrow_start = Some(to_norm(p));
                }
            }
            if response.drag_stopped() {
                if let (Some(start), Some(p)) = (app.arrow_start.take(), pointer) {
                    let [iw, ih] = scene.work_size();
                    let end = if app.arrow_fixed {
                        fixed_arrow_end(start, to_norm(p), app.arrow_len, iw, ih).unwrap_or(start)
                    } else {
                        to_norm(p)
                    };
                    let len_px = ((end[0] - start[0]) * img_rect.width()).hypot((end[1] - start[1]) * img_rect.height());
                    if len_px > 8.0 {
                        scene.checkpoint();
                        scene.arrows.push(Arrow { from: start, to: end });
                        if scene.mask_at(start) < 0.3 {
                            app.status = "That arrow starts outside the moving region; paint the region so it has an effect.".into();
                        }
                    }
                }
            }
            if response.secondary_clicked() {
                if let Some(p) = pointer {
                    scene.checkpoint();
                    scene.remove_hint_near(to_norm(p), 0.03);
                }
            }
        }
        Tool::Pin => {
            if response.clicked_by(egui::PointerButton::Primary) {
                if let Some(p) = pointer {
                    scene.checkpoint();
                    scene.pins.push(to_norm(p));
                }
            }
            if response.secondary_clicked() {
                if let Some(p) = pointer {
                    scene.checkpoint();
                    scene.remove_hint_near(to_norm(p), 0.03);
                }
            }
        }
        Tool::Rect => {
            if response.drag_started_by(egui::PointerButton::Primary) {
                if let Some(p) = pointer {
                    app.rect_start = Some(to_norm(p));
                }
            }
            if response.drag_stopped() {
                if let (Some(start), Some(p)) = (app.rect_start.take(), pointer) {
                    let end = to_norm(p);
                    let subtract = response.ctx.input(|i| i.modifiers.alt);
                    scene.checkpoint();
                    let feather = app.brush_radius * (1.0 - app.brush_hardness) * 0.5;
                    scene.fill_rect(start, end, if subtract { 0.0 } else { 1.0 }, feather);
                }
            }
        }
        Tool::Crop => crop_interact(&mut app.crop_drag, &mut app.crop_before, scene, response, to_norm, img_rect),
        Tool::Hand | Tool::Select => {}
    }
}

/// Drag corners to resize, drag inside to move, drag outside to draw a new
/// box, double-click to reset. The aspect ratio stays locked unless Free.
fn crop_interact(
    drag: &mut Option<CropDrag>,
    before: &mut Option<CropRect>,
    scene: &mut crate::scene::Scene,
    response: &egui::Response,
    to_norm: &dyn Fn(Pos2) -> [f32; 2],
    img_rect: Rect,
) {
    let (iw, ih) = (scene.full.width(), scene.full.height());
    let aspect = scene.aspect;
    let pointer = response.interact_pointer_pos().or(response.hover_pos());
    let to_screen = |n: [f32; 2]| {
        Pos2::new(img_rect.min.x + n[0] * img_rect.width(), img_rect.min.y + n[1] * img_rect.height())
    };
    let r = scene.crop;
    let corners = [[r.x, r.y], [r.x + r.w, r.y], [r.x, r.y + r.h], [r.x + r.w, r.y + r.h]];

    // Cursor feedback while hovering.
    if drag.is_none() {
        if let Some(p) = response.hover_pos() {
            let n = to_norm(p);
            let near_corner = corners.iter().position(|c| to_screen(*c).distance(p) < 14.0);
            let inside = n[0] > r.x && n[0] < r.x + r.w && n[1] > r.y && n[1] < r.y + r.h;
            let icon = match near_corner {
                Some(0) | Some(3) => egui::CursorIcon::ResizeNwSe,
                Some(_) => egui::CursorIcon::ResizeNeSw,
                None if inside && !r.is_full() => egui::CursorIcon::Grab,
                None => egui::CursorIcon::Crosshair,
            };
            response.ctx.set_cursor_icon(icon);
        }
    }

    if response.double_clicked() {
        scene.checkpoint();
        scene.crop = match aspect.ratio(iw, ih) {
            Some(ratio) => CropRect::largest(ratio, [0.5, 0.5], iw, ih),
            None => CropRect::FULL,
        };
        *drag = None;
        return;
    }

    if response.drag_started_by(egui::PointerButton::Primary) {
        if let Some(p) = response.interact_pointer_pos() {
            let n = to_norm(p);
            *before = Some(r);
            scene.checkpoint();
            let corner = corners.iter().position(|c| to_screen(*c).distance(p) < 14.0);
            let tol_x = 10.0 / img_rect.width();
            let tol_y = 10.0 / img_rect.height();
            let within_y = n[1] > r.y && n[1] < r.y + r.h;
            let within_x = n[0] > r.x && n[0] < r.x + r.w;
            let side = if aspect != Aspect::Free || r.is_full() {
                None
            } else if (n[0] - r.x).abs() < tol_x && within_y {
                Some(0)
            } else if (n[0] - (r.x + r.w)).abs() < tol_x && within_y {
                Some(1)
            } else if (n[1] - r.y).abs() < tol_y && within_x {
                Some(2)
            } else if (n[1] - (r.y + r.h)).abs() < tol_y && within_x {
                Some(3)
            } else {
                None
            };
            *drag = Some(if let Some(i) = corner.filter(|_| !r.is_full()) {
                CropDrag::Corner { anchor: corners[3 - i] }
            } else if let Some(side) = side {
                CropDrag::Side { side, start: r }
            } else if within_x && within_y && !r.is_full() {
                CropDrag::Move { grab: n, start: r }
            } else {
                CropDrag::New { anchor: [n[0].clamp(0.0, 1.0), n[1].clamp(0.0, 1.0)] }
            });
        }
    }

    if let (Some(d), Some(p)) = (*drag, pointer) {
        if response.dragged_by(egui::PointerButton::Primary) {
            let n = to_norm(p);
            scene.crop = match d {
                CropDrag::Corner { anchor } | CropDrag::New { anchor } => {
                    CropRect::from_anchor(anchor, n, aspect, iw, ih)
                }
                CropDrag::Move { grab, start } => start.moved(n[0] - grab[0], n[1] - grab[1]),
                CropDrag::Side { side, start } => {
                    let (x0, x1) = (start.x, start.x + start.w);
                    let (y0, y1) = (start.y, start.y + start.h);
                    match side {
                        0 => CropRect::from_corners([n[0].min(x1 - 0.02), y0], [x1, y1]),
                        1 => CropRect::from_corners([x0, y0], [n[0].max(x0 + 0.02), y1]),
                        2 => CropRect::from_corners([x0, n[1].min(y1 - 0.02)], [x1, y1]),
                        _ => CropRect::from_corners([x0, y0], [x1, n[1].max(y0 + 0.02)]),
                    }
                }
            };
        }
    }

    if response.drag_stopped() {
        *drag = None;
        // A tiny box is almost always a slip: keep the previous one.
        let px_w = scene.crop.w * img_rect.width();
        let px_h = scene.crop.h * img_rect.height();
        if px_w < 12.0 || px_h < 12.0 {
            if let Some(b) = before.take() {
                scene.crop = b;
                scene.discard_checkpoint();
            }
        } else if before.take() == Some(scene.crop) {
            // A click without a change is not an undo step.
            scene.discard_checkpoint();
        }
    }
}

/// End of an arrow of the set length (fraction of the picture width),
/// pointing from `start` towards `toward`. `None` when there is no direction.
pub fn fixed_arrow_end(start: [f32; 2], toward: [f32; 2], len: f32, iw: u32, ih: u32) -> Option<[f32; 2]> {
    let (w, h) = (iw as f32, ih as f32);
    let d = [(toward[0] - start[0]) * w, (toward[1] - start[1]) * h];
    let n = (d[0] * d[0] + d[1] * d[1]).sqrt();
    if n < 1.0 {
        return None;
    }
    let l = len * w;
    Some([start[0] + d[0] / n * l / w, start[1] + d[1] / n * l / h])
}

/// Top-right switch: edited picture, original, or a split view with a slider.
fn compare_switch(app: &mut StudioApp, ui: &mut egui::Ui, area: Rect) {
    egui::Area::new(egui::Id::unique("compare_switch"))
        .pivot(Align2::RIGHT_TOP)
        .fixed_pos(Pos2::new(area.right() - 16.0, area.top() + 14.0))
        .order(egui::Order::Foreground)
        .show(ui.ctx(), |ui| {
            floating_frame().show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing = Vec2::splat(4.0);
                    for (mode, label, tip) in [
                        (Compare::Edited, "Edited", "Edited picture"),
                        (Compare::Original, "Original", "Original photo without edits. Hold \\ to show it briefly."),
                        (Compare::Split, "Split", "Original on the left, edited on the right; drag on the picture to move the divider"),
                    ] {
                        let sel = app.compare == mode;
                        let b = egui::Button::new(egui::RichText::new(label).size(12.5).color(if sel {
                            Color32::WHITE
                        } else {
                            theme::TEXT_DIM
                        }))
                        .fill(if sel { theme::ACCENT } else { Color32::TRANSPARENT })
                        .stroke(Stroke::NONE)
                        .corner_radius(CornerRadius::same(8))
                        .min_size(Vec2::new(0.0, 28.0));
                        if ui.add(b).on_hover_text(tip).clicked() {
                            app.set_compare(mode);
                        }
                    }
                });
                if app.compare == Compare::Split {
                    // Divider position on its own row so the switch stays narrow.
                    ui.spacing_mut().slider_width = ui.available_width() - 8.0;
                    ui.add(egui::Slider::new(&mut app.split, 0.0..=1.0).show_value(false))
                        .on_hover_text("Where the divider sits (or drag on the picture)");
                }
            });
        });
}

/// Average color of a small patch around a 0..1 position (smooths out grain).
pub fn sample_color(img: &image::RgbaImage, n: [f32; 2]) -> [u8; 3] {
    let (w, h) = img.dimensions();
    let cx = ((n[0] * w as f32) as i32).clamp(0, w as i32 - 1);
    let cy = ((n[1] * h as f32) as i32).clamp(0, h as i32 - 1);
    let mut sum = [0u32; 3];
    let mut count = 0;
    for dy in -1..=1 {
        for dx in -1..=1 {
            let x = (cx + dx).clamp(0, w as i32 - 1) as u32;
            let y = (cy + dy).clamp(0, h as i32 - 1) as u32;
            let p = img.get_pixel(x, y);
            for k in 0..3 {
                sum[k] += p[k] as u32;
            }
            count += 1;
        }
    }
    [(sum[0] / count) as u8, (sum[1] / count) as u8, (sum[2] / count) as u8]
}

/// Floating bar shown while cropping: confirm or start over.
fn crop_bar(app: &mut StudioApp, ui: &mut egui::Ui, area: Rect) {
    egui::Area::new(egui::Id::unique("crop_bar"))
        .pivot(Align2::CENTER_BOTTOM)
        .fixed_pos(Pos2::new(area.center().x, area.bottom() - 18.0))
        .order(egui::Order::Foreground)
        .show(ui.ctx(), |ui| {
            floating_frame().show(ui, |ui| {
                ui.horizontal(|ui| {
                    let label = app
                        .scene
                        .as_ref()
                        .map(|s| {
                            let [_, _, w, h] = s.crop.pixels(s.full.width(), s.full.height());
                            format!("{} · {w} × {h}", s.aspect.label())
                        })
                        .unwrap_or_default();
                    ui.label(egui::RichText::new(label).color(theme::TEXT_DIM).size(12.0));
                    if ui.button("Reset").on_hover_text("Keep the whole picture").clicked() {
                        if let Some(s) = &mut app.scene {
                            s.checkpoint();
                        }
                        app.set_crop(CropRect::FULL);
                    }
                    let done = crate::icons::button(
                        Icon::Crop,
                        egui::RichText::new("Done").color(Color32::WHITE).strong(),
                        Color32::WHITE,
                    )
                    .fill(theme::ACCENT)
                    .min_size(Vec2::new(90.0, 30.0));
                    if ui.add(done).on_hover_text("Confirm the crop and hide the cut-away part (Enter)").clicked() {
                        app.finish_crop();
                    }
                });
            });
        });
}

fn draw_hints(
    painter: &egui::Painter,
    arrows: &[Arrow],
    pins: &[[f32; 2]],
    to_screen: &dyn Fn([f32; 2]) -> Pos2,
    _scale: f32,
) {
    for a in arrows {
        let p = to_screen(a.from);
        let q = to_screen(a.to);
        painter.arrow(p, q - p, Stroke::new(4.5, Color32::from_black_alpha(140)));
        painter.arrow(p, q - p, Stroke::new(2.5, theme::ARROW));
        painter.circle_filled(p, 3.5, theme::ARROW);
    }
    for pin in pins {
        let p = to_screen(*pin);
        painter.circle_filled(p, 7.0, Color32::from_black_alpha(140));
        painter.circle_filled(p, 5.0, theme::PIN);
        painter.circle_filled(p, 2.0, Color32::from_rgb(20, 60, 40));
    }
}

/// Darken what the crop will cut away. With `handles`, also draw the
/// thirds grid and corner grips of the crop tool.
fn draw_crop_guide(painter: &egui::Painter, img_rect: Rect, crop: CropRect, handles: bool) {
    if crop.is_full() && !handles {
        return;
    }
    let keep = Rect::from_min_size(
        Pos2::new(img_rect.min.x + crop.x * img_rect.width(), img_rect.min.y + crop.y * img_rect.height()),
        Vec2::new(crop.w * img_rect.width(), crop.h * img_rect.height()),
    );
    let shade = Color32::from_black_alpha(if handles { 170 } else { 140 });
    for r in [
        Rect::from_min_max(img_rect.min, Pos2::new(img_rect.max.x, keep.min.y)),
        Rect::from_min_max(Pos2::new(img_rect.min.x, keep.max.y), img_rect.max),
        Rect::from_min_max(Pos2::new(img_rect.min.x, keep.min.y), Pos2::new(keep.min.x, keep.max.y)),
        Rect::from_min_max(Pos2::new(keep.max.x, keep.min.y), Pos2::new(img_rect.max.x, keep.max.y)),
    ] {
        if r.width() > 0.0 && r.height() > 0.0 {
            painter.rect_filled(r, CornerRadius::ZERO, shade);
        }
    }
    let line = Stroke::new(1.0, Color32::from_white_alpha(if handles { 220 } else { 150 }));
    painter.rect_stroke(keep, CornerRadius::ZERO, line, egui::StrokeKind::Middle);
    if !handles {
        return;
    }
    let thirds = Stroke::new(1.0, Color32::from_white_alpha(70));
    for k in 1..3 {
        let x = keep.min.x + keep.width() * k as f32 / 3.0;
        let y = keep.min.y + keep.height() * k as f32 / 3.0;
        painter.line_segment([Pos2::new(x, keep.min.y), Pos2::new(x, keep.max.y)], thirds);
        painter.line_segment([Pos2::new(keep.min.x, y), Pos2::new(keep.max.x, y)], thirds);
    }
    let grip = Stroke::new(3.5, Color32::WHITE);
    let len = 14.0f32.min(keep.width() / 3.0).min(keep.height() / 3.0);
    for (c, dx, dy) in [
        (keep.left_top(), 1.0, 1.0),
        (keep.right_top(), -1.0, 1.0),
        (keep.left_bottom(), 1.0, -1.0),
        (keep.right_bottom(), -1.0, -1.0),
    ] {
        painter.line_segment([c, c + Vec2::new(dx * len, 0.0)], grip);
        painter.line_segment([c, c + Vec2::new(0.0, dy * len)], grip);
    }
}

fn draw_dot_grid(painter: &egui::Painter, rect: Rect) {
    painter.rect_filled(rect, CornerRadius::ZERO, theme::BG);
    let step = 24.0;
    let color = Color32::from_rgb(30, 30, 37);
    let mut y = rect.top() + step / 2.0;
    while y < rect.bottom() {
        let mut x = rect.left() + step / 2.0;
        while x < rect.right() {
            painter.circle_filled(Pos2::new(x, y), 1.0, color);
            x += step;
        }
        y += step;
    }
}

fn floating_frame() -> egui::Frame {
    egui::Frame::default()
        .fill(theme::PANEL_RAISED)
        .stroke(Stroke::new(1.0, theme::BORDER))
        .corner_radius(CornerRadius::same(12))
        .inner_margin(egui::Margin::same(6))
        .shadow(egui::Shadow {
            offset: [0, 4],
            blur: 16,
            spread: 0,
            color: Color32::from_black_alpha(110),
        })
}

fn tool_button(ui: &mut egui::Ui, icon: Icon, tip: &str, selected: bool) -> bool {
    let fill = if selected { theme::ACCENT } else { Color32::TRANSPARENT };
    let tint = if !ui.is_enabled() {
        theme::TEXT_DIM.gamma_multiply(0.5)
    } else if selected {
        Color32::WHITE
    } else {
        theme::TEXT
    };
    let btn = egui::Button::image(icon.image(19.0, tint))
    .fill(fill)
    .stroke(Stroke::NONE)
    .corner_radius(CornerRadius::same(8))
    .min_size(Vec2::splat(34.0));
    ui.add(btn).on_hover_text(tip).clicked()
}

fn tool_rail(app: &mut StudioApp, ui: &mut egui::Ui, area: Rect) {
    let pos = Pos2::new(area.left() + 14.0, area.top() + 70.0);
    egui::Area::new(egui::Id::unique("tool_rail"))
        .fixed_pos(pos)
        .order(egui::Order::Foreground)
        .show(ui.ctx(), |ui| {
            floating_frame().show(ui, |ui| {
                ui.spacing_mut().item_spacing = Vec2::splat(4.0);
                ui.vertical(|ui| {
                    let is_video = app.scene.as_ref().is_some_and(|s| s.is_video());
                    let tools = [
                        (Tool::Select, Icon::Pointer, "Pointer (V)\nSelect without editing"),
                        (Tool::Rect, Icon::Box, "Drag a box to select an area (R)\nHold Alt/Option to remove a box"),
                        (Tool::Brush, Icon::Brush, "Paint the selected area (B)"),
                        (Tool::Eraser, Icon::Eraser, "Erase from the selected area (E)"),
                        (Tool::Arrow, Icon::Arrow, "Draw motion arrow (A)\nRight-click an arrow to remove it"),
                        (Tool::Pin, Icon::Pin, "Pin a point that should stay still (P)"),
                        (Tool::Crop, Icon::Crop, "Crop (C)\nDrag corners to resize, drag inside to move,\ndouble-click to reset. Pick a shape in the Crop card."),
                        (Tool::Hand, Icon::Hand, "Pan the view (H, or hold Space)"),
                    ];
                    for (tool, icon, tip) in tools {
                        if is_video && matches!(tool, Tool::Arrow | Tool::Pin) {
                            continue;
                        }
                        if tool_button(ui, icon, tip, app.tool == tool) {
                            app.pick_tool(tool);
                        }
                    }
                    ui.add_space(2.0);
                    ui.separator();
                    let can_undo = app.scene.as_ref().is_some_and(|s| s.can_undo());
                    let can_redo = app.scene.as_ref().is_some_and(|s| s.can_redo());
                    ui.add_enabled_ui(can_undo, |ui| {
                        if tool_button(ui, Icon::Undo, "Undo (Cmd/Ctrl+Z)", false) {
                            let ctx = ui.ctx().clone();
                            app.undo(&ctx);
                        }
                    });
                    ui.add_enabled_ui(can_redo, |ui| {
                        if tool_button(ui, Icon::Redo, "Redo (Cmd/Ctrl+Shift+Z)", false) {
                            let ctx = ui.ctx().clone();
                            app.redo(&ctx);
                        }
                    });
                    // View group: zoom in, current zoom, zoom out, fit.
                    ui.add_space(2.0);
                    ui.separator();
                    if tool_button(ui, Icon::ZoomIn, "Zoom in\n(or scroll / pinch on the picture)", false) {
                        app.zoom = (app.zoom * 1.25).min(8.0);
                    }
                    let pct = egui::Label::new(
                        egui::RichText::new(format!("{:.0}%", app.zoom * 100.0)).size(10.5).color(theme::TEXT_DIM),
                    )
                    .sense(Sense::click());
                    ui.vertical_centered(|ui| {
                        if ui.add(pct).on_hover_text("Click to show at fitted size").clicked() {
                            app.zoom = 1.0;
                            app.pan = Vec2::ZERO;
                        }
                    });
                    if tool_button(ui, Icon::ZoomOut, "Zoom out", false) {
                        app.zoom = (app.zoom / 1.25).max(0.2);
                    }
                    let fitted = (app.zoom - 1.0).abs() < 1e-3 && app.pan.length() < 0.5;
                    ui.add_enabled_ui(!fitted, |ui| {
                        if tool_button(
                            ui,
                            Icon::Fit,
                            "Fit picture to window\nResets zoom and moves the picture back to the center",
                            false,
                        ) {
                            app.zoom = 1.0;
                            app.pan = Vec2::ZERO;
                        }
                    });
                });
            });
        });
}

fn view_switch(app: &mut StudioApp, ui: &mut egui::Ui, area: Rect) {
    egui::Area::new(egui::Id::unique("view_switch"))
        .pivot(Align2::CENTER_TOP)
        .fixed_pos(Pos2::new(area.center().x, area.top() + 14.0))
        .order(egui::Order::Foreground)
        .show(ui.ctx(), |ui| {
            floating_frame().show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing = Vec2::splat(4.0);
                    let has_clip = app.active_clip.is_some();
                    for (view, icon, label) in [(View::Edit, Icon::Edit, "Edit"), (View::Preview, Icon::Play, "Preview")] {
                        let sel = app.view == view;
                        let enabled = view == View::Edit || has_clip;
                        let color = if sel { Color32::WHITE } else { theme::TEXT_DIM };
                        let btn = crate::icons::button(icon, egui::RichText::new(label).color(color), color)
                        .fill(if sel { theme::ACCENT } else { Color32::TRANSPARENT })
                        .stroke(Stroke::NONE)
                        .corner_radius(CornerRadius::same(8))
                        .min_size(Vec2::new(96.0, 30.0));
                        if ui.add_enabled(enabled, btn).clicked() {
                            app.view = view;
                        }
                    }
                });
            });
        });
}

fn playback_bar(app: &mut StudioApp, ui: &mut egui::Ui, area: Rect) {
    let Some(clip) = app.active_clip.and_then(|i| app.clips.get(i)) else { return };
    let n = clip.textures.len();
    let fps = clip.recipe.fps as f32;
    let label = format!("Loop {}", clip.number);
    // Preview quality: the loop is animated at the editing size; export
    // animates it again at the size picked in Export.
    let (pw, ph) = clip.frames.first().map(|f| f.dimensions()).unwrap_or((0, 0));
    let quality = format!("{} px · {} fps", pw.max(ph), clip.recipe.fps);
    let quality_tip = format!(
        "Preview quality: {pw} × {ph} px, {} frames a second.\n\
         The preview is made small so it is quick. Export makes the loop again \
         at the size chosen in Export (up to the full photo).",
        clip.recipe.fps
    );
    if app.playing {
        let dt = ui.input(|i| i.stable_dt).min(0.1);
        app.play_pos = (app.play_pos + dt * fps) % n as f32;
        ui.ctx().request_repaint();
    }
    let width = 490.0;
    let pos = Pos2::new(area.center().x - width / 2.0, area.bottom() - 62.0);
    egui::Area::new(egui::Id::unique("playback"))
        .fixed_pos(pos)
        .order(egui::Order::Foreground)
        .show(ui.ctx(), |ui| {
            floating_frame().show(ui, |ui| {
                ui.set_width(width - 12.0);
                ui.horizontal(|ui| {
                    let icon = if app.playing { Icon::Pause } else { Icon::Play };
                    if tool_button(ui, icon, "Play / pause (Space)", false) {
                        app.playing = !app.playing;
                    }
                    let mut frame = app.play_pos.floor();
                    ui.spacing_mut().slider_width = width - 280.0;
                    let r = ui.add(egui::Slider::new(&mut frame, 0.0..=(n - 1) as f32).show_value(false).step_by(1.0));
                    if r.changed() {
                        app.play_pos = frame;
                        app.playing = false;
                    }
                    ui.label(
                        egui::RichText::new(format!("{label}  {}/{}", frame as usize + 1, n))
                            .color(theme::TEXT_DIM)
                            .size(12.0),
                    );
                    ui.separator();
                    ui.label(egui::RichText::new(quality).color(theme::TEXT_DIM).size(12.0))
                        .on_hover_text(quality_tip);
                });
            });
        });
}

/// Time bar for an imported video while editing: play / pause, a slider
/// over the part in use (the trim range), the time, and a button that makes
/// the frame shown the frozen frame (the one the still parts show).
fn video_bar(app: &mut StudioApp, ui: &mut egui::Ui, area: Rect) {
    let Some(scene) = &app.scene else { return };
    let Some(video) = &scene.video else { return };
    let fps = video.fps.max(1.0);
    let total = video.frames.len();
    let still = scene.still_index;
    let [start, end] = app.trim_range();
    let Some(idx) = app.live_index() else { return };
    let width = 545.0;
    let pos = Pos2::new(area.center().x - width / 2.0, area.bottom() - 62.0);
    egui::Area::new(egui::Id::unique("video_time_bar"))
        .fixed_pos(pos)
        .order(egui::Order::Foreground)
        .show(ui.ctx(), |ui| {
            floating_frame().show(ui, |ui| {
                ui.set_width(width - 12.0);
                ui.horizontal(|ui| {
                    let icon = if app.playing && app.live { Icon::Pause } else { Icon::Play };
                    if tool_button(ui, icon, "Play / pause (Space)", false) {
                        if app.live {
                            app.playing = !app.playing;
                        } else {
                            app.live = true;
                            app.playing = true;
                        }
                    }
                    let mut frame = idx as f32;
                    ui.spacing_mut().slider_width = 260.0;
                    let r = ui
                        .add(
                            egui::Slider::new(&mut frame, start as f32..=end as f32)
                                .show_value(false)
                                .step_by(1.0),
                        )
                        .on_hover_text(
                            "Drag to move through the video. The blue dot marks the frozen frame. \
                             Set the part used with Trim in the Video card.",
                        );
                    // Mark where the frozen frame sits on the slider.
                    if (start..=end).contains(&still) {
                        let rail = r.rect.shrink2(Vec2::new(r.rect.height() / 2.0, 0.0));
                        let x = rail.left() + rail.width() * (still - start) as f32 / (end - start).max(1) as f32;
                        ui.painter().circle_filled(Pos2::new(x, r.rect.bottom() + 1.0), 2.5, theme::ACCENT);
                    }
                    if r.changed() {
                        app.seek_live(frame.round() as usize);
                    }
                    ui.label(
                        egui::RichText::new(format!("{:.2}s / {:.2}s", idx as f32 / fps, total as f32 / fps))
                            .color(theme::TEXT_DIM)
                            .size(12.0),
                    );
                    ui.separator();
                    let is_still = idx == still;
                    if ui
                        .add_enabled(!is_still, crate::icons::button(Icon::Snow, "Freeze this frame", theme::TEXT))
                        .on_hover_text(
                            "In a cinemagraph the still parts of the picture show one single frame of the video. \
                             Click to use the frame you see now for them. The blue dot on the slider marks it.",
                        )
                        .on_disabled_hover_text("The still parts already show this frame (the blue dot).")
                        .clicked()
                    {
                        let ctx = ui.ctx().clone();
                        app.set_still(&ctx, idx);
                        app.status = format!("The still parts now show the frame at {:.2}s.", idx as f32 / fps);
                    }
                });
            });
        });
}

fn empty_state(app: &mut StudioApp, ui: &mut egui::Ui, area: Rect) {
    let size = Vec2::new(420.0, 250.0);
    let rect = Rect::from_center_size(area.center(), size);
    let painter = ui.painter();
    painter.rect_filled(rect, CornerRadius::same(16), theme::PANEL);
    painter.rect_stroke(
        rect.shrink(8.0),
        CornerRadius::same(12),
        Stroke::new(1.5, Color32::from_rgb(58, 58, 72)),
        egui::StrokeKind::Inside,
    );
    let mut child = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(rect.shrink(28.0))
            .layout(egui::Layout::top_down(egui::Align::Center)),
    );
    child.add_space(6.0);
    child.add(Icon::Image.image(44.0, theme::TEXT_DIM));
    child.label(egui::RichText::new("Open a photo or video").size(19.0).strong());
    child.label(
        egui::RichText::new("Drop a photo or a short video here.\nSky, water and smoke animate best.")
            .color(theme::TEXT_DIM),
    );
    child.add_space(10.0);
    child.horizontal(|ui| {
        ui.add_space(52.0);
        let open = egui::Button::new(egui::RichText::new("Open file…").color(Color32::WHITE).strong())
            .fill(theme::ACCENT)
            .min_size(Vec2::new(140.0, 34.0));
        if ui.add(open).clicked() {
            let ctx = ui.ctx().clone();
            app.open_photo_dialog(&ctx);
        }
        if ui.add(egui::Button::new("Try the sample").min_size(Vec2::new(140.0, 34.0))).clicked() {
            let ctx = ui.ctx().clone();
            app.load_sample(&ctx);
        }
    });
}

#[cfg(test)]
mod tests {
    #[test]
    fn eyedropper_samples_the_patch_under_the_pointer() {
        let mut img = image::RgbaImage::from_pixel(10, 10, image::Rgba([0, 0, 0, 255]));
        for y in 0..10 {
            for x in 5..10 {
                img.put_pixel(x, y, image::Rgba([200, 100, 50, 255]));
            }
        }
        assert_eq!(super::sample_color(&img, [0.8, 0.5]), [200, 100, 50]);
        assert_eq!(super::sample_color(&img, [0.1, 0.5]), [0, 0, 0]);
    }

    #[test]
    fn fixed_arrows_have_the_same_pixel_length_in_any_direction() {
        let (w, h) = (1280u32, 720u32);
        for toward in [[0.9, 0.5], [0.5, 0.95], [0.1, 0.1], [0.52, 0.48]] {
            let end = super::fixed_arrow_end([0.5, 0.5], toward, 0.12, w, h).unwrap();
            let len = ((end[0] - 0.5) * w as f32).hypot((end[1] - 0.5) * h as f32);
            assert!((len - 0.12 * w as f32).abs() < 0.01, "{len}");
        }
        assert!(super::fixed_arrow_end([0.5, 0.5], [0.5, 0.5], 0.12, w, h).is_none());
    }
}
