//! Comparison with Lightroom: the photos in `references/` were exported
//! from Lightroom with a preset applied. These tests apply the same preset
//! here, the way a saved still is made, and measure how far the result is
//! from Lightroom's.
//!
//! The reference photos are not part of the repository; the tests pass
//! without doing anything when the folder is missing.

use std::path::{Path, PathBuf};

use image::RgbaImage;

/// An original photo, a preset, the Lightroom export made with them, and
/// the largest mean difference allowed (0..255 steps per channel). The
/// measured values when the tone and color steps were fitted to these
/// exports were 2.6, 2.8, 3.8, 7.2 and 5.9. With the code as it was before
/// any fitting the first three were 12.6, 16.5 and 18.2.
const PAIRS: [(&str, &str, &str, f32); 5] = [
    ("IMG_5876-original.JPG", "amit_std.xmp", "IMG_5876-LR.JPG", 4.0),
    ("IMG_5876-original.JPG", "amit_cnt.xmp", "IMG_5876-CNT.JPG", 4.0),
    ("IMG_5876-original.JPG", "amit_tel.xmp", "IMG_5876-TEL.JPG", 5.5),
    // An outdoor photo with sky and leaves. Lightroom's Shadows and Blacks
    // adapt to the picture, which the app's do not, so the dark tones of
    // this photo stay further away.
    ("IMG_5836.JPG", "amit_tel.xmp", "IMG_5836-TEL.jpg", 9.0),
    ("IMG_5836.JPG", "amit_kodak.xmp", "IMG_5836-KODAK.jpg", 7.5),
];

fn references() -> Option<PathBuf> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("references");
    PAIRS.iter().all(|(o, p, e, _)| [o, p, e].iter().all(|f| dir.join(f).exists())).then_some(dir)
}

/// An original photo at the size of the Lightroom exports, in sRGB.
fn original(dir: &Path, name: &str, long: u32) -> RgbaImage {
    let full = crate::app::open_upright(&dir.join(name)).expect("original photo").to_rgba8();
    crate::scene::downscale(&full, long)
}

/// The preset applied the way a saved still gets it.
fn rendered(small: &RgbaImage, dir: &Path, preset: &str, grain: bool) -> RgbaImage {
    let mut look = crate::presets::read(&dir.join(preset)).expect("preset").look;
    if !grain {
        look.grain = 0.0;
    }
    let mut img = small.clone();
    crate::finish::apply_look(&mut img, &look);
    img
}

/// Average colour of each cell of a `cols` x `rows` grid, as 0..1 values.
/// Averages hide grain, sharpening and small differences in position.
fn cells(img: &RgbaImage, cols: u32, rows: u32) -> Vec<[f32; 3]> {
    let (w, h) = (img.width(), img.height());
    let mut out = Vec::with_capacity((cols * rows) as usize);
    for j in 0..rows {
        for i in 0..cols {
            let (x0, x1) = (i * w / cols, ((i + 1) * w / cols).max(i * w / cols + 1));
            let (y0, y1) = (j * h / rows, ((j + 1) * h / rows).max(j * h / rows + 1));
            let mut sum = [0.0f64; 3];
            for y in y0..y1 {
                for x in x0..x1 {
                    let p = img.get_pixel(x, y);
                    for k in 0..3 {
                        sum[k] += p[k] as f64;
                    }
                }
            }
            let n = ((x1 - x0) * (y1 - y0)) as f64 * 255.0;
            out.push([(sum[0] / n) as f32, (sum[1] / n) as f32, (sum[2] / n) as f32]);
        }
    }
    out
}

/// Mean difference between two pictures, in 0..255 steps per channel.
fn mean_difference(a: &RgbaImage, b: &RgbaImage) -> f32 {
    let (ca, cb) = (cells(a, 48, 64), cells(b, 48, 64));
    let total: f32 = ca.iter().zip(&cb).map(|(p, q)| (0..3).map(|k| (p[k] - q[k]).abs()).sum::<f32>()).sum();
    total / (ca.len() * 3) as f32 * 255.0
}

/// Write the pictures used for measuring into the folder named by
/// `ANIMAGRAPH_LR_OUT`: each original at export size, and each preset
/// applied to it without grain. Run with `cargo test lr_match -- --ignored`.
#[test]
#[ignore]
fn write_comparison_pictures() {
    let (Some(dir), Ok(out)) = (references(), std::env::var("ANIMAGRAPH_LR_OUT")) else { return };
    let out = PathBuf::from(out);
    std::fs::create_dir_all(&out).unwrap();
    let stem = |f: &str| f.split('.').next().unwrap().to_string();
    for (photo, preset, _, _) in PAIRS {
        let small = original(&dir, photo, 2048);
        small.save(out.join(format!("{}.png", stem(photo)))).unwrap();
        let img = rendered(&small, &dir, preset, false);
        img.save(out.join(format!("{}+{}.png", stem(photo), stem(preset)))).unwrap();
    }
}

#[test]
fn presets_come_out_close_to_lightroom() {
    let Some(dir) = references() else { return };
    for (photo, preset, export, limit) in PAIRS {
        let small = original(&dir, photo, 2048);
        let lightroom = image::open(dir.join(export)).expect("Lightroom export").to_rgba8();
        assert_eq!((lightroom.width(), lightroom.height()), (small.width(), small.height()), "{export}");
        let before = mean_difference(&small, &lightroom);
        let after = mean_difference(&rendered(&small, &dir, preset, true), &lightroom);
        println!("{export}: unedited {before:.2}, with {preset} {after:.2}");
        assert!(after < limit && after < before, "{export}: {after:.2} steps from Lightroom's export");
    }
}
