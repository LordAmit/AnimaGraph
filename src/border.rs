//! A colored border around the output. It can be even on all sides, keep
//! the picture's shape, or pad the picture out to a chosen aspect ratio
//! without changing the picture itself.

use image::{Rgba, RgbaImage};

use crate::crop::Aspect;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BorderFit {
    /// Same thickness on every side.
    Even,
    /// Thicker on the long sides so the output keeps the picture's shape.
    KeepShape,
    /// Padded out to this aspect ratio, picture centered.
    Ratio(Aspect),
    /// Instant-photo frame: thin top and sides, deep bottom strip.
    Polaroid,
}

/// The bottom strip of the instant-photo frame is this many times the sides.
pub const POLAROID_BOTTOM: f32 = 3.6;

impl BorderFit {
    pub fn label(self) -> &'static str {
        match self {
            BorderFit::Even => "Even",
            BorderFit::Polaroid => "Polaroid",
            BorderFit::KeepShape => "Keep shape",
            BorderFit::Ratio(a) => a.label(),
        }
    }
}

/// Aspect ratios offered as border targets.
pub const RATIO_CHOICES: [Aspect; 8] = [
    Aspect::Square,
    Aspect::Portrait45,
    Aspect::Portrait34,
    Aspect::Portrait23,
    Aspect::Story,
    Aspect::Classic43,
    Aspect::Photo32,
    Aspect::Wide,
];

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Border {
    pub enabled: bool,
    /// Thickness as a fraction of the picture's shorter side.
    pub size: f32,
    pub color: [u8; 3],
    pub fit: BorderFit,
}

pub const DEFAULT_SIZE: f32 = 0.05;

impl Default for Border {
    fn default() -> Self {
        Self { enabled: false, size: DEFAULT_SIZE, color: [255, 255, 255], fit: BorderFit::Even }
    }
}

impl Border {
    /// Cream instant-photo frame.
    pub fn polaroid() -> Self {
        Self { enabled: true, size: 0.075, color: [250, 248, 242], fit: BorderFit::Polaroid }
    }

    /// Output size and where the picture sits in it, for a `w` x `h` picture:
    /// (output width, output height, left offset, top offset).
    pub fn layout(&self, w: f32, h: f32) -> [f32; 4] {
        if !self.enabled {
            return [w, h, 0.0, 0.0];
        }
        let b = self.size.max(0.0) * w.min(h);
        if self.fit == BorderFit::Polaroid {
            return [w + 2.0 * b, h + b * (1.0 + POLAROID_BOTTOM), b, b];
        }
        let (mut ow, mut oh) = match self.fit {
            BorderFit::KeepShape => {
                let s = 1.0 + 2.0 * b / w.min(h);
                (w * s, h * s)
            }
            _ => (w + 2.0 * b, h + 2.0 * b),
        };
        if let BorderFit::Ratio(a) = self.fit {
            if let Some(r) = a.ratio(w as u32, h as u32) {
                if ow / oh < r {
                    ow = oh * r;
                } else {
                    oh = ow / r;
                }
            }
        }
        [ow, oh, (ow - w) / 2.0, (oh - h) / 2.0]
    }

    pub fn apply(&self, img: &RgbaImage) -> RgbaImage {
        if !self.enabled {
            return img.clone();
        }
        let (w, h) = img.dimensions();
        let [ow, oh, ox, oy] = self.layout(w as f32, h as f32);
        let (ow, oh) = (ow.round() as u32, oh.round() as u32);
        let (ox, oy) = (ox.round() as i64, oy.round() as i64);
        let [r, g, b] = self.color;
        let mut out = RgbaImage::from_pixel(ow.max(w), oh.max(h), Rgba([r, g, b, 255]));
        image::imageops::replace(&mut out, img, ox, oy);
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn even_border_adds_the_same_on_every_side() {
        let b = Border { enabled: true, size: 0.1, ..Border::default() };
        let out = b.apply(&RgbaImage::new(200, 100));
        assert_eq!(out.dimensions(), (220, 120));
        assert_eq!(out.get_pixel(0, 0)[0], 255);
        assert_eq!(out.get_pixel(110, 60)[0], 0);
    }

    #[test]
    fn keep_shape_keeps_the_ratio() {
        let b = Border { enabled: true, size: 0.1, fit: BorderFit::KeepShape, ..Border::default() };
        let [w, h, _, _] = b.layout(200.0, 100.0);
        assert!((w / h - 2.0).abs() < 1e-4);
    }

    #[test]
    fn ratio_pads_to_target_without_resizing_the_picture() {
        let b = Border { enabled: true, size: 0.0, fit: BorderFit::Ratio(Aspect::Square), ..Border::default() };
        let out = b.apply(&RgbaImage::new(160, 90));
        assert_eq!(out.dimensions(), (160, 160));
        // Picture is centered: 35 px bands above and below.
        let story = Border { fit: BorderFit::Ratio(Aspect::Story), size: 0.05, ..b };
        let [w, h, ox, oy] = story.layout(160.0, 90.0);
        assert!((w / h - 9.0 / 16.0).abs() < 1e-4);
        assert!((ox - 0.0).abs() > 0.0 || oy > 0.0);
        assert!(w >= 160.0 && h >= 90.0);
    }

    #[test]
    fn polaroid_has_a_deep_bottom_strip() {
        let b = Border::polaroid();
        let [w, h, ox, oy] = b.layout(400.0, 400.0);
        let side = 0.075 * 400.0;
        assert!((ox - side).abs() < 1e-3 && (oy - side).abs() < 1e-3);
        assert!((w - (400.0 + 2.0 * side)).abs() < 1e-3);
        let bottom = h - oy - 400.0;
        assert!((bottom - side * POLAROID_BOTTOM).abs() < 1e-3);
        let out = b.apply(&RgbaImage::from_pixel(400, 400, Rgba([0, 0, 0, 255])));
        // Picture sits at the top-left offset; the bottom strip is frame color.
        assert_eq!(out.get_pixel(30, 30)[0], 0);
        assert_eq!(out.get_pixel(230, out.height() - 10)[0], 250);
    }

    #[test]
    fn disabled_changes_nothing() {
        let img = RgbaImage::new(30, 20);
        assert_eq!(Border::default().apply(&img).dimensions(), (30, 20));
    }
}
