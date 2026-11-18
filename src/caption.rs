//! Text caption drawn on the picture, in any font the app can read.

use ab_glyph::{Font, FontRef, PxScale, ScaleFont, point};
use image::{Rgba, RgbaImage};

use crate::fonts::FontEntry;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum Place {
    Top,
    Middle,
    Bottom,
}

/// Where the caption goes: on the picture, or in the border around it.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum Area {
    Picture,
    /// In the border strip (bottom strip for Bottom and Middle, top strip
    /// for Top). Needs a border; without one it stays on the picture.
    Border,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum Align {
    Left,
    Center,
    Right,
}

#[derive(Clone, PartialEq, Debug)]
pub struct Caption {
    pub enabled: bool,
    pub text: String,
    pub font: FontEntry,
    /// Letter height as a fraction of the picture height.
    pub size: f32,
    pub color: [u8; 3],
    pub opacity: f32,
    pub place: Place,
    pub align: Align,
    /// Distance from the edge as a fraction of the picture height.
    pub margin: f32,
    pub shadow: bool,
    pub area: Area,
}

pub const DEFAULT_SIZE: f32 = 0.07;
pub const DEFAULT_MARGIN: f32 = 0.06;

impl Default for Caption {
    fn default() -> Self {
        Self {
            enabled: false,
            text: String::new(),
            font: FontEntry::builtin(),
            size: DEFAULT_SIZE,
            color: [255, 255, 255],
            opacity: 1.0,
            place: Place::Bottom,
            align: Align::Center,
            margin: DEFAULT_MARGIN,
            shadow: true,
            area: Area::Picture,
        }
    }
}

impl Caption {
    /// Placeholder shown on the preview while the caption box is empty.
    pub const PLACEHOLDER: &'static str = "Your caption";

    /// A string that changes whenever the drawn result would change.
    pub fn key(&self, w: u32, h: u32) -> String {
        format!("{self:?}{w}x{h}")
    }

    /// The caption alone, on a transparent picture of size `w` x `h`.
    /// `None` when there is nothing to draw.
    pub fn render(&self, w: u32, h: u32) -> Option<RgbaImage> {
        let margin = self.margin * h as f32;
        self.render_box(w, h, [0.0, 0.0, w as f32, h as f32], h as f32, margin, false)
    }

    /// Whether the caption goes in the border (only when a border is on).
    pub fn on_border(&self, border: &crate::border::Border) -> bool {
        self.area == Area::Border && border.enabled
    }

    /// The caption for a `w` x `h` picture with `border` around it. The
    /// flag is true when it is drawn on the whole output (picture and
    /// border), so it is blended after the border is added.
    pub fn render_with_border(&self, w: u32, h: u32, border: &crate::border::Border) -> Option<(RgbaImage, bool)> {
        if !self.on_border(border) {
            return self.render(w, h).map(|img| (img, false));
        }
        let [ow, oh, ox, oy] = border.layout(w as f32, h as f32);
        let (ow_px, oh_px) = ((ow.round() as u32).max(w), (oh.round() as u32).max(h));
        // The strip the text sits in, centered in it from top to bottom.
        let (y0, y1) = match self.place {
            Place::Top => (0.0, oy),
            Place::Middle | Place::Bottom => (oy + h as f32, oh),
        };
        // Left and right text lines up with the picture's edges.
        let img = self.render_box(ow_px, oh_px, [0.0, y0, ow, y1], h as f32, ox, true)?;
        Some((img, true))
    }

    /// Draw the caption on a transparent `cw` x `ch` picture, inside the box
    /// `zone` (x0, y0, x1, y1). Letter size and margin follow `ref_h` (the
    /// picture's height); `inset` is the distance from the box's sides for
    /// left and right alignment. With `center` the text is centered top to
    /// bottom in the box whatever the place.
    fn render_box(&self, cw: u32, ch: u32, zone: [f32; 4], ref_h: f32, inset: f32, center: bool) -> Option<RgbaImage> {
        let (w, h) = (cw, ch);
        if !self.enabled || self.text.trim().is_empty() || w == 0 || h == 0 {
            return None;
        }
        let bytes = crate::fonts::font_bytes(&self.font)?;
        let font = FontRef::try_from_slice(&bytes).ok()?;
        let px = (self.size * ref_h).max(4.0);
        let scaled = font.as_scaled(PxScale::from(px));
        let line_h = scaled.height() + scaled.line_gap();
        let lines: Vec<&str> = self.text.lines().collect();
        let block_h = line_h * lines.len() as f32;
        let margin = self.margin * ref_h;
        let [zx0, zy0, zx1, zy1] = zone;
        let top = if center {
            zy0 + (zy1 - zy0 - block_h) / 2.0
        } else {
            match self.place {
                Place::Top => zy0 + margin,
                Place::Middle => zy0 + (zy1 - zy0 - block_h) / 2.0,
                Place::Bottom => zy1 - margin - block_h,
            }
        };

        // Coverage of the letters (0..1) and of the shadow.
        let mut cover = vec![0.0f32; (w * h) as usize];
        for (li, line) in lines.iter().enumerate() {
            let mut width = 0.0;
            let mut prev = None;
            for ch in line.chars() {
                let id = font.glyph_id(ch);
                if let Some(p) = prev {
                    width += scaled.kern(p, id);
                }
                width += scaled.h_advance(id);
                prev = Some(id);
            }
            let x0 = match self.align {
                Align::Left => zx0 + inset,
                Align::Center => (zx0 + zx1 - width) / 2.0,
                Align::Right => zx1 - inset - width,
            };
            let baseline = top + li as f32 * line_h + scaled.ascent();
            let mut x = x0;
            let mut prev = None;
            for ch in line.chars() {
                let id = font.glyph_id(ch);
                if let Some(p) = prev {
                    x += scaled.kern(p, id);
                }
                let glyph = id.with_scale_and_position(PxScale::from(px), point(x, baseline));
                x += scaled.h_advance(id);
                prev = Some(id);
                let Some(outline) = font.outline_glyph(glyph) else { continue };
                let b = outline.px_bounds();
                outline.draw(|gx, gy, c| {
                    let xx = b.min.x as i32 + gx as i32;
                    let yy = b.min.y as i32 + gy as i32;
                    if xx >= 0 && yy >= 0 && (xx as u32) < w && (yy as u32) < h {
                        let i = (yy as u32 * w + xx as u32) as usize;
                        cover[i] = (cover[i] + c).min(1.0);
                    }
                });
            }
        }

        let mut out = RgbaImage::new(w, h);
        let [r, g, b] = self.color;
        let shift = (px * 0.06).round().max(1.0) as i32;
        for y in 0..h as i32 {
            for x in 0..w as i32 {
                let i = (y as u32 * w + x as u32) as usize;
                let a = cover[i] * self.opacity;
                let s = if self.shadow {
                    let (sx, sy) = (x - shift, y - shift);
                    if sx >= 0 && sy >= 0 {
                        cover[(sy as u32 * w + sx as u32) as usize] * 0.55 * self.opacity
                    } else {
                        0.0
                    }
                } else {
                    0.0
                };
                // Letter over shadow ("over" blend of two layers).
                let out_a = a + s * (1.0 - a);
                if out_a <= 0.0 {
                    continue;
                }
                let mix = |c: u8| ((c as f32 * a) / out_a).round() as u8;
                out.put_pixel(x as u32, y as u32, Rgba([mix(r), mix(g), mix(b), (out_a * 255.0).round() as u8]));
            }
        }
        Some(out)
    }
}

/// Blend a caption picture (same size) over `img`.
pub fn blend(img: &mut RgbaImage, overlay: &RgbaImage) {
    if img.dimensions() != overlay.dimensions() {
        return;
    }
    for (d, s) in img.pixels_mut().zip(overlay.pixels()) {
        let a = s[3] as f32 / 255.0;
        if a <= 0.0 {
            continue;
        }
        for k in 0..3 {
            d[k] = (d[k] as f32 * (1.0 - a) + s[k] as f32 * a).round() as u8;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn caption_draws_where_asked() {
        let c = Caption { enabled: true, text: "Hello".into(), place: Place::Bottom, ..Caption::default() };
        let o = c.render(400, 200).unwrap();
        let ink_rows: Vec<u32> = (0..200).filter(|&y| (0..400).any(|x| o.get_pixel(x, y)[3] > 0)).collect();
        assert!(!ink_rows.is_empty());
        assert!(*ink_rows.first().unwrap() > 120, "text should sit low: {:?}", ink_rows.first());
        let mut img = RgbaImage::from_pixel(400, 200, Rgba([0, 0, 0, 255]));
        blend(&mut img, &o);
        assert!(img.pixels().any(|p| p[0] > 200));
        assert!(Caption { enabled: false, ..c.clone() }.render(400, 200).is_none());
    }

    #[test]
    fn caption_in_the_border_strip() {
        let border = crate::border::Border::polaroid();
        let c = Caption { enabled: true, text: "Hello".into(), area: Area::Border, size: 0.05, ..Caption::default() };
        let (o, on_border) = c.render_with_border(400, 400, &border).unwrap();
        assert!(on_border);
        let [ow, oh, _, oy] = border.layout(400.0, 400.0);
        assert_eq!(o.dimensions(), (ow.round() as u32, oh.round() as u32));
        // All ink is below the picture, inside the bottom strip.
        let ink_rows: Vec<u32> = (0..o.height()).filter(|&y| (0..o.width()).any(|x| o.get_pixel(x, y)[3] > 0)).collect();
        assert!(!ink_rows.is_empty());
        assert!(*ink_rows.first().unwrap() as f32 > oy + 400.0);
        // Without a border it stays on the picture.
        let (_, on) = c.render_with_border(400, 400, &crate::border::Border::default()).unwrap();
        assert!(!on);
    }
}
