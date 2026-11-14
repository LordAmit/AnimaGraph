//! Turns sparse motion hints into a dense, smooth Eulerian motion field:
//! one velocity per pixel that stays constant over time, as used by
//! research methods for animating still landscape photos.

use crate::scene::Arrow;

/// Frames over which content travels one arrow length at speed 1.
pub const FRAMES_PER_ARROW: f32 = 48.0;

pub fn dist_to_segment(p: [f32; 2], a: [f32; 2], b: [f32; 2]) -> f32 {
    let ab = [b[0] - a[0], b[1] - a[1]];
    let ap = [p[0] - a[0], p[1] - a[1]];
    let len2 = ab[0] * ab[0] + ab[1] * ab[1];
    let t = if len2 > 0.0 {
        ((ap[0] * ab[0] + ap[1] * ab[1]) / len2).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let q = [a[0] + ab[0] * t, a[1] + ab[1] * t];
    ((p[0] - q[0]).powi(2) + (p[1] - q[1]).powi(2)).sqrt()
}

/// Velocity field in pixels per frame, `w * h` entries.
/// `mask` (same size) scales the motion so still areas stay still.
pub fn dense_field(
    w: u32,
    h: u32,
    arrows: &[Arrow],
    pins: &[[f32; 2]],
    mask: &[f32],
    speed: f32,
) -> Vec<[f32; 2]> {
    let n = (w * h) as usize;
    if arrows.is_empty() {
        return vec![[0.0, 0.0]; n];
    }
    // Hints in pixel units.
    struct Hint {
        a: [f32; 2],
        b: [f32; 2],
        v: [f32; 2],
    }
    let (wf, hf) = (w as f32, h as f32);
    let px = |p: [f32; 2]| [p[0] * wf, p[1] * hf];
    let mut hints: Vec<Hint> = arrows
        .iter()
        .map(|ar| {
            let a = px(ar.from);
            let b = px(ar.to);
            let v = [
                (b[0] - a[0]) * speed / FRAMES_PER_ARROW,
                (b[1] - a[1]) * speed / FRAMES_PER_ARROW,
            ];
            Hint { a, b, v }
        })
        .collect();
    hints.extend(pins.iter().map(|p| {
        let a = px(*p);
        Hint { a, b: a, v: [0.0, 0.0] }
    }));

    // Evaluate on a coarse grid, then upsample: the field is smooth anyway.
    let step = 4u32;
    let gw = w.div_ceil(step) + 1;
    let gh = h.div_ceil(step) + 1;
    let diag = (wf * wf + hf * hf).sqrt();
    let soft = (diag * 0.02).powi(2);
    let mut grid = vec![[0.0f32; 2]; (gw * gh) as usize];
    crate::render::par_chunks(&mut grid, gw as usize, |gy, row| {
        for (gx, cell) in row.iter_mut().enumerate() {
            let p = [(gx as u32 * step) as f32, (gy as u32 * step) as f32];
            let (mut sx, mut sy, mut sw) = (0.0f32, 0.0f32, 0.0f32);
            for hint in &hints {
                let d = dist_to_segment(p, hint.a, hint.b);
                let wgt = 1.0 / (d * d + soft).powf(1.5);
                sx += hint.v[0] * wgt;
                sy += hint.v[1] * wgt;
                sw += wgt;
            }
            *cell = [sx / sw, sy / sw];
        }
    });

    let feather = crate::scene::blur_mask(mask, w, h, ((wf.max(hf)) / 200.0).ceil() as u32);
    let mut field = vec![[0.0f32; 2]; n];
    crate::render::par_chunks(&mut field, w as usize, |y, row| {
        let fy = y as f32 / step as f32;
        let y0 = (fy.floor() as u32).min(gh - 2);
        let ty = fy - y0 as f32;
        for (x, out) in row.iter_mut().enumerate() {
            let fx = x as f32 / step as f32;
            let x0 = (fx.floor() as u32).min(gw - 2);
            let tx = fx - x0 as f32;
            let g = |xx: u32, yy: u32| grid[(yy * gw + xx) as usize];
            let (a, b, c, d) = (g(x0, y0), g(x0 + 1, y0), g(x0, y0 + 1), g(x0 + 1, y0 + 1));
            let m = feather[y * w as usize + x];
            for k in 0..2 {
                let top = a[k] * (1.0 - tx) + b[k] * tx;
                let bot = c[k] * (1.0 - tx) + d[k] * tx;
                out[k] = (top * (1.0 - ty) + bot * ty) * m;
            }
        }
    });
    field
}

/// Place a regular grid of arrows pointing at `angle_deg` over the moving
/// region, as a quick starting point the person can edit.
pub fn auto_arrows(mask: &[f32], w: u32, h: u32, angle_deg: f32, length: f32) -> Vec<Arrow> {
    let (dx, dy) = (angle_deg.to_radians().cos(), angle_deg.to_radians().sin());
    let aspect = w as f32 / h as f32;
    let len_x = length * dx;
    let len_y = length * dy * aspect;
    let mut out = Vec::new();
    let cols = 6;
    let rows = 5;
    for r in 0..rows {
        for c in 0..cols {
            let cx = (c as f32 + 0.5) / cols as f32;
            let cy = (r as f32 + 0.5) / rows as f32;
            let x = ((cx * w as f32) as u32).min(w - 1);
            let y = ((cy * h as f32) as u32).min(h - 1);
            if mask[(y * w + x) as usize] < 0.5 {
                continue;
            }
            out.push(Arrow {
                from: [cx - len_x / 2.0, cy - len_y / 2.0],
                to: [cx + len_x / 2.0, cy + len_y / 2.0],
            });
        }
    }
    out
}
