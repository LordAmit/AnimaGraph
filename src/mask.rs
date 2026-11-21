//! Masks for local edits on still photos: a painted area, a linear
//! gradient or a radial (oval) gradient, optionally narrowed to a range of
//! brightness or colour, each with its own light and colour adjustments.
//! Positions are fractions (0..1) of the whole picture, so masks work at
//! any size; painted areas are kept at the editing size.

use std::sync::Arc;

use image::RgbaImage;

use crate::finish::Look;

/// Where a mask applies, before any range narrows it.
#[derive(Clone, Debug, PartialEq)]
pub enum Shape {
    /// Painted coverage (0..1) at `size`, row by row.
    Brush { map: Arc<Vec<f32>>, size: [u32; 2] },
    /// Full effect on the `start` side, fading to none at `end`.
    Linear { start: [f32; 2], end: [f32; 2] },
    /// An oval: centre, half-width and half-height (fractions of the
    /// picture's width and height), and how much of it fades out (0..1).
    Radial { center: [f32; 2], radius: [f32; 2], feather: f32 },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RangeKind {
    Off,
    /// Only tones between `lum[0]` and `lum[1]`.
    Luminance,
    /// Only colours near `hue`.
    Color,
}

/// Narrows a mask to some tones or colours.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Range {
    pub kind: RangeKind,
    /// Brightness from and to, 0..1.
    pub lum: [f32; 2],
    /// Hue in degrees and how wide the band is (degrees).
    pub hue: f32,
    pub hue_width: f32,
    /// How softly the range edges fade, 0..1.
    pub smooth: f32,
}

impl Default for Range {
    fn default() -> Self {
        Self { kind: RangeKind::Off, lum: [0.0, 1.0], hue: 210.0, hue_width: 60.0, smooth: 0.5 }
    }
}

/// The adjustments a mask applies, in the same units as `Look`.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct Local {
    pub exposure: f32,
    pub contrast: f32,
    pub highlights: f32,
    pub shadows: f32,
    pub whites: f32,
    pub blacks: f32,
    pub temperature: f32,
    pub tint: f32,
    pub saturation: f32,
    pub clarity: f32,
    pub texture: f32,
    pub dehaze: f32,
}

impl Local {
    pub fn look(&self) -> Look {
        Look {
            exposure: self.exposure,
            contrast: self.contrast,
            highlights: self.highlights,
            shadows: self.shadows,
            whites: self.whites,
            blacks: self.blacks,
            warmth: self.temperature,
            tint: self.tint,
            saturation: self.saturation,
            clarity: self.clarity,
            texture: self.texture,
            dehaze: self.dehaze,
            ..Look::default()
        }
    }

    pub fn is_none(&self) -> bool {
        *self == Local::default()
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Mask {
    pub name: String,
    pub shape: Shape,
    pub invert: bool,
    /// Hidden masks change nothing.
    pub visible: bool,
    pub range: Range,
    pub local: Local,
}

/// Painted masks are kept at most this many pixels on the long side.
pub const BRUSH_SIZE: u32 = 1024;

impl Mask {
    fn new(name: String, shape: Shape) -> Self {
        Self { name, shape, invert: false, visible: true, range: Range::default(), local: Local::default() }
    }

    /// An empty painted mask for a picture of `w` x `h`.
    pub fn brush(name: String, w: u32, h: u32) -> Self {
        let s = BRUSH_SIZE as f32 / w.max(h).max(1) as f32;
        let size = [((w as f32 * s).round() as u32).max(1), ((h as f32 * s).round() as u32).max(1)];
        Self::new(name, Shape::Brush { map: Arc::new(vec![0.0; (size[0] * size[1]) as usize]), size })
    }

    /// Top of the picture, fading out towards the middle.
    pub fn linear(name: String) -> Self {
        Self::new(name, Shape::Linear { start: [0.5, 0.15], end: [0.5, 0.55] })
    }

    /// An oval in the middle.
    pub fn radial(name: String) -> Self {
        Self::new(name, Shape::Radial { center: [0.5, 0.5], radius: [0.3, 0.3], feather: 0.5 })
    }

    /// The sky, estimated from the picture (see `sky`), as a painted mask
    /// that can be touched up with the brush.
    pub fn sky(name: String, img: &RgbaImage) -> Self {
        let small = crate::scene::downscale(img, BRUSH_SIZE);
        let map = crate::sky::sky_mask(&small);
        Self::new(name, Shape::Brush { map: Arc::new(map), size: [small.width(), small.height()] })
    }

    pub fn kind_name(&self) -> &'static str {
        match self.shape {
            Shape::Brush { .. } => "Brush",
            Shape::Linear { .. } => "Linear",
            Shape::Radial { .. } => "Radial",
        }
    }

    /// Whether this mask changes the picture at all.
    pub fn active(&self) -> bool {
        self.visible && !self.local.is_none()
    }

    /// Paint (or erase) a soft round dab at a 0..1 position. `radius` is a
    /// fraction of the picture's longer side.
    pub fn dab(&mut self, at: [f32; 2], radius: f32, hardness: f32, erase: bool) {
        let Shape::Brush { map, size } = &mut self.shape else { return };
        let [w, h] = *size;
        let r = radius * w.max(h) as f32;
        let (cx, cy) = (at[0] * w as f32, at[1] * h as f32);
        let x0 = (cx - r).floor().max(0.0) as u32;
        let y0 = (cy - r).floor().max(0.0) as u32;
        let x1 = ((cx + r).ceil().max(0.0) as u32).min(w.saturating_sub(1));
        let y1 = ((cy + r).ceil().max(0.0) as u32).min(h.saturating_sub(1));
        let inner = r * hardness.clamp(0.0, 0.98);
        let m = Arc::make_mut(map);
        for y in y0..=y1 {
            for x in x0..=x1 {
                let d = ((x as f32 + 0.5 - cx).powi(2) + (y as f32 + 0.5 - cy).powi(2)).sqrt();
                if d > r {
                    continue;
                }
                let a = if d <= inner {
                    1.0
                } else {
                    let t = (r - d) / (r - inner).max(1e-3);
                    t * t * (3.0 - 2.0 * t)
                };
                let v = &mut m[(y * w + x) as usize];
                *v = if erase { v.min(1.0 - a) } else { v.max(a) };
            }
        }
    }

    /// Fill the painted area completely, or clear it.
    pub fn fill(&mut self, value: f32) {
        if let Shape::Brush { map, .. } = &mut self.shape {
            Arc::make_mut(map).iter_mut().for_each(|v| *v = value);
        }
    }

    /// The same mask after a quarter turn of the picture.
    pub fn turned(&self, clockwise: bool) -> Self {
        let point = |p: [f32; 2]| if clockwise { [1.0 - p[1], p[0]] } else { [p[1], 1.0 - p[0]] };
        let shape = match &self.shape {
            Shape::Linear { start, end } => Shape::Linear { start: point(*start), end: point(*end) },
            Shape::Radial { center, radius, feather } => {
                Shape::Radial { center: point(*center), radius: [radius[1], radius[0]], feather: *feather }
            }
            Shape::Brush { map, size } => {
                let [w, h] = *size;
                let mut out = vec![0.0; map.len()];
                for y in 0..h {
                    for x in 0..w {
                        let (nx, ny) = if clockwise { (h - 1 - y, x) } else { (y, w - 1 - x) };
                        out[(ny * h + nx) as usize] = map[(y * w + x) as usize];
                    }
                }
                Shape::Brush { map: Arc::new(out), size: [h, w] }
            }
        };
        Self { shape, ..self.clone() }
    }

    /// How strongly the mask applies at each pixel of `img` (0..1), row by
    /// row. Ranges are measured on `img`.
    pub fn weights(&self, img: &RgbaImage) -> Vec<f32> {
        let (w, h) = img.dimensions();
        let mut out = vec![0.0f32; (w * h) as usize];
        if w == 0 || h == 0 {
            return out;
        }
        let raw = img.as_raw();
        crate::render::par_chunks(&mut out, w as usize, |y, row| {
            let ny = (y as f32 + 0.5) / h as f32;
            for (x, v) in row.iter_mut().enumerate() {
                let nx = (x as f32 + 0.5) / w as f32;
                let mut s = self.shape_at(nx, ny, w as f32 / h as f32);
                if self.invert {
                    s = 1.0 - s;
                }
                if s > 0.0 && self.range.kind != RangeKind::Off {
                    let i = (y * w as usize + x) * 4;
                    s *= self.range_at([raw[i], raw[i + 1], raw[i + 2]]);
                }
                *v = s;
            }
        });
        out
    }

    /// The mask's area at a 0..1 position, before range and invert.
    /// `aspect` is width ÷ height (keeps distances even across the picture).
    pub fn shape_at(&self, x: f32, y: f32, aspect: f32) -> f32 {
        let smooth = |t: f32| {
            let t = t.clamp(0.0, 1.0);
            t * t * (3.0 - 2.0 * t)
        };
        match &self.shape {
            Shape::Brush { map, size } => {
                let [bw, bh] = *size;
                let fx = (x * bw as f32 - 0.5).clamp(0.0, (bw - 1) as f32);
                let fy = (y * bh as f32 - 0.5).clamp(0.0, (bh - 1) as f32);
                let (x0, y0) = (fx.floor() as u32, fy.floor() as u32);
                let (x1, y1) = ((x0 + 1).min(bw - 1), (y0 + 1).min(bh - 1));
                let (tx, ty) = (fx - x0 as f32, fy - y0 as f32);
                let at = |xx: u32, yy: u32| map[(yy * bw + xx) as usize];
                let top = at(x0, y0) * (1.0 - tx) + at(x1, y0) * tx;
                let bottom = at(x0, y1) * (1.0 - tx) + at(x1, y1) * tx;
                top * (1.0 - ty) + bottom * ty
            }
            Shape::Linear { start, end } => {
                // Measured with the picture's own proportions.
                let (dx, dy) = ((end[0] - start[0]) * aspect, end[1] - start[1]);
                let len2 = dx * dx + dy * dy;
                if len2 < 1e-8 {
                    return 0.0;
                }
                let t = ((x - start[0]) * aspect * dx + (y - start[1]) * dy) / len2;
                1.0 - smooth(t)
            }
            Shape::Radial { center, radius, feather } => {
                let rx = radius[0].max(1e-4);
                let ry = radius[1].max(1e-4);
                let d = (((x - center[0]) / rx).powi(2) + ((y - center[1]) / ry).powi(2)).sqrt();
                let f = feather.clamp(0.0, 1.0).max(0.01);
                1.0 - smooth((d - (1.0 - f)) / f)
            }
        }
    }

    /// How well a colour fits the range, 0..1.
    fn range_at(&self, rgb: [u8; 3]) -> f32 {
        let r = &self.range;
        let (cr, cg, cb) = (rgb[0] as f32 / 255.0, rgb[1] as f32 / 255.0, rgb[2] as f32 / 255.0);
        let soft = 0.02 + r.smooth.clamp(0.0, 1.0) * 0.2;
        let edge = |v: f32, lo: f32, hi: f32| {
            let a = ((v - lo) / soft + 0.5).clamp(0.0, 1.0);
            let b = ((hi - v) / soft + 0.5).clamp(0.0, 1.0);
            a.min(b)
        };
        match r.kind {
            RangeKind::Off => 1.0,
            RangeKind::Luminance => {
                let lum = 0.2126 * cr + 0.7152 * cg + 0.0722 * cb;
                edge(lum, r.lum[0].min(r.lum[1]), r.lum[0].max(r.lum[1]))
            }
            RangeKind::Color => {
                let (h, s) = hue_sat(cr, cg, cb);
                let d = ((h - r.hue).rem_euclid(360.0)).min((r.hue - h).rem_euclid(360.0));
                let half = (r.hue_width / 2.0).max(1.0);
                let fade = half * (0.2 + r.smooth.clamp(0.0, 1.0));
                let t = ((half + fade - d) / (2.0 * fade)).clamp(0.0, 1.0);
                // Greys have no colour to match.
                t * ((s - 0.05) / 0.15).clamp(0.0, 1.0)
            }
        }
    }
}

/// Hue (degrees) and saturation (0..1) of a colour.
pub fn hue_sat(r: f32, g: f32, b: f32) -> (f32, f32) {
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let d = max - min;
    if d <= 1e-6 {
        return (0.0, 0.0);
    }
    let h = if max == r {
        60.0 * ((g - b) / d).rem_euclid(6.0)
    } else if max == g {
        60.0 * ((b - r) / d + 2.0)
    } else {
        60.0 * ((r - g) / d + 4.0)
    };
    (h, if max > 0.0 { d / max } else { 0.0 })
}

/// Apply every visible mask's adjustments to a picture of the whole photo
/// (after the global edits). Each mask is measured on the picture before
/// any mask was applied.
pub fn apply_masks(img: &mut RgbaImage, masks: &[Mask]) {
    if !masks.iter().any(Mask::active) {
        return;
    }
    let base = img.clone();
    for m in masks.iter().filter(|m| m.active()) {
        let w = m.weights(&base);
        if !w.iter().any(|&v| v > 0.002) {
            continue;
        }
        let mut adjusted = img.clone();
        crate::finish::apply_tone(&mut adjusted, &m.local.look());
        let width = img.width() as usize;
        let adj = adjusted.as_raw();
        let data: &mut [u8] = img;
        crate::render::par_chunks(data, width * 4, |y, row| {
            for x in 0..width {
                let a = w[y * width + x];
                if a <= 0.0 {
                    continue;
                }
                let i = x * 4;
                let j = (y * width + x) * 4;
                for k in 0..3 {
                    row[i + k] = (row[i + k] as f32 * (1.0 - a) + adj[j + k] as f32 * a).round() as u8;
                }
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn linear_gradient_fades_from_start_to_end() {
        let m = Mask::linear("L".into());
        assert!(m.shape_at(0.5, 0.05, 1.5) > 0.99);
        assert!(m.shape_at(0.5, 0.9, 1.5) < 0.01);
        let mid = m.shape_at(0.5, 0.35, 1.5);
        assert!(mid > 0.3 && mid < 0.7, "{mid}");
    }

    #[test]
    fn radial_is_full_inside_and_empty_outside() {
        let m = Mask::radial("R".into());
        assert!(m.shape_at(0.5, 0.5, 1.0) > 0.99);
        assert_eq!(m.shape_at(0.05, 0.05, 1.0), 0.0);
        let mut inv = m.clone();
        inv.invert = true;
        let img = RgbaImage::from_pixel(10, 10, image::Rgba([100, 100, 100, 255]));
        let w = inv.weights(&img);
        assert!(w[0] > 0.99 && w[55] < 0.01);
    }

    #[test]
    fn brush_paints_and_follows_a_quarter_turn() {
        let mut m = Mask::brush("B".into(), 200, 100);
        m.dab([0.1, 0.2], 0.05, 0.8, false);
        assert!(m.shape_at(0.1, 0.2, 2.0) > 0.9);
        assert!(m.shape_at(0.9, 0.8, 2.0) < 0.01);
        let t = m.turned(true);
        // (0.1, 0.2) moves to (0.8, 0.1) after a right turn.
        assert!(t.shape_at(0.8, 0.1, 0.5) > 0.9);
        m.dab([0.1, 0.2], 0.05, 0.8, true);
        assert!(m.shape_at(0.1, 0.2, 2.0) < 0.01);
    }

    #[test]
    fn ranges_pick_tones_and_colours() {
        let mut m = Mask::radial("R".into());
        m.shape = Shape::Radial { center: [0.5, 0.5], radius: [5.0, 5.0], feather: 0.1 };
        m.range = Range { kind: RangeKind::Luminance, lum: [0.7, 1.0], ..Range::default() };
        let mut img = RgbaImage::from_pixel(2, 1, image::Rgba([240, 240, 240, 255]));
        img.put_pixel(1, 0, image::Rgba([30, 30, 30, 255]));
        let w = m.weights(&img);
        assert!(w[0] > 0.9 && w[1] < 0.05);
        m.range = Range { kind: RangeKind::Color, hue: 220.0, hue_width: 60.0, ..Range::default() };
        img.put_pixel(0, 0, image::Rgba([40, 90, 200, 255]));
        img.put_pixel(1, 0, image::Rgba([200, 60, 40, 255]));
        let w = m.weights(&img);
        assert!(w[0] > 0.9 && w[1] < 0.05, "{w:?}");
    }

    #[test]
    fn masks_change_only_their_area() {
        let mut img = RgbaImage::from_pixel(40, 20, image::Rgba([100, 100, 100, 255]));
        let mut m = Mask::radial("R".into());
        m.shape = Shape::Radial { center: [0.25, 0.5], radius: [0.2, 0.4], feather: 0.2 };
        m.local.exposure = 1.0;
        apply_masks(&mut img, &[m]);
        assert!(img.get_pixel(10, 10)[0] > 120);
        assert_eq!(img.get_pixel(35, 10)[0], 100);
    }
}
