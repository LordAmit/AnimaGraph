//! Finishing steps applied to a loop after it is made: color look and
//! bounce (forward then backward).

use image::RgbaImage;

use crate::render::par_chunks;

/// Color and tone adjustments. `Look::default()` means "no change".
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Look {
    /// Brightness in stops, -1..1.
    pub exposure: f32,
    pub contrast: f32,
    /// S-curve strength: positive deepens shadows and brightens highlights
    /// around the pivot, negative flattens.
    pub s_curve: f32,
    /// Tone (0..1) the S-curve turns around.
    pub pivot: f32,
    /// Lift (positive) or deepen (negative) the dark tones.
    pub shadows: f32,
    /// Brighten (positive) or recover (negative) the bright tones.
    pub highlights: f32,
    pub saturation: f32,
    /// Negative is cooler (bluer), positive warmer.
    pub warmth: f32,
    /// Lifts blacks and softens colors, 0..1.
    pub fade: f32,
    /// Darkens the corners, 0..1.
    pub vignette: f32,
    /// Film grain strength 0..1 and grain size in pixels.
    pub grain: f32,
    pub grain_size: f32,
}

pub const DEFAULT_PIVOT: f32 = 0.5;
pub const DEFAULT_GRAIN_SIZE: f32 = 1.5;

impl Default for Look {
    fn default() -> Self {
        Self {
            exposure: 0.0,
            contrast: 0.0,
            s_curve: 0.0,
            pivot: DEFAULT_PIVOT,
            shadows: 0.0,
            highlights: 0.0,
            saturation: 0.0,
            warmth: 0.0,
            fade: 0.0,
            vignette: 0.0,
            grain: 0.0,
            grain_size: DEFAULT_GRAIN_SIZE,
        }
    }
}

impl Look {
    pub fn is_neutral(&self) -> bool {
        let n = Look { grain_size: self.grain_size, ..Look::default() };
        *self == n
    }

    fn tone_is_neutral(&self) -> bool {
        Look { grain: 0.0, grain_size: DEFAULT_GRAIN_SIZE, ..*self } == Look::default()
    }

    pub fn presets() -> [(&'static str, Look); 8] {
        let n = Look::default();
        [
            ("Natural", n),
            ("Warm", Look { warmth: 0.5, saturation: 0.1, contrast: 0.05, ..n }),
            ("Cool", Look { warmth: -0.45, contrast: 0.05, ..n }),
            ("Vivid", Look { saturation: 0.35, s_curve: 0.35, vignette: 0.2, ..n }),
            ("Faded", Look { fade: 0.6, contrast: -0.15, saturation: -0.2, ..n }),
            ("Dusk", Look { warmth: 0.35, exposure: -0.15, s_curve: 0.25, vignette: 0.35, ..n }),
            ("Film", Look { s_curve: 0.3, fade: 0.25, warmth: 0.15, grain: 0.35, ..n }),
            ("Mono", Look { saturation: -1.0, s_curve: 0.3, vignette: 0.25, grain: 0.2, ..n }),
        ]
    }
}

/// One channel value (0..1) through exposure, contrast, shadows,
/// highlights and the S-curve.
fn tone(v: f32, look: &Look, gain: f32) -> f32 {
    let mut v = v * gain;
    v = ((v - 0.5) * (1.0 + look.contrast) + 0.5).clamp(0.0, 1.0);
    v += look.shadows * 0.6 * v * (1.0 - v) * (1.0 - v);
    v += look.highlights * 0.6 * v * v * (1.0 - v);
    v = v.clamp(0.0, 1.0);
    if look.s_curve != 0.0 {
        let p = look.pivot.clamp(0.05, 0.95);
        let e = if look.s_curve >= 0.0 { 1.0 + 2.0 * look.s_curve } else { 1.0 / (1.0 - 2.0 * look.s_curve) };
        v = if v < p { p * (v / p).powf(e) } else { 1.0 - (1.0 - p) * ((1.0 - v) / (1.0 - p)).powf(e) };
    }
    v
}

/// Color and tone, without grain.
pub fn apply_tone(img: &mut RgbaImage, look: &Look) {
    if look.tone_is_neutral() {
        return;
    }
    let (w, h) = (img.width() as usize, img.height() as usize);
    let gain = 2f32.powf(look.exposure);
    let warm = [1.0 + 0.12 * look.warmth, 1.0, 1.0 - 0.12 * look.warmth];
    // Per-channel tone table: every 8-bit value mapped once.
    let mut lut = [[0.0f32; 256]; 3];
    for (k, table) in lut.iter_mut().enumerate() {
        for (i, t) in table.iter_mut().enumerate() {
            *t = tone(i as f32 / 255.0 * warm[k], look, gain);
        }
    }
    let sat = 1.0 + look.saturation;
    let (cx, cy) = (w as f32 / 2.0, h as f32 / 2.0);
    let rmax2 = cx * cx + cy * cy;
    par_chunks(img.as_mut(), w * 4, |y, row| {
        let dy = y as f32 - cy;
        for x in 0..w {
            let px = &mut row[x * 4..x * 4 + 3];
            let c = [lut[0][px[0] as usize], lut[1][px[1] as usize], lut[2][px[2] as usize]];
            let l = 0.299 * c[0] + 0.587 * c[1] + 0.114 * c[2];
            let mut vig = 1.0;
            if look.vignette > 0.0 {
                let dx = x as f32 - cx;
                let r = ((dx * dx + dy * dy) / rmax2).sqrt();
                let t = ((r - 0.45) / 0.55).clamp(0.0, 1.0);
                vig = 1.0 - look.vignette * 0.65 * t * t * (3.0 - 2.0 * t);
            }
            for k in 0..3 {
                let mut v = l + (c[k] - l) * sat;
                v = v * (1.0 - 0.22 * look.fade) + 0.1 * look.fade;
                px[k] = (v * vig * 255.0).round().clamp(0.0, 255.0) as u8;
            }
        }
    });
}

fn hash3(x: u32, y: u32, seed: u32) -> f32 {
    let mut n = x.wrapping_mul(0x8da6_b343) ^ y.wrapping_mul(0xd816_3841) ^ seed.wrapping_mul(0xcb1a_b31f);
    n = (n ^ (n >> 13)).wrapping_mul(0x5bd1_e995);
    n ^= n >> 15;
    (n & 0xffff) as f32 / 65535.0
}

/// Film grain. `seed` changes the pattern, so each frame of a loop gets its own.
pub fn apply_grain(img: &mut RgbaImage, amount: f32, size: f32, seed: u32) {
    if amount <= 0.0 {
        return;
    }
    let w = img.width() as usize;
    let cell = size.max(1.0);
    let strength = amount * 0.2 * 255.0;
    par_chunks(img.as_mut(), w * 4, |y, row| {
        let gy = (y as f32 / cell) as u32;
        for x in 0..w {
            let gx = (x as f32 / cell) as u32;
            // Sum of two uniform values: softer, film-like distribution.
            let n = hash3(gx, gy, seed) + hash3(gx, gy, seed ^ 0x9e37_79b9) - 1.0;
            let px = &mut row[x * 4..x * 4 + 3];
            let l = (0.299 * px[0] as f32 + 0.587 * px[1] as f32 + 0.114 * px[2] as f32) / 255.0;
            // Grain shows most in the mid tones.
            let weight = 1.0 - 0.6 * (2.0 * l - 1.0).powi(2);
            let d = n * strength * weight;
            for v in px.iter_mut() {
                *v = (*v as f32 + d).round().clamp(0.0, 255.0) as u8;
            }
        }
    });
}

/// Tone and grain for a single picture.
pub fn apply_look(img: &mut RgbaImage, look: &Look) {
    apply_tone(img, look);
    apply_grain(img, look.grain, look.grain_size, 0);
}

/// Forward then backward, without repeating the end frames.
pub fn bounce(frames: Vec<RgbaImage>) -> Vec<RgbaImage> {
    let n = frames.len();
    if n < 3 {
        return frames;
    }
    let back: Vec<RgbaImage> = frames[1..n - 1].iter().rev().cloned().collect();
    let mut out = frames;
    out.extend(back);
    out
}

/// Look, bounce and grain, in the order the preview and the export use.
/// Grain comes last so every played frame has its own pattern.
pub fn finish(mut frames: Vec<RgbaImage>, look: &Look, bounce_loop: bool) -> Vec<RgbaImage> {
    for f in frames.iter_mut() {
        apply_tone(f, look);
    }
    let mut frames = if bounce_loop { bounce(frames) } else { frames };
    for (i, f) in frames.iter_mut().enumerate() {
        apply_grain(f, look.grain, look.grain_size, i as u32 + 1);
    }
    frames
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bounce_plays_back_without_repeating_ends() {
        let frames: Vec<RgbaImage> = (0..4u8).map(|i| RgbaImage::from_pixel(2, 2, image::Rgba([i, 0, 0, 255]))).collect();
        let out = bounce(frames);
        let order: Vec<u8> = out.iter().map(|f| f.get_pixel(0, 0)[0]).collect();
        assert_eq!(order, vec![0, 1, 2, 3, 2, 1]);
    }

    #[test]
    fn s_curve_keeps_pivot_and_spreads_tones() {
        let look = Look { s_curve: 0.6, pivot: 0.5, ..Look::default() };
        assert!((tone(0.5, &look, 1.0) - 0.5).abs() < 1e-5);
        assert!(tone(0.25, &look, 1.0) < 0.25);
        assert!(tone(0.75, &look, 1.0) > 0.75);
        let low = Look { pivot: 0.3, ..look };
        assert!((tone(0.3, &low, 1.0) - 0.3).abs() < 1e-5);
        // Shadows lift dark tones and leave black alone.
        let sh = Look { shadows: 1.0, ..Look::default() };
        assert!(tone(0.2, &sh, 1.0) > 0.2 && tone(0.0, &sh, 1.0) == 0.0);
        let hi = Look { highlights: -1.0, ..Look::default() };
        assert!(tone(0.8, &hi, 1.0) < 0.8 && tone(1.0, &hi, 1.0) == 1.0);
    }

    #[test]
    fn grain_differs_per_frame_and_zero_does_nothing() {
        let base = RgbaImage::from_pixel(16, 16, image::Rgba([128, 128, 128, 255]));
        let mut a = base.clone();
        let mut b = base.clone();
        apply_grain(&mut a, 0.5, 1.0, 1);
        apply_grain(&mut b, 0.5, 1.0, 2);
        assert_ne!(a, base);
        assert_ne!(a, b);
        let mut c = base.clone();
        apply_grain(&mut c, 0.0, 1.0, 1);
        assert_eq!(c, base);
    }

    #[test]
    fn looks_change_color_and_neutral_does_not() {
        let base = RgbaImage::from_pixel(8, 8, image::Rgba([120, 140, 160, 255]));
        let mut same = base.clone();
        apply_look(&mut same, &Look::default());
        assert_eq!(same, base);
        let mut mono = base.clone();
        apply_tone(&mut mono, &Look { grain: 0.0, ..Look::presets()[7].1 });
        let p = mono.get_pixel(4, 4);
        assert!(p[0] == p[1] && p[1] == p[2]);
        let mut warm = base.clone();
        apply_look(&mut warm, &Look::presets()[1].1);
        assert!(warm.get_pixel(4, 4)[0] > base.get_pixel(4, 4)[0]);
    }
}
