//! Saving one finished still: colour edits, masks, crop, caption and border,
//! at the chosen size. Used by "Save image" and by saving all photos.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use image::RgbaImage;

use crate::crop::CropRect;
use crate::export::Format;
use crate::export_size::{ExportSize, natural_size};
use crate::scene::Edits;

pub struct StillJob {
    pub full: std::sync::Arc<RgbaImage>,
    pub crop: CropRect,
    pub edits: Edits,
    pub size: ExportSize,
    pub format: Format,
    pub target: PathBuf,
}

impl StillJob {
    pub fn render(&self) -> RgbaImage {
        let full = self.full.dimensions();
        let natural = natural_size(full, self.crop, &self.edits.border);
        let long = self.size.source_long(full, natural);
        let img = crate::app::framed_still(&self.full, self.crop, &self.edits, long);
        self.size.exact(img, natural)
    }

    pub fn run(self) -> Result<PathBuf, String> {
        crate::export::save_still(self.format, &self.render(), &self.target)
    }
}

/// A file name in `folder` that is neither on disk nor in `taken`: a number
/// is added when needed, so nothing is overwritten.
pub fn unique_target(folder: &Path, base: &str, ext: &str, taken: &HashSet<PathBuf>) -> PathBuf {
    (1..)
        .map(|n| folder.join(if n == 1 { format!("{base}.{ext}") } else { format!("{base} {n}.{ext}") }))
        .find(|p| !p.exists() && !taken.contains(p))
        .expect("some number is free")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::border::Border;
    use crate::mask::{Mask, Shape};

    fn job(size: ExportSize, edits: Edits, crop: CropRect) -> StillJob {
        let full = RgbaImage::from_pixel(400, 300, image::Rgba([100, 100, 100, 255]));
        StillJob { full: full.into(), crop, edits, size, format: Format::Png, target: PathBuf::new() }
    }

    #[test]
    fn saved_size_is_exact_with_crop_and_border() {
        let crop = CropRect { x: 0.1, y: 0.0, w: 0.51, h: 0.97 };
        for border in [Border::default(), Border::polaroid(), Border { enabled: true, size: 0.033, ..Border::default() }] {
            for px in [97, 108, 151] {
                let size = ExportSize { side: crate::export_size::Side::Width, px };
                let img = job(size, Edits { border, ..Edits::default() }, crop).render();
                assert_eq!(img.width(), px);
            }
        }
    }

    #[test]
    fn masks_are_in_the_saved_picture() {
        let mut m = Mask::radial("R".into());
        m.shape = Shape::Radial { center: [0.25, 0.5], radius: [0.2, 0.4], feather: 0.2 };
        m.local.exposure = 1.0;
        let edits = Edits { masks: vec![m], ..Edits::default() };
        let img = job(ExportSize::ORIGINAL, edits, CropRect::FULL).render();
        assert!(img.get_pixel(100, 150)[0] > 120);
        assert_eq!(img.get_pixel(380, 150)[0], 100);
    }

    #[test]
    fn names_never_overwrite() {
        let dir = std::env::temp_dir().join(format!("animagraph_unique_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("a.png"), b"x").unwrap();
        let mut taken = HashSet::new();
        let second = unique_target(&dir, "a", "png", &taken);
        assert_eq!(second, dir.join("a 2.png"));
        taken.insert(second);
        assert_eq!(unique_target(&dir, "a", "png", &taken), dir.join("a 3.png"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
