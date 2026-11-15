//! Apple Log footage (iPhone "Log" video) to normal display colours.
//!
//! Apple Log stores a flat, log-encoded picture in Rec.2020 colours so it
//! can be graded later. Shown as is it looks grey and washed out. This turns
//! it into an ordinary sRGB picture: undo the log curve (Apple Log Profile
//! White Paper), move the colours from Rec.2020 to Rec.709 / sRGB, roll off
//! the highlights, and encode for the screen.

use image::RgbaImage;

// Apple Log curve constants (Apple Log Profile White Paper).
const R0: f32 = -0.056_410_88;
const RT: f32 = 0.01;
const C: f32 = 47.287_112;
const B: f32 = 0.009_640_52;
const GAMMA: f32 = 0.085_504_79;
const BETA: f32 = 0.693_369_45;

/// Apple Log signal (0..1) to scene-linear light (0.18 is middle grey).
pub fn decode(p: f32) -> f32 {
    let pt = C * (RT - R0) * (RT - R0);
    if p >= pt {
        (2f32).powf((p - BETA) / GAMMA) - B
    } else if p >= 0.0 {
        (p / C).sqrt() + R0
    } else {
        R0
    }
}

/// Rec.2020 linear to Rec.709 / sRGB linear.
const TO_709: [[f32; 3]; 3] = [
    [1.660_491, -0.587_641, -0.072_850],
    [-0.124_550, 1.132_900, -0.008_349],
    [-0.018_151, -0.100_579, 1.118_730],
];

/// Brightest scene light kept before it turns white (about 4.5 stops above
/// middle grey).
const WHITE: f32 = 4.0;

/// Highlight roll-off that keeps middle grey where it is.
fn tone(x: f32) -> f32 {
    let x = x.max(0.0);
    let w2 = WHITE * WHITE;
    let curve = |v: f32| v * (1.0 + v / w2) / (1.0 + v);
    let k = 0.18 / curve(0.18);
    (k * curve(x)).min(1.0)
}

fn srgb_encode(v: f32) -> f32 {
    let v = v.clamp(0.0, 1.0);
    if v <= 0.003_130_8 { 12.92 * v } else { 1.055 * v.powf(1.0 / 2.4) - 0.055 }
}

const STEPS: usize = 4096;

/// Convert an Apple Log frame (as decoded to 8-bit RGB) to display colours.
pub fn to_display(img: &mut RgbaImage) {
    let lin: Vec<f32> = (0..256).map(|i| decode(i as f32 / 255.0)).collect();
    // Display value for scene light 0..WHITE.
    let out: Vec<u8> = (0..=STEPS)
        .map(|i| {
            let x = i as f32 / STEPS as f32 * WHITE;
            (srgb_encode(tone(x)) * 255.0).round() as u8
        })
        .collect();
    for px in img.pixels_mut() {
        let r = lin[px[0] as usize];
        let g = lin[px[1] as usize];
        let b = lin[px[2] as usize];
        for (c, row) in TO_709.iter().enumerate() {
            let v = row[0] * r + row[1] * g + row[2] * b;
            let i = ((v / WHITE).clamp(0.0, 1.0) * STEPS as f32).round() as usize;
            px[c] = out[i];
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn middle_grey_decodes_to_eighteen_percent() {
        // Apple Log puts 18% grey at a signal of about 0.488.
        let p = GAMMA * (0.18f32 + B).log2() + BETA;
        assert!((p - 0.488).abs() < 0.005, "{p}");
        assert!((decode(p) - 0.18).abs() < 1e-3);
    }

    #[test]
    fn curve_is_continuous_at_the_join() {
        let pt = C * (RT - R0) * (RT - R0);
        assert!((decode(pt - 1e-5) - decode(pt + 1e-5)).abs() < 1e-3);
        assert!((decode(pt) - RT).abs() < 1e-3);
    }

    #[test]
    fn grey_stays_neutral_and_gains_contrast() {
        let grey = (0.488f32 * 255.0).round() as u8;
        let mut img = RgbaImage::from_pixel(2, 1, image::Rgba([grey, grey, grey, 255]));
        img.put_pixel(1, 0, image::Rgba([60, 60, 60, 255]));
        to_display(&mut img);
        let g = img.get_pixel(0, 0);
        assert_eq!(g[0], g[1]);
        assert_eq!(g[1], g[2]);
        // sRGB middle grey is about 118.
        assert!((g[0] as i32 - 118).abs() <= 4, "{}", g[0]);
        // A log value of 60 is about 3.6 stops under middle grey: dark on screen.
        let d = img.get_pixel(1, 0)[0];
        assert!((25..40).contains(&d), "{d}");
    }
}
