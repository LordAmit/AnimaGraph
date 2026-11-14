//! Steadies a handheld clip: finds how far each frame has drifted from the
//! still frame and shifts it back, with a slight zoom to hide the edges.

use image::RgbaImage;

use crate::render::par_chunks;

/// Width of the small gray copies used to measure drift.
const PROBE_W: u32 = 192;
/// Largest step searched between neighbouring frames, in probe pixels.
const SEARCH: i32 = 5;

fn probe(img: &RgbaImage) -> (Vec<f32>, usize, usize) {
    let (w, h) = img.dimensions();
    let pw = PROBE_W.min(w);
    let ph = ((h as f32 * pw as f32 / w as f32).round() as u32).max(8);
    let small = image::imageops::resize(img, pw, ph, image::imageops::FilterType::Triangle);
    let g = small
        .pixels()
        .map(|p| 0.299 * p[0] as f32 + 0.587 * p[1] as f32 + 0.114 * p[2] as f32)
        .collect();
    (g, pw as usize, ph as usize)
}

/// Mean difference between `frame` moved by (dx, dy) and `reference`.
fn cost(frame: &[f32], reference: &[f32], w: usize, h: usize, dx: i32, dy: i32) -> f32 {
    let margin = (SEARCH * 4) as usize;
    let (mut sum, mut n) = (0.0f32, 0u32);
    let mut y = margin;
    while y + margin < h {
        let sy = (y as i32 + dy) as usize;
        let mut x = margin;
        while x + margin < w {
            let sx = (x as i32 + dx) as usize;
            sum += (frame[sy * w + sx] - reference[y * w + x]).abs();
            n += 1;
            x += 2;
        }
        y += 2;
    }
    sum / n.max(1) as f32
}

/// Drift of every frame relative to frame `reference`, in pixels of the
/// given frames: frame content at x lines up with the reference at x - offset.
pub fn offsets(frames: &[RgbaImage], reference: usize) -> Vec<[f32; 2]> {
    let n = frames.len();
    if n == 0 {
        return Vec::new();
    }
    let probes: Vec<_> = frames.iter().map(probe).collect();
    let (_, w, h) = probes[0];
    let scale = frames[0].width() as f32 / w as f32;
    let reference = reference.min(n - 1);
    let refp = &probes[reference].0;
    let mut out = vec![[0i32; 2]; n];
    // Walk outwards from the reference, searching near the neighbour's result.
    let order: Vec<(usize, usize)> = (reference + 1..n)
        .map(|i| (i, i - 1))
        .chain((0..reference).rev().map(|i| (i, i + 1)))
        .collect();
    let limit = (SEARCH * 4) - 1;
    for (i, prev) in order {
        let start = out[prev];
        let mut best = (f32::MAX, start);
        for dy in -SEARCH..=SEARCH {
            for dx in -SEARCH..=SEARCH {
                let c = [start[0] + dx, start[1] + dy];
                if c[0].abs() > limit || c[1].abs() > limit {
                    continue;
                }
                let v = cost(&probes[i].0, refp, w, h, c[0], c[1]);
                if v < best.0 {
                    best = (v, c);
                }
            }
        }
        out[i] = best.1;
    }
    out.iter().map(|o| [o[0] as f32 * scale, o[1] as f32 * scale]).collect()
}

/// Zoom needed so shifted frames never show an empty edge.
pub fn zoom_for(offsets: &[[f32; 2]], w: u32, h: u32) -> f32 {
    let mx = offsets.iter().map(|o| o[0].abs()).fold(0.0, f32::max);
    let my = offsets.iter().map(|o| o[1].abs()).fold(0.0, f32::max);
    (1.0 + 2.0 * mx / w as f32).max(1.0 + 2.0 * my / h as f32)
}

/// Shift by `offset` and zoom by `zoom` around the center.
pub fn transform(img: &RgbaImage, offset: [f32; 2], zoom: f32) -> RgbaImage {
    let (w, h) = (img.width() as usize, img.height() as usize);
    let src = img.as_raw();
    let (cx, cy) = (w as f32 / 2.0, h as f32 / 2.0);
    let mut out = vec![0u8; w * h * 4];
    par_chunks(&mut out, w * 4, |y, row| {
        for x in 0..w {
            let sx = (cx + (x as f32 - cx) / zoom + offset[0]).clamp(0.0, (w - 1) as f32);
            let sy = (cy + (y as f32 - cy) / zoom + offset[1]).clamp(0.0, (h - 1) as f32);
            let (x0, y0) = (sx.floor() as usize, sy.floor() as usize);
            let (x1, y1) = ((x0 + 1).min(w - 1), (y0 + 1).min(h - 1));
            let (tx, ty) = (sx - x0 as f32, sy - y0 as f32);
            for k in 0..4 {
                let p = |xx: usize, yy: usize| src[(yy * w + xx) * 4 + k] as f32;
                let top = p(x0, y0) * (1.0 - tx) + p(x1, y0) * tx;
                let bot = p(x0, y1) * (1.0 - tx) + p(x1, y1) * tx;
                row[x * 4 + k] = (top * (1.0 - ty) + bot * ty).round() as u8;
            }
        }
    });
    RgbaImage::from_raw(w as u32, h as u32, out).expect("size")
}

/// Apply only the zoom to a mask so it still lines up with zoomed frames.
pub fn zoom_mask(mask: &[f32], w: u32, h: u32, zoom: f32) -> Vec<f32> {
    let (w, h) = (w as usize, h as usize);
    let (cx, cy) = (w as f32 / 2.0, h as f32 / 2.0);
    let mut out = vec![0.0f32; w * h];
    for y in 0..h {
        for x in 0..w {
            let sx = ((cx + (x as f32 - cx) / zoom).round() as usize).min(w - 1);
            let sy = ((cy + (y as f32 - cy) / zoom).round() as usize).min(h - 1);
            out[y * w + x] = mask[sy * w + sx];
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_a_known_shift() {
        let base = crate::sample::landscape(384, 216);
        // Frame 1 shows the scene moved 6 px right and 4 px down.
        let moved = RgbaImage::from_fn(384, 216, |x, y| {
            *base.get_pixel(x.saturating_sub(6), y.saturating_sub(4))
        });
        let offs = offsets(&[base, moved], 0);
        assert_eq!(offs[0], [0.0, 0.0]);
        assert!((offs[1][0] - 6.0).abs() <= 2.0 && (offs[1][1] - 4.0).abs() <= 2.0, "{:?}", offs[1]);
    }
}
