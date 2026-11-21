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

/// The bottom strip of the instant-photo frame is this many times the sides
/// (the default; the person can change it).
pub const POLAROID_BOTTOM: f32 = 3.6;
/// Range offered for the bottom strip, in times the sides.
pub const POLAROID_BOTTOM_RANGE: std::ops::RangeInclusive<f32> = 0.5..=8.0;
/// Card proportions of real instant films, as height ÷ width.
pub const INSTANT_CARDS: [(&str, f32, &str); 3] = [
    ("Square instant", 107.0 / 88.0, "Square picture card, about 88 × 107 mm"),
    ("Mini instant", 86.0 / 54.0, "Small tall card, about 54 × 86 mm"),
    ("Wide instant", 86.0 / 108.0, "Wide card, about 108 × 86 mm"),
];

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
    /// Polaroid: depth of the bottom strip in times the top and sides.
    pub bottom: f32,
    /// Polaroid: output height ÷ width to reach. When set, the bottom strip
    /// is sized to reach it and `bottom` is not used.
    pub target: Option<f32>,
    /// Rounded photo corners: radius as a fraction of the photo's shorter side.
    pub radius: f32,
    /// Paper corner mounts holding the photo, their length along each edge
    /// (fraction of the photo's shorter side) and colour.
    pub mounts: bool,
    pub mount_size: f32,
    pub mount_color: [u8; 3],
}

pub const DEFAULT_MOUNT_SIZE: f32 = 0.1;
pub const DEFAULT_MOUNT_COLOR: [u8; 3] = [28, 28, 30];

pub const DEFAULT_SIZE: f32 = 0.05;

impl Default for Border {
    fn default() -> Self {
        Self {
            enabled: false,
            size: DEFAULT_SIZE,
            color: [255, 255, 255],
            fit: BorderFit::Even,
            bottom: POLAROID_BOTTOM,
            target: None,
            radius: 0.0,
            mounts: false,
            mount_size: DEFAULT_MOUNT_SIZE,
            mount_color: DEFAULT_MOUNT_COLOR,
        }
    }
}

impl Border {
    /// Cream instant-photo frame.
    pub fn polaroid() -> Self {
        Self { enabled: true, size: 0.075, color: [250, 248, 242], fit: BorderFit::Polaroid, ..Self::default() }
    }

    /// Depth of the Polaroid bottom strip, in times the sides, for a
    /// `w` x `h` picture: the chosen depth, or the one that reaches the
    /// target ratio (limited to what the slider offers).
    pub fn polaroid_bottom(&self, w: f32, h: f32) -> f32 {
        let b = self.size.max(0.0) * w.min(h);
        let (lo, hi) = (*POLAROID_BOTTOM_RANGE.start(), *POLAROID_BOTTOM_RANGE.end());
        match self.target {
            Some(t) if b > 0.0 => ((t * (w + 2.0 * b) - h) / b - 1.0).clamp(lo, hi),
            _ => self.bottom,
        }
    }

    /// Whether the target ratio is reached (it may need a strip deeper or
    /// shallower than the slider allows).
    pub fn polaroid_target_reached(&self, w: f32, h: f32) -> bool {
        let Some(t) = self.target else { return true };
        let [ow, oh, _, _] = self.layout(w, h);
        ow > 0.0 && (oh / ow - t).abs() < 0.005
    }

    /// Output size and where the picture sits in it, for a `w` x `h` picture:
    /// (output width, output height, left offset, top offset).
    pub fn layout(&self, w: f32, h: f32) -> [f32; 4] {
        if !self.enabled {
            return [w, h, 0.0, 0.0];
        }
        let b = self.size.max(0.0) * w.min(h);
        if self.fit == BorderFit::Polaroid {
            return [w + 2.0 * b, h + b * (1.0 + self.polaroid_bottom(w, h)), b, b];
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
        crate::corners::apply(&mut out, [ox as f32, oy as f32, w as f32, h as f32], self);
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
    fn polaroid_bottom_strip_and_target_ratio() {
        let mut b = Border::polaroid();
        b.bottom = 1.0;
        let [w, h, ox, _] = b.layout(400.0, 600.0);
        // A 1x strip is as deep as the sides.
        assert!((h - 600.0 - 2.0 * ox).abs() < 1e-3);
        assert!((w - 460.0).abs() < 1e-3);
        // A target ratio sizes the strip to reach it.
        b.target = Some(86.0 / 54.0);
        let [w, h, _, _] = b.layout(400.0, 600.0);
        assert!((h / w - 86.0 / 54.0).abs() < 1e-3);
        assert!(b.polaroid_target_reached(400.0, 600.0));
        // A wide card cannot be reached by a tall picture: limited, and reported.
        b.target = Some(0.8);
        assert!(!b.polaroid_target_reached(400.0, 600.0));
        assert!((b.polaroid_bottom(400.0, 600.0) - 0.5).abs() < 1e-6);
    }

    #[test]
    fn rounded_corners_and_mounts_are_drawn_in_the_border() {
        let photo = RgbaImage::from_pixel(200, 100, Rgba([0, 0, 0, 255]));
        let b = Border { enabled: true, size: 0.1, radius: 0.2, ..Border::default() };
        let out = b.apply(&photo);
        // Photo starts at (10, 10); its very corner is now border colour.
        assert_eq!(out.get_pixel(10, 10)[0], 255);
        assert_eq!(out.get_pixel(110, 60)[0], 0);
        let m = Border { radius: 0.0, mounts: true, mount_color: [200, 0, 0], ..b };
        let out = m.apply(&photo);
        assert_eq!(out.get_pixel(11, 11).0[..3], [200, 0, 0]);
        assert_eq!(out.get_pixel(110, 60)[0], 0);
    }

    #[test]
    fn disabled_changes_nothing() {
        let img = RgbaImage::new(30, 20);
        assert_eq!(Border::default().apply(&img).dimensions(), (30, 20));
    }
}
