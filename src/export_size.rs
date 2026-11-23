//! The size of a saved picture. A size is measured on the finished picture
//! (after crop and border), so "1080 px wide" is what the file really is.
//! Pictures are never enlarged.

use image::RgbaImage;

use crate::border::Border;
use crate::crop::CropRect;

/// Which side of the saved picture the number of pixels is for.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Side {
    Long,
    Width,
    Height,
}

impl Side {
    pub const ALL: [Side; 3] = [Side::Long, Side::Width, Side::Height];

    pub fn label(self) -> &'static str {
        match self {
            Side::Long => "long side",
            Side::Width => "wide",
            Side::Height => "high",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ExportSize {
    pub side: Side,
    /// 0 keeps the original size.
    pub px: u32,
}

/// Smallest and largest custom size.
pub const CUSTOM_RANGE: std::ops::RangeInclusive<u32> = 16..=20000;

impl ExportSize {
    pub const ORIGINAL: Self = Self { side: Side::Long, px: 0 };
    /// Instagram shows posts 1080 px wide.
    pub const INSTAGRAM: Self = Self { side: Side::Width, px: 1080 };

    pub const fn long(px: u32) -> Self {
        Self { side: Side::Long, px }
    }

    pub fn label(self) -> String {
        if self == Self::INSTAGRAM {
            return "Instagram · 1080 px wide".into();
        }
        match (self.px, self.side) {
            (0, _) => "Original".into(),
            (px, Side::Long) => format!("{px} px"),
            (px, side) => format!("{px} px {}", side.label()),
        }
    }

    /// How much the finished picture is reduced (1 keeps it as it is).
    fn scale(self, natural: (u32, u32)) -> f64 {
        if self.px == 0 {
            return 1.0;
        }
        let side = match self.side {
            Side::Long => natural.0.max(natural.1),
            Side::Width => natural.0,
            Side::Height => natural.1,
        };
        (self.px as f64 / side.max(1) as f64).min(1.0)
    }

    /// Whether the picture is smaller than the size asked for.
    pub fn too_big(self, natural: (u32, u32)) -> bool {
        self.px != 0
            && match self.side {
                Side::Long => self.px > natural.0.max(natural.1),
                Side::Width => self.px > natural.0,
                Side::Height => self.px > natural.1,
            }
    }

    /// Size of the saved file for a finished picture of `natural` size.
    pub fn output(self, natural: (u32, u32)) -> (u32, u32) {
        let k = self.scale(natural);
        let f = |v: u32| ((v as f64 * k).round() as u32).max(1);
        (f(natural.0), f(natural.1))
    }

    /// Long side the whole photo is reduced to before it is finished, so the
    /// finished picture comes out at (or within a pixel of) `output`.
    pub fn source_long(self, full: (u32, u32), natural: (u32, u32)) -> u32 {
        let long = full.0.max(full.1);
        ((long as f64 * self.scale(natural)).round() as u32).clamp(1, long.max(1))
    }

    /// Correct the last pixel or two left over from rounding.
    pub fn exact(self, img: RgbaImage, natural: (u32, u32)) -> RgbaImage {
        let (w, h) = self.output(natural);
        if self.scale(natural) >= 1.0 || img.dimensions() == (w, h) {
            return img;
        }
        image::imageops::resize(&img, w, h, image::imageops::FilterType::Triangle)
    }
}

/// Size of the finished picture at full quality: crop, then border.
pub fn natural_size(full: (u32, u32), crop: CropRect, border: &Border) -> (u32, u32) {
    let [_, _, w, h] = crop.pixels(full.0, full.1);
    let [ow, oh, _, _] = border.layout(w as f32, h as f32);
    ((ow.round() as u32).max(w), (oh.round() as u32).max(h))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sizes_are_measured_on_the_saved_picture() {
        assert_eq!(ExportSize::INSTAGRAM.output((3024, 4032)), (1080, 1440));
        assert_eq!(ExportSize::long(1000).output((3024, 4032)), (750, 1000));
        assert_eq!(ExportSize { side: Side::Height, px: 500 }.output((2000, 1000)), (1000, 500));
        assert_eq!(ExportSize::ORIGINAL.output((3024, 4032)), (3024, 4032));
    }

    #[test]
    fn pictures_are_not_enlarged() {
        assert_eq!(ExportSize::INSTAGRAM.output((800, 600)), (800, 600));
        assert!(ExportSize::INSTAGRAM.too_big((800, 600)));
        assert!(!ExportSize::INSTAGRAM.too_big((1080, 600)));
    }
}
