//! Crop box with an optional locked aspect ratio. The box is stored in 0..1
//! fractions of the picture so it works at any size.

use image::RgbaImage;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Aspect {
    Free,
    Original,
    Square,
    Portrait45,
    Portrait34,
    Portrait23,
    Story,
    Classic43,
    Photo32,
    Wide,
}

impl Aspect {
    pub const ALL: [Aspect; 10] = [
        Aspect::Free,
        Aspect::Original,
        Aspect::Square,
        Aspect::Portrait45,
        Aspect::Portrait34,
        Aspect::Portrait23,
        Aspect::Story,
        Aspect::Classic43,
        Aspect::Photo32,
        Aspect::Wide,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Aspect::Free => "Free",
            Aspect::Original => "Original",
            Aspect::Square => "1:1",
            Aspect::Portrait45 => "4:5",
            Aspect::Portrait34 => "3:4",
            Aspect::Portrait23 => "2:3",
            Aspect::Story => "9:16",
            Aspect::Classic43 => "4:3",
            Aspect::Photo32 => "3:2",
            Aspect::Wide => "16:9",
        }
    }

    pub fn hint(self) -> &'static str {
        match self {
            Aspect::Free => "Any shape",
            Aspect::Original => "Same shape as the picture",
            Aspect::Square => "Square posts",
            Aspect::Portrait45 => "Tall feed posts",
            Aspect::Portrait34 => "Portrait",
            Aspect::Portrait23 => "Portrait photo",
            Aspect::Story => "Stories and reels",
            Aspect::Classic43 => "Classic screen",
            Aspect::Photo32 => "Landscape photo",
            Aspect::Wide => "Widescreen video",
        }
    }

    /// Width divided by height in pixels, `None` when free.
    pub fn ratio(self, img_w: u32, img_h: u32) -> Option<f32> {
        match self {
            Aspect::Free => None,
            Aspect::Original => Some(img_w as f32 / img_h as f32),
            Aspect::Square => Some(1.0),
            Aspect::Portrait45 => Some(4.0 / 5.0),
            Aspect::Portrait34 => Some(3.0 / 4.0),
            Aspect::Portrait23 => Some(2.0 / 3.0),
            Aspect::Story => Some(9.0 / 16.0),
            Aspect::Classic43 => Some(4.0 / 3.0),
            Aspect::Photo32 => Some(3.0 / 2.0),
            Aspect::Wide => Some(16.0 / 9.0),
        }
    }
}

/// The kept part of the picture as fractions: x, y, width, height.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct CropRect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Default for CropRect {
    fn default() -> Self {
        Self::FULL
    }
}

impl CropRect {
    pub const FULL: CropRect = CropRect { x: 0.0, y: 0.0, w: 1.0, h: 1.0 };

    pub fn is_full(&self) -> bool {
        self.x <= 1e-4 && self.y <= 1e-4 && self.w >= 1.0 - 1e-4 && self.h >= 1.0 - 1e-4
    }

    pub fn from_corners(a: [f32; 2], b: [f32; 2]) -> Self {
        let x0 = a[0].min(b[0]).clamp(0.0, 1.0);
        let y0 = a[1].min(b[1]).clamp(0.0, 1.0);
        let x1 = a[0].max(b[0]).clamp(0.0, 1.0);
        let y1 = a[1].max(b[1]).clamp(0.0, 1.0);
        Self { x: x0, y: y0, w: x1 - x0, h: y1 - y0 }
    }

    pub fn center(&self) -> [f32; 2] {
        [self.x + self.w / 2.0, self.y + self.h / 2.0]
    }

    /// Size in pixels of a picture `img_w` x `img_h`.
    pub fn pixels(&self, img_w: u32, img_h: u32) -> [u32; 4] {
        let x = (self.x * img_w as f32).round() as u32;
        let y = (self.y * img_h as f32).round() as u32;
        let w = ((self.w * img_w as f32).round() as u32).clamp(1, img_w.saturating_sub(x).max(1));
        let h = ((self.h * img_h as f32).round() as u32).clamp(1, img_h.saturating_sub(y).max(1));
        [x, y, w, h]
    }

    pub fn apply(&self, img: &RgbaImage) -> RgbaImage {
        if self.is_full() {
            return img.clone();
        }
        let [x, y, w, h] = self.pixels(img.width(), img.height());
        image::imageops::crop_imm(img, x, y, w, h).to_image()
    }

    /// Move without changing size, kept inside the picture.
    pub fn moved(&self, dx: f32, dy: f32) -> Self {
        Self {
            x: (self.x + dx).clamp(0.0, 1.0 - self.w),
            y: (self.y + dy).clamp(0.0, 1.0 - self.h),
            ..*self
        }
    }

    /// The largest box with `ratio` that fits the picture, centered on `center`.
    pub fn largest(ratio: f32, center: [f32; 2], img_w: u32, img_h: u32) -> Self {
        // In fractions: w / h = ratio * img_h / img_w.
        let rf = ratio * img_h as f32 / img_w as f32;
        let (w, h) = if rf >= 1.0 { (1.0, 1.0 / rf) } else { (rf, 1.0) };
        Self { x: center[0] - w / 2.0, y: center[1] - h / 2.0, w, h }.moved(0.0, 0.0)
    }

    /// Fit this box to an aspect ratio, keeping its center and roughly its size.
    pub fn with_aspect(&self, aspect: Aspect, img_w: u32, img_h: u32) -> Self {
        let Some(ratio) = aspect.ratio(img_w, img_h) else { return *self };
        let rf = ratio * img_h as f32 / img_w as f32;
        let area = (self.w * self.h).max(0.05);
        let mut w = (area * rf).sqrt();
        let mut h = w / rf;
        let fit = (1.0 / w).min(1.0 / h).min(1.0);
        w *= fit;
        h *= fit;
        let c = self.center();
        Self { x: c[0] - w / 2.0, y: c[1] - h / 2.0, w, h }.moved(0.0, 0.0)
    }

    /// A box from a fixed corner `anchor` to the pointer `p`, respecting the
    /// aspect ratio and the picture edges.
    pub fn from_anchor(anchor: [f32; 2], p: [f32; 2], aspect: Aspect, img_w: u32, img_h: u32) -> Self {
        let p = [p[0].clamp(0.0, 1.0), p[1].clamp(0.0, 1.0)];
        let Some(ratio) = aspect.ratio(img_w, img_h) else {
            return Self::from_corners(anchor, p);
        };
        let rf = ratio * img_h as f32 / img_w as f32;
        let sx = if p[0] >= anchor[0] { 1.0 } else { -1.0 };
        let sy = if p[1] >= anchor[1] { 1.0 } else { -1.0 };
        let mut w = (p[0] - anchor[0]).abs();
        let mut h = (p[1] - anchor[1]).abs();
        if w / h.max(1e-6) > rf {
            h = w / rf;
        } else {
            w = h * rf;
        }
        // Shrink to stay inside the picture.
        let room_x = if sx > 0.0 { 1.0 - anchor[0] } else { anchor[0] };
        let room_y = if sy > 0.0 { 1.0 - anchor[1] } else { anchor[1] };
        let s = (room_x / w.max(1e-6)).min(room_y / h.max(1e-6)).min(1.0);
        w *= s;
        h *= s;
        Self::from_corners(anchor, [anchor[0] + sx * w, anchor[1] + sy * h])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn locked_ratio_holds_in_pixels() {
        let r = CropRect::from_anchor([0.1, 0.1], [0.9, 0.5], Aspect::Square, 1600, 900);
        let [_, _, w, h] = r.pixels(1600, 900);
        assert!((w as i32 - h as i32).abs() <= 1, "{w}x{h}");
        assert!(r.x >= 0.0 && r.y >= 0.0 && r.x + r.w <= 1.0 + 1e-5 && r.y + r.h <= 1.0 + 1e-5);
    }

    #[test]
    fn largest_story_crop_is_full_height() {
        let r = CropRect::largest(9.0 / 16.0, [0.5, 0.5], 1600, 900);
        let [_, _, w, h] = r.pixels(1600, 900);
        assert_eq!(h, 900);
        assert!((w as i32 - 506).abs() <= 1);
    }

    #[test]
    fn free_crop_and_apply() {
        let img = RgbaImage::new(200, 100);
        let r = CropRect::from_anchor([0.25, 0.2], [0.75, 0.8], Aspect::Free, 200, 100);
        assert_eq!(r.apply(&img).dimensions(), (100, 60));
        assert_eq!(CropRect::FULL.apply(&img).dimensions(), (200, 100));
    }

    #[test]
    fn moving_stays_inside() {
        let r = CropRect { x: 0.5, y: 0.5, w: 0.4, h: 0.4 }.moved(0.5, -0.9);
        assert!((r.x - 0.6).abs() < 1e-6 && r.y == 0.0);
    }

    #[test]
    fn switching_aspect_keeps_center() {
        let r = CropRect { x: 0.2, y: 0.2, w: 0.4, h: 0.4 }.with_aspect(Aspect::Wide, 1000, 1000);
        let c = r.center();
        assert!((c[0] - 0.4).abs() < 1e-4 && (c[1] - 0.4).abs() < 1e-4);
        assert!(((r.w * 1000.0) / (r.h * 1000.0) - 16.0 / 9.0).abs() < 1e-3);
    }
}
