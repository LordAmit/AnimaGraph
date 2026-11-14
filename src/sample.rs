//! A procedurally drawn landscape (sky, clouds, hills, lake) so the app can
//! be tried before the person opens one of their own photos.

use image::{Rgba, RgbaImage};

use crate::scene::Arrow;

fn hash(x: i32, y: i32, seed: i32) -> f32 {
    let mut n = x.wrapping_mul(374_761_393) ^ y.wrapping_mul(668_265_263) ^ seed.wrapping_mul(1_442_695_041);
    n = (n ^ (n >> 13)).wrapping_mul(1_274_126_177);
    ((n ^ (n >> 16)) & 0xffff) as f32 / 65535.0
}

fn value_noise(x: f32, y: f32, seed: i32) -> f32 {
    let (xi, yi) = (x.floor() as i32, y.floor() as i32);
    let (tx, ty) = (x - xi as f32, y - yi as f32);
    let s = |t: f32| t * t * (3.0 - 2.0 * t);
    let (sx, sy) = (s(tx), s(ty));
    let a = hash(xi, yi, seed);
    let b = hash(xi + 1, yi, seed);
    let c = hash(xi, yi + 1, seed);
    let d = hash(xi + 1, yi + 1, seed);
    (a * (1.0 - sx) + b * sx) * (1.0 - sy) + (c * (1.0 - sx) + d * sx) * sy
}

fn fbm(x: f32, y: f32, seed: i32) -> f32 {
    let mut v = 0.0;
    let mut amp = 0.5;
    let mut f = 1.0;
    for o in 0..5 {
        v += amp * value_noise(x * f, y * f, seed + o);
        f *= 2.0;
        amp *= 0.5;
    }
    v
}

fn mix(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t]
}

/// Sample landscape at the given size.
pub fn landscape(w: u32, h: u32) -> RgbaImage {
    let mut img = RgbaImage::new(w, h);
    let horizon = 0.62;
    for y in 0..h {
        for x in 0..w {
            let u = x as f32 / w as f32;
            let v = y as f32 / h as f32;
            let ridge_far = 0.47 + 0.07 * fbm(u * 3.0, 1.0, 7) - 0.04 * (u * 5.0).sin();
            let ridge_near = 0.55 + 0.08 * fbm(u * 4.0 + 3.0, 2.0, 11);
            let col = if v < ridge_far {
                sky(u, v)
            } else if v < ridge_near.min(horizon) {
                let t = (v - ridge_far) / 0.15;
                mix([0.36, 0.42, 0.58], [0.28, 0.33, 0.45], t.clamp(0.0, 1.0))
            } else if v < horizon {
                let n = fbm(u * 18.0, v * 18.0, 3);
                mix([0.14, 0.25, 0.18], [0.22, 0.34, 0.2], n)
            } else {
                // Lake: mirrored sky with ripples.
                let mv = horizon - (v - horizon) * 1.4;
                let ripple = 0.004 * (v * 220.0 + fbm(u * 8.0, v * 30.0, 5) * 6.0).sin();
                let mut c = if mv > 0.0 { sky(u + ripple, mv.max(0.0)) } else { sky(u, 0.0) };
                let depth = (v - horizon) / (1.0 - horizon);
                c = mix(c, [0.06, 0.16, 0.24], 0.35 + 0.4 * depth);
                c
            };
            img.put_pixel(
                x,
                y,
                Rgba([
                    (col[0].clamp(0.0, 1.0) * 255.0) as u8,
                    (col[1].clamp(0.0, 1.0) * 255.0) as u8,
                    (col[2].clamp(0.0, 1.0) * 255.0) as u8,
                    255,
                ]),
            );
        }
    }
    img
}

fn sky(u: f32, v: f32) -> [f32; 3] {
    let base = mix([0.98, 0.72, 0.52], [0.32, 0.52, 0.86], (1.0 - v / 0.5).clamp(0.0, 1.0));
    let cloud = fbm(u * 4.0, v * 9.0, 21);
    let c = ((cloud - 0.48) * 3.2).clamp(0.0, 1.0) * (1.0 - (v / 0.5).clamp(0.0, 1.0) * 0.6);
    mix(base, [1.0, 0.95, 0.93], c)
}

/// The moving region of the sample: sky above the far hills, and the lake.
pub fn sample_mask(w: u32, h: u32) -> Vec<f32> {
    let mut m = vec![0.0f32; (w * h) as usize];
    for y in 0..h {
        for x in 0..w {
            let u = x as f32 / w as f32;
            let v = y as f32 / h as f32;
            let ridge_far = 0.47 + 0.07 * fbm(u * 3.0, 1.0, 7) - 0.04 * (u * 5.0).sin();
            if v < ridge_far - 0.01 || v > 0.635 {
                m[(y * w + x) as usize] = 1.0;
            }
        }
    }
    crate::scene::blur_mask(&m, w, h, 2)
}

/// Starting hints for the sample: clouds drift right, the lake flows left.
pub fn sample_arrows() -> (Vec<Arrow>, Vec<[f32; 2]>) {
    let mut arrows = Vec::new();
    for (x, y) in [(0.15, 0.12), (0.45, 0.2), (0.75, 0.1), (0.3, 0.33), (0.65, 0.36)] {
        arrows.push(Arrow { from: [x, y], to: [x + 0.12, y + 0.005] });
    }
    for (x, y) in [(0.25, 0.75), (0.6, 0.85), (0.85, 0.72)] {
        arrows.push(Arrow { from: [x, y], to: [x - 0.07, y] });
    }
    (arrows, vec![[0.5, 0.58]])
}
