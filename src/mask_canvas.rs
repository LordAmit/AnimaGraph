//! The selected mask on the canvas: a red overlay where it applies, the
//! gradient guides and handles, and the brush. Drag to paint a brush mask
//! (hold Alt to erase); drag a handle to move it, or drag elsewhere to draw
//! the gradient again.

use eframe::egui::{self, Color32, Pos2, Rect, Stroke, TextureOptions, Vec2};

use crate::app::StudioApp;
use crate::mask::{Mask, Shape};
use crate::theme;

/// What a drag on the canvas is changing.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Handle {
    Start,
    End,
    Center,
    RadiusX,
    RadiusY,
    /// Drawing a new gradient from this point.
    New([f32; 2]),
    Paint,
}

/// Overlay size (longest side).
const OVERLAY: u32 = 512;

/// Make the overlay texture for the selected mask when it changed.
pub fn refresh_overlay(app: &mut StudioApp, ctx: &egui::Context) {
    let mask = app
        .mask_edit
        .and_then(|i| app.scene.as_ref().and_then(|s| s.edits.masks.get(i)))
        .filter(|_| app.mask_overlay)
        .cloned();
    let Some(mask) = mask else {
        app.mask_overlay_tex = None;
        app.mask_overlay_of = None;
        return;
    };
    if app.mask_overlay_of.as_ref() == Some(&mask) && app.mask_overlay_tex.is_some() {
        return;
    }
    let Some(scene) = &app.scene else { return };
    let src = app.shown_still.as_ref().unwrap_or(&scene.work);
    let small = crate::scene::downscale(src, OVERLAY);
    let w = mask.weights(&small);
    let pixels: Vec<Color32> =
        w.iter().map(|&a| Color32::from_rgba_unmultiplied(255, 60, 60, (a.clamp(0.0, 1.0) * 150.0) as u8)).collect();
    let img = egui::ColorImage::new([small.width() as usize, small.height() as usize], pixels);
    match &mut app.mask_overlay_tex {
        Some(t) => t.set(img, TextureOptions::LINEAR),
        None => app.mask_overlay_tex = Some(ctx.load_texture("mask_overlay", img, TextureOptions::LINEAR)),
    }
    app.mask_overlay_of = Some(mask);
}

/// Overlay, guides and handles of the selected mask. `full` is where the
/// whole picture is on screen.
pub fn draw(app: &StudioApp, painter: &egui::Painter, full: Rect, pointer: Option<Pos2>) {
    let Some(mask) = app.mask_edit.and_then(|i| app.scene.as_ref().and_then(|s| s.edits.masks.get(i))) else { return };
    if let Some(t) = &app.mask_overlay_tex {
        painter.image(t.id(), full, Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), Color32::WHITE);
    }
    let to_screen = |n: [f32; 2]| Pos2::new(full.min.x + n[0] * full.width(), full.min.y + n[1] * full.height());
    let line = Stroke::new(1.5, Color32::WHITE);
    let shadow = Stroke::new(3.5, Color32::from_black_alpha(120));
    let handle = |p: Pos2| {
        painter.circle_filled(p, 7.0, theme::ACCENT);
        painter.circle_stroke(p, 7.0, Stroke::new(2.0, Color32::WHITE));
    };
    match &mask.shape {
        Shape::Linear { start, end } => {
            let (a, b) = (to_screen(*start), to_screen(*end));
            let dir = (b - a).normalized();
            let across = Vec2::new(-dir.y, dir.x) * full.width().max(full.height()) * 2.0;
            for (p, s) in [(a, line), (b, Stroke::new(1.5, Color32::from_white_alpha(140)))] {
                painter.line_segment([p - across, p + across], shadow);
                painter.line_segment([p - across, p + across], s);
            }
            painter.line_segment([a, b], shadow);
            painter.line_segment([a, b], line);
            handle(a);
            handle(b);
        }
        Shape::Radial { center, radius, feather } => {
            let c = to_screen(*center);
            let (rx, ry) = (radius[0] * full.width(), radius[1] * full.height());
            let ellipse = |k: f32| -> Vec<Pos2> {
                (0..=64)
                    .map(|i| {
                        let t = i as f32 / 64.0 * std::f32::consts::TAU;
                        c + Vec2::new(t.cos() * rx * k, t.sin() * ry * k)
                    })
                    .collect()
            };
            painter.add(egui::Shape::line(ellipse(1.0), shadow));
            painter.add(egui::Shape::line(ellipse(1.0), line));
            painter.add(egui::Shape::dashed_line(&ellipse(1.0 - feather.clamp(0.0, 1.0)), Stroke::new(1.0, Color32::from_white_alpha(170)), 6.0, 5.0));
            handle(c);
            handle(c + Vec2::new(rx, 0.0));
            handle(c + Vec2::new(0.0, ry));
        }
        Shape::Brush { .. } => {
            if let Some(p) = pointer.filter(|p| full.contains(*p)) {
                let r = brush_radius(app) * full.width().max(full.height());
                let color = if app.mask_erase { theme::DANGER } else { Color32::WHITE };
                painter.circle_stroke(p, r, Stroke::new(1.5, color));
                painter.circle_stroke(p, r * app.brush_hardness.clamp(0.0, 0.98), Stroke::new(1.0, color.gamma_multiply(0.5)));
            }
        }
    }
}

/// Brush radius as a fraction of the picture's longer side.
fn brush_radius(app: &StudioApp) -> f32 {
    let long = app.scene.as_ref().map_or(crate::scene::WORK_SIZE, |s| s.work.width().max(s.work.height())) as f32;
    app.brush_radius / long.max(1.0)
}

/// Paint or move the selected mask.
pub fn interact(app: &mut StudioApp, response: &egui::Response, full: Rect, ui: &egui::Ui) {
    let Some(i) = app.mask_edit else { return };
    let to_norm = |p: Pos2| [(p.x - full.min.x) / full.width(), (p.y - full.min.y) / full.height()];
    let to_screen = |n: [f32; 2]| Pos2::new(full.min.x + n[0] * full.width(), full.min.y + n[1] * full.height());
    let radius = brush_radius(app);
    let erase = app.mask_erase || ui.input(|inp| inp.modifiers.alt);
    let hardness = app.brush_hardness;
    let Some(mask) = app.scene.as_mut().and_then(|s| s.edits.masks.get_mut(i)) else { return };

    if response.drag_started() {
        let p = response.interact_pointer_pos().unwrap_or_default();
        app.mask_handle = Some(pick_handle(mask, p, &to_screen, &to_norm));
    }
    if response.hovered() {
        let near = response.hover_pos().map(|p| pick_handle(mask, p, &to_screen, &to_norm));
        ui.ctx().set_cursor_icon(match near {
            Some(Handle::Paint) => egui::CursorIcon::Crosshair,
            Some(Handle::New(_)) => egui::CursorIcon::Crosshair,
            Some(_) => egui::CursorIcon::Grab,
            None => egui::CursorIcon::Default,
        });
    }
    let Some(p) = response.interact_pointer_pos() else { return };
    let n = to_norm(p);
    if response.clicked() {
        if let Shape::Brush { .. } = mask.shape {
            mask.dab(n, radius, hardness, erase);
        }
    }
    if response.dragged() {
        match app.mask_handle {
            Some(Handle::Paint) => {
                // Dabs along the stroke so fast moves leave no gaps.
                let prev = to_norm(p - response.drag_delta());
                let steps = ((n[0] - prev[0]).hypot(n[1] - prev[1]) / (radius * 0.3).max(1e-4)).ceil().max(1.0) as usize;
                for s in 1..=steps {
                    let t = s as f32 / steps as f32;
                    mask.dab([prev[0] + (n[0] - prev[0]) * t, prev[1] + (n[1] - prev[1]) * t], radius, hardness, erase);
                }
            }
            Some(h) => move_handle(mask, h, n),
            None => {}
        }
    }
    if response.drag_stopped() {
        app.mask_handle = None;
    }
}

/// Which handle a press at `p` grabs (handles within 12 points win).
fn pick_handle(mask: &Mask, p: Pos2, to_screen: &dyn Fn([f32; 2]) -> Pos2, to_norm: &dyn Fn(Pos2) -> [f32; 2]) -> Handle {
    let near = |q: [f32; 2]| to_screen(q).distance(p) <= 12.0;
    match &mask.shape {
        Shape::Brush { .. } => Handle::Paint,
        Shape::Linear { start, end } => {
            if near(*start) {
                Handle::Start
            } else if near(*end) {
                Handle::End
            } else {
                Handle::New(to_norm(p))
            }
        }
        Shape::Radial { center, radius, .. } => {
            if near([center[0] + radius[0], center[1]]) {
                Handle::RadiusX
            } else if near([center[0], center[1] + radius[1]]) {
                Handle::RadiusY
            } else if near(*center) {
                Handle::Center
            } else {
                Handle::New(to_norm(p))
            }
        }
    }
}

fn move_handle(mask: &mut Mask, h: Handle, n: [f32; 2]) {
    match (&mut mask.shape, h) {
        (Shape::Linear { start, .. }, Handle::Start) => *start = n,
        (Shape::Linear { end, .. }, Handle::End) => *end = n,
        (Shape::Linear { start, end }, Handle::New(from)) => {
            *start = from;
            *end = n;
        }
        (Shape::Radial { center, .. }, Handle::Center) => *center = n,
        (Shape::Radial { center, radius, .. }, Handle::RadiusX) => radius[0] = (n[0] - center[0]).abs().max(0.005),
        (Shape::Radial { center, radius, .. }, Handle::RadiusY) => radius[1] = (n[1] - center[1]).abs().max(0.005),
        (Shape::Radial { center, radius, .. }, Handle::New(from)) => {
            *center = from;
            *radius = [(n[0] - from[0]).abs().max(0.005), (n[1] - from[1]).abs().max(0.005)];
        }
        _ => {}
    }
}
