//! Camera RAW files (RAF, DNG, CR2/CR3, NEF, ARW, ORF, RW2 and more).
//! On macOS they are developed with the system's own RAW engine (the one
//! the Photos app uses), through the built-in `sips` tool.

use std::path::Path;

use image::RgbaImage;

/// File extensions of camera RAW formats.
pub const EXTENSIONS: [&str; 22] = [
    "raf", "dng", "cr2", "cr3", "crw", "nef", "nrw", "arw", "srf", "sr2", "orf", "rw2", "rwl", "raw", "pef",
    "srw", "3fr", "fff", "iiq", "mos", "erf", "x3f",
];

/// Other formats the app reads through the same system converter
/// (iPhone photos, AVIF).
pub const SYSTEM_FORMATS: [&str; 3] = ["heic", "heif", "avif"];

/// RAW or HEIC: needs the system converter.
pub fn needs_system_decoder(path: &Path) -> bool {
    is_raw(path)
        || path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| SYSTEM_FORMATS.contains(&e.to_ascii_lowercase().as_str()))
            .unwrap_or(false)
}

pub fn is_raw(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| EXTENSIONS.contains(&e.to_ascii_lowercase().as_str()))
        .unwrap_or(false)
}

/// Develop a RAW file into a picture, turned upright.
pub fn decode(path: &Path) -> Result<RgbaImage, String> {
    #[cfg(target_os = "macos")]
    {
        use std::sync::atomic::{AtomicU64, Ordering};
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let tmp = std::env::temp_dir().join(format!(
            "animagraph_raw_{}_{}.jpg",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        // Highest JPEG quality; the camera's orientation tag is kept.
        let out = std::process::Command::new("/usr/bin/sips")
            .args(["-s", "format", "jpeg", "-s", "formatOptions", "100"])
            .arg(path)
            .arg("--out")
            .arg(&tmp)
            .output()
            .map_err(|e| format!("Could not start the RAW converter: {e}"))?;
        if !out.status.success() || !tmp.exists() {
            let _ = std::fs::remove_file(&tmp);
            return Err(if is_raw(path) {
                "macOS could not develop this RAW file. Its camera may be newer than this version of macOS supports."
                    .into()
            } else {
                "macOS could not read this file. It may be damaged, or use a variant this version of macOS does not support."
                    .into()
            });
        }
        let img = crate::app::open_upright(&tmp).map_err(|e| e.to_string());
        let _ = std::fs::remove_file(&tmp);
        Ok(img?.to_rgba8())
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = path;
        Err("RAW, HEIC and AVIF files can be opened on macOS only. Convert it to JPEG or TIFF first.".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognizes_raw_extensions() {
        assert!(is_raw(Path::new("/x/DSCF0001.RAF")));
        assert!(is_raw(Path::new("a.dng")));
        assert!(is_raw(Path::new("b.CR3")));
        assert!(!is_raw(Path::new("c.jpg")));
        assert!(!is_raw(Path::new("d.mov")));
        assert!(needs_system_decoder(Path::new("IMG_0001.HEIC")));
        assert!(!needs_system_decoder(Path::new("e.png")));
    }

    /// The same developing path, checked with a TIFF made on the spot
    /// (no RAW file ships with the app).
    #[cfg(target_os = "macos")]
    #[test]
    fn develops_through_the_system_converter() {
        let dir = std::env::temp_dir().join(format!("animagraph_rawtest_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let src = dir.join("in.tiff");
        RgbaImage::from_pixel(64, 40, image::Rgba([200, 120, 40, 255])).save(&src).unwrap();
        let img = decode(&src).unwrap();
        assert_eq!(img.dimensions(), (64, 40));
        let p = img.get_pixel(10, 10);
        assert!((p[0] as i32 - 200).abs() < 12 && (p[2] as i32 - 40).abs() < 12, "{p:?}");
        let _ = std::fs::remove_dir_all(dir);
    }
}
