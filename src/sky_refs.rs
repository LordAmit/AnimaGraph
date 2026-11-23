//! Sky detector on the real reference photos (tests only). Every photo has
//! marked points, in pixels of the photo, that must or must not be sky. The
//! tests do nothing when the photos are not there.

use image::RgbaImage;

struct Photo {
    name: &'static str,
    sky: &'static [(u32, u32)],
    not_sky: &'static [(u32, u32)],
}

const PHOTOS: [Photo; 9] = [
    Photo {
        name: "01",
        sky: &[(100, 100), (900, 300), (700, 600), (100, 760), (950, 760)],
        not_sky: &[(400, 560), (383, 750), (100, 830), (900, 840), (500, 1300), (750, 990)],
    },
    Photo {
        name: "02",
        sky: &[(100, 100), (800, 400), (450, 550), (1000, 780), (290, 620)],
        not_sky: &[
            (575, 250), (50, 535), (150, 660), (243, 670), (338, 645), (437, 640), (517, 645), (500, 780), (250, 800),
            (600, 900), (300, 1100), (845, 800), (250, 780),
        ],
    },
    Photo {
        name: "03",
        sky: &[(100, 100), (800, 60), (500, 400), (100, 600), (1000, 590)],
        not_sky: &[(500, 620), (1050, 740), (100, 700), (950, 680), (500, 1300), (900, 800), (200, 860)],
    },
    Photo {
        name: "04",
        sky: &[(300, 100), (900, 400), (600, 1000), (600, 1130), (200, 1000), (1000, 1100)],
        not_sky: &[
            (350, 1250), (800, 1250), (980, 1250), (680, 1235), (500, 720), (460, 900), (682, 900), (230, 1450),
            (600, 1450), (50, 1350),
        ],
    },
    Photo {
        name: "05",
        sky: &[(100, 100), (700, 300), (400, 650), (900, 680), (600, 500)],
        not_sky: &[(100, 700), (790, 700), (500, 760), (350, 840), (520, 890), (650, 840), (1000, 830), (500, 1300), (800, 500)],
    },
    Photo {
        name: "06",
        sky: &[(100, 100), (900, 100), (100, 400), (950, 400), (800, 660), (250, 660)],
        not_sky: &[(575, 220), (495, 220), (420, 240), (670, 240), (460, 298), (613, 298), (525, 470), (435, 530), (190, 580)],
    },
    Photo {
        name: "07",
        sky: &[(100, 100), (900, 300), (500, 450), (560, 680), (360, 720)],
        not_sky: &[(570, 570), (240, 710), (340, 830), (800, 760), (930, 670), (300, 1050), (500, 1350), (800, 880)],
    },
    Photo {
        name: "08",
        sky: &[(100, 200), (900, 300), (200, 900), (900, 900), (100, 1300), (1020, 1400)],
        not_sky: &[(150, 645), (660, 678), (600, 600), (600, 1020), (530, 1090), (590, 1400), (385, 1100)],
    },
    Photo {
        name: "09",
        sky: &[(100, 100), (900, 200), (30, 800), (1060, 800), (100, 1180), (300, 1180)],
        not_sky: &[(515, 345), (595, 345), (430, 375), (650, 370), (520, 440), (295, 455), (755, 455), (530, 800), (540, 1200), (900, 1150)],
    },
];

fn load(name: &str) -> Option<RgbaImage> {
    let p = format!("{}/../reference/{name}-before.webp", env!("CARGO_MANIFEST_DIR"));
    let img = crate::app::open_upright(std::path::Path::new(&p)).ok()?.to_rgba8();
    Some(crate::scene::downscale(&img, crate::mask::BRUSH_SIZE))
}

/// Each marked point with its mask value; points are given at the photo's
/// own size.
fn wrong_points(p: &Photo, img: &RgbaImage, m: &[f32], long: u32) -> Vec<String> {
    let k = img.width().max(img.height()) as f32 / long as f32;
    let at = |(x, y): (u32, u32)| {
        let (x, y) = ((x as f32 * k) as u32, (y as f32 * k) as u32);
        m[(y.min(img.height() - 1) * img.width() + x.min(img.width() - 1)) as usize]
    };
    let sky = p.sky.iter().filter(|q| at(**q) < 0.5).map(|q| format!("{}: sky missed at {q:?}", p.name));
    let not = p.not_sky.iter().filter(|q| at(**q) > 0.5).map(|q| format!("{}: not sky, but selected at {q:?}", p.name));
    sky.chain(not).collect()
}

fn original_long(name: &str) -> u32 {
    let p = format!("{}/../reference/{name}-before.webp", env!("CARGO_MANIFEST_DIR"));
    image::image_dimensions(p).map_or(1, |(w, h)| w.max(h))
}

#[test]
fn marked_points_in_the_reference_photos() {
    let mut wrong = Vec::new();
    for p in &PHOTOS {
        let Some(img) = load(p.name) else { continue };
        let m = crate::sky::sky_mask(&img);
        wrong.extend(wrong_points(p, &img, &m, original_long(p.name)));
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

/// Side-by-side pictures: the photo, and the photo with the mask in green.
/// Run with: cargo test side_by_side -- --ignored
#[test]
#[ignore]
fn side_by_side() {
    let (tw, gap) = (420u32, 8u32);
    for (n, group) in PHOTOS.chunks(2).enumerate() {
        let mut tiles = Vec::new();
        for p in group {
            let Some(img) = load(p.name) else { continue };
            let m = crate::sky::sky_mask(&img);
            let mut out = img.clone();
            for (px, a) in out.pixels_mut().zip(&m) {
                for (k, c) in [0.0, 255.0, 60.0].into_iter().enumerate() {
                    px[k] = (px[k] as f32 * (1.0 - a * 0.6) + c * a * 0.6) as u8;
                }
            }
            for t in [&img, &out] {
                let th = tw * t.height() / t.width();
                tiles.push(image::imageops::resize(t, tw, th, image::imageops::FilterType::Triangle));
            }
        }
        let Some(th) = tiles.iter().map(|t| t.height()).max() else { continue };
        let mut sheet = RgbaImage::from_pixel((tw + gap) * tiles.len() as u32, th, image::Rgba([20, 20, 20, 255]));
        for (i, t) in tiles.iter().enumerate() {
            image::imageops::replace(&mut sheet, t, (i as u32 * (tw + gap)) as i64, 0);
        }
        sheet.save(format!("{}/shots/sky_{n}.png", env!("CARGO_MANIFEST_DIR"))).unwrap();
    }
}
