//! Colour management for still photos.
//!
//! Animagraph edits and shows pictures as sRGB. Photos often carry another
//! colour profile: iPhone HEIC and JPEG photos use Display P3. On import the
//! pixels are converted from their profile to sRGB (so colours look as they
//! do in Photos and Lightroom), and every saved still carries an sRGB profile
//! (so other apps do not have to guess).

use image::RgbaImage;

/// Convert a picture from its embedded profile to sRGB. Pictures without a
/// profile, or with one that cannot be read, are left as they are (treated
/// as sRGB).
pub fn to_srgb(img: &mut RgbaImage, icc: &[u8]) -> bool {
    let Some(input) = qcms::Profile::new_from_slice(icc, false) else { return false };
    let output = qcms::Profile::new_sRGB();
    let Some(t) = qcms::Transform::new(&input, &output, qcms::DataType::RGBA8, qcms::Intent::Perceptual) else {
        return false;
    };
    // In parallel, a band of rows at a time.
    let row = img.width() as usize * 4;
    if row == 0 {
        return false;
    }
    let data: &mut [u8] = img;
    let rows = data.len() / row;
    let threads = std::thread::available_parallelism().map_or(4, |n| n.get()).max(1);
    let band = rows.div_ceil(threads).max(1) * row;
    std::thread::scope(|s| {
        for chunk in data.chunks_mut(band) {
            let t = &t;
            s.spawn(move || t.apply(chunk));
        }
    });
    true
}

// ----- An sRGB profile to attach to saved pictures ------------------------

/// Primaries of sRGB, adapted to D50 as ICC profiles store them.
const SRGB_PRIMARIES: [[f64; 3]; 3] = [
    [0.436_065, 0.222_488, 0.013_916],
    [0.385_147, 0.716_873, 0.097_076],
    [0.143_066, 0.060_608, 0.714_096],
];

/// The ICC profile written into saved PNG, JPEG and HEIC files: sRGB
/// (IEC 61966-2.1 primaries and tone curve), version 2, made here.
pub fn srgb_icc() -> Vec<u8> {
    matrix_profile("sRGB IEC61966-2.1", SRGB_PRIMARIES)
}

/// Display P3 (for tests): P3 primaries, sRGB tone curve, D50-adapted.
#[cfg(test)]
pub fn display_p3_icc() -> Vec<u8> {
    matrix_profile(
        "Display P3",
        [[0.515_102, 0.241_196, -0.001_053], [0.291_965, 0.692_236, 0.041_882], [0.157_153, 0.066_561, 0.784_073]],
    )
}

/// A display profile with three primaries (columns of the RGB to XYZ
/// matrix, D50) and the sRGB tone curve on each channel.
fn matrix_profile(name: &str, primaries: [[f64; 3]; 3]) -> Vec<u8> {
    fn s15(v: f64) -> [u8; 4] {
        ((v * 65536.0).round() as i32).to_be_bytes()
    }
    fn xyz(v: [f64; 3]) -> Vec<u8> {
        let mut t = b"XYZ \0\0\0\0".to_vec();
        for c in v {
            t.extend_from_slice(&s15(c));
        }
        t
    }
    fn desc(text: &str) -> Vec<u8> {
        let mut t = b"desc\0\0\0\0".to_vec();
        t.extend_from_slice(&(text.len() as u32 + 1).to_be_bytes());
        t.extend_from_slice(text.as_bytes());
        t.push(0);
        t.extend_from_slice(&[0; 8]); // no Unicode text
        t.extend_from_slice(&[0; 3]); // no ScriptCode text
        t.extend_from_slice(&[0; 67]);
        t
    }
    fn text(s: &str) -> Vec<u8> {
        let mut t = b"text\0\0\0\0".to_vec();
        t.extend_from_slice(s.as_bytes());
        t.push(0);
        t
    }
    // sRGB tone curve as a 1024-entry table.
    let mut curve = b"curv\0\0\0\0".to_vec();
    let n = 1024u32;
    curve.extend_from_slice(&n.to_be_bytes());
    for i in 0..n {
        let v = i as f64 / (n - 1) as f64;
        let lin = if v <= 0.04045 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) };
        curve.extend_from_slice(&((lin * 65535.0).round() as u16).to_be_bytes());
    }
    let d50 = [0.9642, 1.0, 0.8249];
    let tags: Vec<(&[u8; 4], Vec<u8>)> = vec![
        (b"desc", desc(name)),
        (b"cprt", text("No copyright, use freely")),
        (b"wtpt", xyz(d50)),
        (b"rXYZ", xyz(primaries[0])),
        (b"gXYZ", xyz(primaries[1])),
        (b"bXYZ", xyz(primaries[2])),
        (b"rTRC", curve.clone()),
        (b"gTRC", curve.clone()),
        (b"bTRC", curve),
    ];
    // Header, tag table, then the tags (each on a 4-byte boundary).
    let table_len = 4 + 12 * tags.len();
    let mut body = Vec::new();
    let mut table = (tags.len() as u32).to_be_bytes().to_vec();
    for (sig, data) in &tags {
        while body.len() % 4 != 0 {
            body.push(0);
        }
        let offset = 128 + table_len + body.len();
        table.extend_from_slice(*sig);
        table.extend_from_slice(&(offset as u32).to_be_bytes());
        table.extend_from_slice(&(data.len() as u32).to_be_bytes());
        body.extend_from_slice(data);
    }
    while body.len() % 4 != 0 {
        body.push(0);
    }
    let size = 128 + table_len + body.len();
    let mut h = Vec::with_capacity(size);
    h.extend_from_slice(&(size as u32).to_be_bytes());
    h.extend_from_slice(&[0; 4]); // preferred CMM
    h.extend_from_slice(&[2, 0x10, 0, 0]); // version 2.1
    h.extend_from_slice(b"mntr");
    h.extend_from_slice(b"RGB ");
    h.extend_from_slice(b"XYZ ");
    for v in [2026u16, 1, 1, 0, 0, 0] {
        h.extend_from_slice(&v.to_be_bytes()); // date
    }
    h.extend_from_slice(b"acsp");
    h.extend_from_slice(&[0; 4]); // platform
    h.extend_from_slice(&[0; 4]); // flags
    h.extend_from_slice(&[0; 4]); // manufacturer
    h.extend_from_slice(&[0; 4]); // model
    h.extend_from_slice(&[0; 8]); // attributes
    h.extend_from_slice(&[0; 4]); // perceptual intent
    for c in d50 {
        h.extend_from_slice(&s15(c));
    }
    h.extend_from_slice(&[0; 4]); // creator
    h.extend_from_slice(&[0; 16]); // profile id
    h.extend_from_slice(&[0; 28]); // reserved
    debug_assert_eq!(h.len(), 128);
    h.extend_from_slice(&table);
    h.extend_from_slice(&body);
    h
}


// ----- A wide working space for edits ---------------------------------------

/// Linear sRGB to linear ProPhoto RGB (both as light, not encoded values).
const SRGB_TO_PROPHOTO: [[f32; 3]; 3] =
    [[0.5293, 0.3300, 0.1407], [0.0983, 0.8734, 0.0283], [0.0169, 0.1176, 0.8655]];

/// A color space between sRGB and ProPhoto RGB, for the edits Lightroom
/// makes in its own wide space. `width` 0 is sRGB and 1 is ProPhoto RGB.
pub struct Wide {
    to: [[f32; 3]; 3],
    from: [[f32; 3]; 3],
}

impl Wide {
    pub fn new(width: f32) -> Self {
        let mut to = [[0.0f32; 3]; 3];
        for (i, row) in to.iter_mut().enumerate() {
            for (j, v) in row.iter_mut().enumerate() {
                let same = if i == j { 1.0 } else { 0.0 };
                *v = same + (SRGB_TO_PROPHOTO[i][j] - same) * width;
            }
        }
        Wide { to, from: inverse(&to) }
    }

    /// Light in sRGB to light in the wide space.
    pub fn to_wide(&self, c: [f32; 3]) -> [f32; 3] {
        times(&self.to, c)
    }

    /// Light in the wide space back to light in sRGB.
    pub fn to_srgb(&self, c: [f32; 3]) -> [f32; 3] {
        times(&self.from, c)
    }
}

fn times(m: &[[f32; 3]; 3], c: [f32; 3]) -> [f32; 3] {
    [0, 1, 2].map(|i| m[i][0] * c[0] + m[i][1] * c[1] + m[i][2] * c[2])
}

fn inverse(m: &[[f32; 3]; 3]) -> [[f32; 3]; 3] {
    let cof = |r: usize, c: usize| {
        let (r1, r2, c1, c2) = ((r + 1) % 3, (r + 2) % 3, (c + 1) % 3, (c + 2) % 3);
        m[r1][c1] * m[r2][c2] - m[r1][c2] * m[r2][c1]
    };
    let det = m[0][0] * cof(0, 0) + m[0][1] * cof(0, 1) + m[0][2] * cof(0, 2);
    let mut out = [[0.0f32; 3]; 3];
    for (i, row) in out.iter_mut().enumerate() {
        for (j, v) in row.iter_mut().enumerate() {
            *v = cof(j, i) / det;
        }
    }
    out
}

/// An encoded sRGB value (0..1) as light.
pub fn light(v: f32) -> f32 {
    if v <= 0.04045 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) }
}

/// Light as an encoded sRGB value. Negative light stays negative.
pub fn encoded(v: f32) -> f32 {
    if v <= 0.003_130_8 { v * 12.92 } else { 1.055 * v.powf(1.0 / 2.4) - 0.055 }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn px(r: u8, g: u8, b: u8) -> RgbaImage {
        RgbaImage::from_pixel(1, 1, image::Rgba([r, g, b, 255]))
    }

    #[test]
    fn our_srgb_profile_reads_back_and_changes_nothing() {
        let icc = srgb_icc();
        assert_eq!(u32::from_be_bytes(icc[0..4].try_into().unwrap()) as usize, icc.len());
        for c in [[0, 0, 0], [255, 255, 255], [200, 30, 60], [12, 140, 220], [128, 128, 128]] {
            let mut img = px(c[0], c[1], c[2]);
            assert!(to_srgb(&mut img, &icc));
            let o = img.get_pixel(0, 0);
            for k in 0..3 {
                assert!((o[k] as i32 - c[k] as i32).abs() <= 2, "{c:?} became {:?}", o);
            }
        }
    }

    #[test]
    fn display_p3_colours_come_out_stronger_in_srgb() {
        // The same numbers mean a more saturated colour in P3, so in sRGB
        // they need larger differences between channels.
        let mut img = px(200, 90, 80);
        assert!(to_srgb(&mut img, &display_p3_icc()));
        let o = img.get_pixel(0, 0);
        assert!(o[0] > 200 && o[1] < 90, "{:?}", o);
        // Grey stays grey.
        let mut grey = px(128, 128, 128);
        to_srgb(&mut grey, &display_p3_icc());
        let g = grey.get_pixel(0, 0);
        assert!((g[0] as i32 - g[1] as i32).abs() <= 2 && (g[1] as i32 - g[2] as i32).abs() <= 2);
    }

    #[test]
    fn wide_space_round_trip_and_gray() {
        let wide = Wide::new(1.0);
        for c in [[0.2f32, 0.5, 0.9], [1.0, 0.0, 0.0], [0.4, 0.4, 0.4]] {
            let back = wide.to_srgb(wide.to_wide(c));
            for k in 0..3 {
                assert!((back[k] - c[k]).abs() < 1e-4, "{c:?} became {back:?}");
            }
        }
        // Gray stays gray, and a strong blue is weaker in the wide space.
        let g = wide.to_wide([0.4, 0.4, 0.4]);
        assert!((g[0] - 0.4).abs() < 1e-3 && (g[2] - 0.4).abs() < 1e-3, "{g:?}");
        let b = wide.to_wide([0.05, 0.2, 0.8]);
        assert!(b[0] > 0.05 && b[2] < 0.8, "{b:?}");
        assert!((encoded(light(0.3)) - 0.3).abs() < 1e-5);
    }

    #[test]
    fn unreadable_profile_leaves_the_picture_alone() {
        let mut img = px(10, 20, 30);
        assert!(!to_srgb(&mut img, b"not a profile"));
        assert_eq!(img.get_pixel(0, 0).0, [10, 20, 30, 255]);
    }
}
