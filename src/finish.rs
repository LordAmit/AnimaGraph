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
    /// Darkens (positive) or lightens (negative) the edges, -1..1.
    pub vignette: f32,
    /// Where the vignette starts (0 near the center, 1 near the corners),
    /// how soft its edge is (0..1) and its outline (-1 rectangle,
    /// 0 an oval that follows the picture, 1 a circle).
    pub vignette_mid: f32,
    pub vignette_feather: f32,
    pub vignette_round: f32,
    /// Film grain strength 0..1, grain size in pixels and roughness 0..1
    /// (0 even, 1 clumped).
    pub grain: f32,
    pub grain_size: f32,
    pub grain_rough: f32,
    /// Split toning: color added to the dark and to the bright tones
    /// (per channel, about -0.1..0.1).
    pub tint_shadows: [f32; 3],
    pub tint_highlights: [f32; 3],
    /// Turns greens toward teal (0..1), for wet-foliage looks.
    pub green_shift: f32,
    /// Extra S-curve on the green channel only (-1..1).
    pub green_curve: f32,

    // Raw-developer style controls (each -1..1 unless noted).
    /// Brightest tones (white point) and darkest tones (black point).
    pub whites: f32,
    pub blacks: f32,
    /// Fine-detail contrast (texture), local contrast (clarity) and haze
    /// removal.
    pub texture: f32,
    pub clarity: f32,
    pub dehaze: f32,
    /// Saturation that favors muted colors.
    pub vibrance: f32,
    /// Edge sharpening, 0..1.
    pub sharpen: f32,
    /// White-balance tint: negative green, positive magenta.
    pub tint: f32,
    /// Point tone curves: all channels, then red, green, blue.
    pub curves: [crate::develop::Curve; 4],
    /// Hue, saturation, luminance per color band (see develop::HSL_NAMES).
    pub hsl: [[f32; 3]; 8],
    /// Color grading: (hue in degrees, saturation 0..1, luminance -1..1)
    /// for shadows, midtones, highlights and the whole picture (global),
    /// the balance between shadows and highlights (-1..1) and how much
    /// the ranges overlap (blending, 0..1).
    pub grade: [[f32; 3]; 4],
    pub grade_balance: f32,
    pub grade_blend: f32,
}

/// Blending used when a look does not set it (the middle of the slider).
pub const DEFAULT_BLEND: f32 = 0.5;
/// Vignette and grain settings used when a look does not set them.
pub const DEFAULT_VIGNETTE_MID: f32 = 0.5;
pub const DEFAULT_VIGNETTE_FEATHER: f32 = 0.5;
pub const DEFAULT_GRAIN_ROUGH: f32 = 0.5;

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
            vignette_mid: DEFAULT_VIGNETTE_MID,
            vignette_feather: DEFAULT_VIGNETTE_FEATHER,
            vignette_round: 0.0,
            grain: 0.0,
            grain_size: DEFAULT_GRAIN_SIZE,
            grain_rough: DEFAULT_GRAIN_ROUGH,
            tint_shadows: [0.0; 3],
            tint_highlights: [0.0; 3],
            green_shift: 0.0,
            green_curve: 0.0,
            whites: 0.0,
            blacks: 0.0,
            texture: 0.0,
            clarity: 0.0,
            sharpen: 0.0,
            dehaze: 0.0,
            vibrance: 0.0,
            tint: 0.0,
            curves: [crate::develop::Curve::default(); 4],
            hsl: [[0.0; 3]; 8],
            grade: [[0.0; 3]; 4],
            grade_balance: 0.0,
            grade_blend: DEFAULT_BLEND,
        }
    }
}

impl Look {
    pub fn is_neutral(&self) -> bool {
        let n = Look { grain_size: self.grain_size, grain_rough: self.grain_rough, ..Look::default() };
        self.without_unused_vignette() == n
    }

    fn tone_is_neutral(&self) -> bool {
        let no_grain = Look { grain: 0.0, grain_size: DEFAULT_GRAIN_SIZE, grain_rough: DEFAULT_GRAIN_ROUGH, ..*self };
        no_grain.without_unused_vignette() == Look::default()
    }

    /// The vignette outline settings do nothing while the vignette is off.
    fn without_unused_vignette(&self) -> Look {
        if self.vignette != 0.0 {
            return *self;
        }
        let n = Look::default();
        Look { vignette_mid: n.vignette_mid, vignette_feather: n.vignette_feather, vignette_round: n.vignette_round, ..*self }
    }

    /// This look blended with no change: 0 = untouched, 1 = full look.
    pub fn scaled(&self, t: f32) -> Look {
        let n = Look::default();
        let t = t.clamp(0.0, 1.0);
        let l = |a: f32, b: f32| a + (b - a) * t;
        let l3 = |a: [f32; 3], b: [f32; 3]| [l(a[0], b[0]), l(a[1], b[1]), l(a[2], b[2])];
        let mut hsl = [[0.0f32; 3]; 8];
        for i in 0..8 {
            hsl[i] = l3(n.hsl[i], self.hsl[i]);
        }
        let mut grade = self.grade;
        for g in grade.iter_mut() {
            g[1] *= t; // keep the hue, scale the strength
            g[2] *= t;
        }
        Look {
            exposure: l(n.exposure, self.exposure),
            contrast: l(n.contrast, self.contrast),
            s_curve: l(n.s_curve, self.s_curve),
            pivot: l(n.pivot, self.pivot),
            shadows: l(n.shadows, self.shadows),
            highlights: l(n.highlights, self.highlights),
            saturation: l(n.saturation, self.saturation),
            warmth: l(n.warmth, self.warmth),
            fade: l(n.fade, self.fade),
            vignette: l(n.vignette, self.vignette),
            vignette_mid: self.vignette_mid,
            vignette_feather: self.vignette_feather,
            vignette_round: self.vignette_round,
            grain: l(n.grain, self.grain),
            grain_size: self.grain_size,
            grain_rough: self.grain_rough,
            tint_shadows: l3(n.tint_shadows, self.tint_shadows),
            tint_highlights: l3(n.tint_highlights, self.tint_highlights),
            green_shift: l(n.green_shift, self.green_shift),
            green_curve: l(n.green_curve, self.green_curve),
            whites: l(n.whites, self.whites),
            blacks: l(n.blacks, self.blacks),
            texture: l(n.texture, self.texture),
            clarity: l(n.clarity, self.clarity),
            sharpen: l(n.sharpen, self.sharpen),
            dehaze: l(n.dehaze, self.dehaze),
            vibrance: l(n.vibrance, self.vibrance),
            tint: l(n.tint, self.tint),
            curves: [self.curves[0].scaled(t), self.curves[1].scaled(t), self.curves[2].scaled(t), self.curves[3].scaled(t)],
            hsl,
            grade,
            grade_balance: l(n.grade_balance, self.grade_balance),
            grade_blend: l(n.grade_blend, self.grade_blend),
        }
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

/// Value gain for an exposure change in stops. One stop doubles the light;
/// in the gamma-encoded values the app works with that is about 2^(1/2.2).
fn exposure_gain(stops: f32) -> f32 {
    2f32.powf(stops / 2.2)
}

/// Brightness factor from Shadows and Highlights for a pixel whose
/// neighborhood has brightness `base` (0..1). It follows the neighborhood
/// rather than the pixel itself, so fine detail keeps its contrast.
fn shadow_highlight_gain(look: &Look, base: f32) -> f32 {
    let b = base.clamp(0.0, 1.0);
    let ws = (1.0 - b).powi(3);
    let s = if look.shadows >= 0.0 { look.shadows } else { look.shadows * 0.5 };
    let h = if look.highlights >= 0.0 {
        look.highlights * 0.25 * b.powi(3)
    } else {
        // Recovering highlights reaches well into the upper middle tones,
        // as in Lightroom: measured against two photos edited there, -30
        // darkens tones from about a third of full brightness upward by
        // about an eighth. Stronger settings add less and less.
        let smooth = |a: f32, z: f32| {
            let t = ((b - a) / (z - a)).clamp(0.0, 1.0);
            t * t * (3.0 - 2.0 * t)
        };
        -0.22 * (-look.highlights).sqrt() * smooth(0.12, 0.4) * (1.0 - 0.45 * smooth(0.55, 0.95))
    };
    (1.0 + s * ws) * (1.0 + h)
}

/// One channel value (0..1) through contrast, the black and white points,
/// dehaze and the S-curve. Exposure, white balance, shadows and highlights
/// come before this.
fn tone(v: f32, look: &Look) -> f32 {
    let mut v = v.clamp(0.0, 1.0);
    // Contrast steepens the middle tones and keeps black and white in place.
    if look.contrast != 0.0 {
        let c = (look.contrast * 1.5).clamp(-0.9, 0.9);
        v -= c * (std::f32::consts::TAU * v).sin() / std::f32::consts::TAU;
    }
    // Black and white points.
    v += look.blacks * 0.15 * (1.0 - v).powi(3);
    v += look.whites * 0.15 * v.powi(3);
    // Dehaze: pull the black point down (or lift it for negative values).
    if look.dehaze != 0.0 {
        let d = look.dehaze * 0.12;
        v = if d > 0.0 { (v - d) / (1.0 - d) } else { v * (1.0 + d) - d };
    }
    v = v.clamp(0.0, 1.0);
    if look.s_curve != 0.0 {
        let p = look.pivot.clamp(0.05, 0.95);
        let e = if look.s_curve >= 0.0 { 1.0 + 2.0 * look.s_curve } else { 1.0 / (1.0 - 2.0 * look.s_curve) };
        v = if v < p { p * (v / p).powf(e) } else { 1.0 - (1.0 - p) * ((1.0 - v) / (1.0 - p)).powf(e) };
    }
    v
}

/// S-curve of strength `s` (-1..1) around `pivot`.
fn s_curve(v: f32, s: f32, pivot: f32) -> f32 {
    let p = pivot.clamp(0.05, 0.95);
    let e = if s >= 0.0 { 1.0 + 2.0 * s } else { 1.0 / (1.0 - 2.0 * s) };
    if v < p { p * (v / p).powf(e) } else { 1.0 - (1.0 - p) * ((1.0 - v) / (1.0 - p)).powf(e) }
}

/// Color and tone, without grain.
pub fn apply_tone(img: &mut RgbaImage, look: &Look) {
    if look.tone_is_neutral() {
        return;
    }
    let (w, h) = (img.width() as usize, img.height() as usize);
    let gain = exposure_gain(look.exposure);
    // White balance: warmth (blue-yellow) and tint (green-magenta).
    let warm = [
        (1.0 + 0.12 * look.warmth) * (1.0 + 0.04 * look.tint),
        1.0 - 0.1 * look.tint,
        (1.0 - 0.12 * look.warmth) * (1.0 + 0.04 * look.tint),
    ];
    // First stage, per channel: white balance and exposure (not clipped
    // yet, so Highlights can bring back brightened tones).
    let mut pre = [[0.0f32; 256]; 3];
    for (k, table) in pre.iter_mut().enumerate() {
        for (i, t) in table.iter_mut().enumerate() {
            *t = i as f32 / 255.0 * warm[k] * gain;
        }
    }
    // Second stage, per channel: the tone controls, the all-channel curve
    // and that channel's own curve, sampled finely and interpolated.
    const STEPS: usize = 1024;
    let master = &look.curves[0];
    let mut post = vec![[0.0f32; 3]; STEPS + 1];
    for (i, row) in post.iter_mut().enumerate() {
        for (k, t) in row.iter_mut().enumerate() {
            let own = &look.curves[k + 1];
            let mut v = tone(i as f32 / STEPS as f32, look);
            if !master.is_identity() {
                v = master.eval(v);
            }
            if !own.is_identity() {
                v = own.eval(v);
            }
            *t = v;
        }
    }
    let lut = |k: usize, v: f32| {
        let x = v.clamp(0.0, 1.0) * STEPS as f32;
        let i = (x as usize).min(STEPS - 1);
        let f = x - i as f32;
        post[i][k] * (1.0 - f) + post[i + 1][k] * f
    };
    // Shadows and Highlights work on each pixel's neighborhood brightness.
    let base: Option<Vec<f32>> = (look.shadows != 0.0 || look.highlights != 0.0).then(|| {
        let lum: Vec<f32> = img
            .pixels()
            .map(|p| (0.299 * p[0] as f32 + 0.587 * p[1] as f32 + 0.114 * p[2] as f32) / 255.0 * gain)
            .collect();
        let r = ((w.min(h) / 30).max(2)) as u32;
        crate::scene::blur_mask(&crate::scene::blur_mask(&lum, w as u32, h as u32, r), w as u32, h as u32, r)
    });
    let sat = 1.0 + look.saturation + 0.25 * look.dehaze;
    let use_hsl = look.hsl.iter().any(|b| b.iter().any(|v| *v != 0.0));
    let use_grade = look.grade.iter().any(|g| g[1] > 0.0 || g[2] != 0.0);
    // Detail: the difference between each pixel's brightness and its
    // blurred neighborhood, added back. Clarity uses a wide neighborhood,
    // texture a small one and sharpening the nearest pixels.
    let local = detail(img, look);
    let (cx, cy) = (w as f32 / 2.0, h as f32 / 2.0);
    let rmax2 = cx * cx + cy * cy;
    par_chunks(img.as_mut(), w * 4, |y, row| {
        let dy = y as f32 - cy;
        for x in 0..w {
            let px = &mut row[x * 4..x * 4 + 3];
            let mut c = [pre[0][px[0] as usize], pre[1][px[1] as usize], pre[2][px[2] as usize]];
            if let Some(base) = &base {
                // Mostly the pixel itself, partly its neighborhood: dark detail
                // beside a bright area is not pulled down with it.
                let own = 0.299 * c[0] + 0.587 * c[1] + 0.114 * c[2];
                let g = shadow_highlight_gain(look, 0.25 * base[y * w + x] + 0.75 * own);
                // Recovering highlights also brings near-white down a little,
                // as Lightroom does (about 250 to 233 at -30).
                for v in c.iter_mut() {
                    *v *= g;
                }
                if g < 1.0 {
                    // Lightroom darkens each color channel on its own, so
                    // colors deepen as they darken.
                    let l = 0.299 * c[0] + 0.587 * c[1] + 0.114 * c[2];
                    let k = 1.0 + 2.0 * (1.0 - g);
                    for v in c.iter_mut() {
                        *v = l + (*v - l) * k;
                    }
                }
            }
            let mut c = [lut(0, c[0]), lut(1, c[1]), lut(2, c[2])];
            if let Some(local) = &local {
                let d = local[y * w + x];
                for v in c.iter_mut() {
                    *v += d;
                }
            }
            if look.green_shift > 0.0 || look.green_curve != 0.0 {
                // How green the pixel is: green above both red and blue.
                // Only green areas are changed; other colors stay as they are.
                let greenness = ((c[1] - c[0].max(c[2])) / 0.12).clamp(0.0, 1.0);
                if greenness > 0.0 {
                    if look.green_curve != 0.0 {
                        let curved = s_curve(c[1].clamp(0.0, 1.0), look.green_curve, 0.5);
                        c[1] += (curved - c[1]) * greenness;
                    }
                    let w = greenness * look.green_shift;
                    c[2] += (c[1] - c[2]) * 0.9 * w;
                    c[0] *= 1.0 - 0.3 * w;
                    c[1] *= 1.0 - 0.12 * w;
                }
            }
            if use_hsl {
                c = crate::develop::apply_hsl(c, &look.hsl);
            }
            let l = 0.299 * c[0] + 0.587 * c[1] + 0.114 * c[2];
            // Vibrance: boosts muted colors more than strong ones.
            let mut px_sat = sat;
            if look.vibrance != 0.0 {
                let max = c[0].max(c[1]).max(c[2]);
                let min = c[0].min(c[1]).min(c[2]);
                let s_now = if max > 0.0 { (max - min) / max } else { 0.0 };
                // Raising vibrance holds back on skin tones (orange hues).
                let skin = if look.vibrance > 0.0 && s_now > 0.0 {
                    let hue = crate::develop::rgb_to_hsl([c[0].clamp(0.0, 1.0), c[1].clamp(0.0, 1.0), c[2].clamp(0.0, 1.0)])[0];
                    (1.0 - ((hue - 25.0 + 180.0).rem_euclid(360.0) - 180.0).abs() / 25.0).max(0.0)
                } else {
                    0.0
                };
                px_sat += look.vibrance * (1.0 - s_now) * (1.0 - 0.5 * skin);
            }
            let grade = if use_grade {
                crate::develop::grade_offset(l, &look.grade, look.grade_balance, look.grade_blend)
            } else {
                [0.0; 3]
            };
            let (mut vig, mut lift) = (1.0, 0.0);
            if look.vignette != 0.0 {
                let t = vignette_weight(look, x as f32 - cx, dy, cx, cy, rmax2);
                if look.vignette > 0.0 {
                    vig = 1.0 - look.vignette * 0.65 * t;
                } else {
                    lift = -look.vignette * 0.65 * t;
                }
            }
            for k in 0..3 {
                let mut v = l + (c[k] - l) * px_sat.max(0.0);
                v += grade[k];
                v = v * (1.0 - 0.22 * look.fade) + 0.1 * look.fade;
                // Split toning, strongest in the deep shadows / bright highlights.
                v += look.tint_shadows[k] * (1.0 - l).powi(2) + look.tint_highlights[k] * l * l;
                v = v * vig;
                v += (1.0 - v) * lift;
                px[k] = (v * 255.0).round().clamp(0.0, 255.0) as u8;
            }
        }
    });
}

/// Per-pixel brightness change from clarity, texture and sharpening, or
/// `None` when all three are off.
fn detail(img: &RgbaImage, look: &Look) -> Option<Vec<f32>> {
    if look.clarity == 0.0 && look.texture == 0.0 && look.sharpen <= 0.0 {
        return None;
    }
    let (w, h) = (img.width(), img.height());
    let lum: Vec<f32> = img
        .pixels()
        .map(|p| (0.299 * p[0] as f32 + 0.587 * p[1] as f32 + 0.114 * p[2] as f32) / 255.0)
        .collect();
    let blur2 = |r: u32| crate::scene::blur_mask(&crate::scene::blur_mask(&lum, w, h, r), w, h, r);
    let mut out = vec![0.0f32; lum.len()];
    if look.clarity != 0.0 {
        // Clarity works mostly on the middle tones and spares the deep
        // shadows and bright highlights.
        let blur = blur2((w.min(h) / 80).max(2));
        for ((o, a), b) in out.iter_mut().zip(lum.iter()).zip(blur.iter()) {
            let mid = 1.0 - 0.8 * (2.0 * a - 1.0).powi(2);
            *o += (a - b) * look.clarity * 0.8 * mid;
        }
    }
    let mut add = |blur: &[f32], k: f32| {
        for ((o, a), b) in out.iter_mut().zip(lum.iter()).zip(blur.iter()) {
            *o += (a - b) * k;
        }
    };
    if look.texture != 0.0 {
        add(&blur2((w.min(h) / 250).max(1)), look.texture * 0.9);
    }
    if look.sharpen > 0.0 {
        // Sharpening works on the finest detail of the full-size photo. A
        // smaller copy (the on-screen preview) has no such fine detail, so
        // it gets less, and looks as the full-size result does at that size.
        add(&crate::scene::blur_mask(&lum, w, h, 1), look.sharpen * 1.5 * detail_scale(w, h).min(1.0));
    }
    Some(out)
}

/// Short side of the photo that grain size and sharpening are measured
/// against (a typical phone or camera photo).
const DETAIL_REFERENCE: f32 = 3000.0;

/// How big this picture is compared with a typical full-size photo.
fn detail_scale(w: u32, h: u32) -> f32 {
    w.min(h).max(1) as f32 / DETAIL_REFERENCE
}

/// How strongly the vignette applies at offset (dx, dy) from the center,
/// 0 inside the clear area and 1 at full strength.
fn vignette_weight(look: &Look, dx: f32, dy: f32, cx: f32, cy: f32, rmax2: f32) -> f32 {
    // Distance that reaches 1 at the corners: an oval that follows the
    // picture, a circle, or a rounded rectangle.
    let (u, v) = ((dx / cx.max(1.0)).abs(), (dy / cy.max(1.0)).abs());
    let oval = ((u * u + v * v) / 2.0).sqrt();
    let round = look.vignette_round.clamp(-1.0, 1.0);
    let r = if round >= 0.0 {
        let circle = ((dx * dx + dy * dy) / rmax2).sqrt();
        oval + (circle - oval) * round
    } else {
        let p = 2.0 + 6.0 * -round;
        ((u.powf(p) + v.powf(p)) / 2.0).powf(1.0 / p)
    };
    let start = 0.15 + 0.6 * look.vignette_mid.clamp(0.0, 1.0);
    let width = 0.05 + look.vignette_feather.clamp(0.0, 1.0);
    let t = ((r - start) / width).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn hash3(x: u32, y: u32, seed: u32) -> f32 {
    let mut n = x.wrapping_mul(0x8da6_b343) ^ y.wrapping_mul(0xd816_3841) ^ seed.wrapping_mul(0xcb1a_b31f);
    n = (n ^ (n >> 13)).wrapping_mul(0x5bd1_e995);
    n ^= n >> 15;
    (n & 0xffff) as f32 / 65535.0
}

/// Film grain. `seed` changes the pattern, so each frame of a loop gets its own.
/// `rough` (0..1) mixes in coarser clumps of grain. `size` is in pixels of a
/// typical full-size photo, so the grain looks the same in the small
/// on-screen preview as in the full-size picture.
pub fn apply_grain(img: &mut RgbaImage, amount: f32, size: f32, rough: f32, seed: u32) {
    if amount <= 0.0 {
        return;
    }
    let w = img.width() as usize;
    let scale = detail_scale(img.width(), img.height());
    // A grain speck smaller than one pixel here: one pixel holds several
    // specks, which average out to a fainter grain.
    let fine_size = size.max(1.0) * scale;
    let coarse_size = fine_size * 3.0;
    let (cell, fine_amp) = (fine_size.max(1.0), fine_size.min(1.0));
    let (coarse_cell, coarse_amp) = (coarse_size.max(1.0), coarse_size.min(1.0));
    let a = 0.6 * rough.clamp(0.0, 1.0);
    // Keep the overall strength the same at any roughness.
    let norm = 1.0 / ((1.0 - a) * (1.0 - a) + a * a).sqrt();
    let strength = amount * 0.2 * 255.0;
    par_chunks(img.as_mut(), w * 4, |y, row| {
        let gy = (y as f32 / cell) as u32;
        let cy = (y as f32 / coarse_cell) as u32;
        for x in 0..w {
            let gx = (x as f32 / cell) as u32;
            let cx = (x as f32 / coarse_cell) as u32;
            // Sum of two uniform values: softer, film-like distribution.
            let fine = (hash3(gx, gy, seed) + hash3(gx, gy, seed ^ 0x9e37_79b9) - 1.0) * fine_amp;
            let n = if a > 0.0 {
                let coarse =
                    (hash3(cx, cy, seed ^ 0x51ed_270b) + hash3(cx, cy, seed ^ 0x2545_f491) - 1.0) * coarse_amp;
                (fine * (1.0 - a) + coarse * a) * norm
            } else {
                fine
            };
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
    apply_grain(img, look.grain, look.grain_size, look.grain_rough, 0);
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
        apply_grain(f, look.grain, look.grain_size, look.grain_rough, i as u32 + 1);
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
        assert!((tone(0.5, &look) - 0.5).abs() < 1e-5);
        assert!(tone(0.25, &look) < 0.25);
        assert!(tone(0.75, &look) > 0.75);
        let low = Look { pivot: 0.3, ..look };
        assert!((tone(0.3, &low) - 0.3).abs() < 1e-5);
    }

    fn gray_after(v: u8, look: &Look) -> u8 {
        let mut img = RgbaImage::from_pixel(8, 8, image::Rgba([v, v, v, 255]));
        apply_tone(&mut img, look);
        img.get_pixel(4, 4)[0]
    }

    #[test]
    fn contrast_keeps_black_and_white() {
        for c in [-0.5, 0.5] {
            let look = Look { contrast: c, ..Look::default() };
            assert_eq!(gray_after(0, &look), 0);
            assert_eq!(gray_after(255, &look), 255);
            assert_eq!(gray_after(128, &look), 128);
        }
        let more = Look { contrast: 0.5, ..Look::default() };
        assert!(gray_after(64, &more) < 64 && gray_after(192, &more) > 192);
        assert!(gray_after(5, &more) > 0, "no clipping to black");
    }

    #[test]
    fn exposure_is_in_stops_of_light() {
        // +1 stop doubles the linear light: sRGB 0.3 becomes about 0.41.
        let up = gray_after(77, &Look { exposure: 1.0, ..Look::default() });
        assert!((100..=110).contains(&up), "{up}");
        let down = gray_after(77, &Look { exposure: -1.0, ..Look::default() });
        assert!((52..=60).contains(&down), "{down}");
    }

    #[test]
    fn shadows_and_highlights_work_by_region() {
        let sh = Look { shadows: 1.0, ..Look::default() };
        assert!(gray_after(50, &sh) > 70, "shadows lift dark areas");
        assert_eq!(gray_after(0, &sh), 0, "black stays black");
        assert!(gray_after(220, &sh) < 225, "bright areas barely move");
        let hi = Look { highlights: -1.0, ..Look::default() };
        assert!(gray_after(210, &hi) < 185, "highlights pull bright areas down");
        let white = gray_after(255, &hi);
        assert!(white > 200 && white > gray_after(210, &hi), "white stays the brightest tone: {white}");
        assert!(gray_after(40, &hi) >= 39, "dark areas stay");
        // Detail inside a dark area keeps its contrast.
        let mut img = RgbaImage::from_fn(64, 64, |x, _| {
            let v = if x % 2 == 0 { 40 } else { 60 };
            image::Rgba([v, v, v, 255])
        });
        apply_tone(&mut img, &sh);
        let (a, b) = (img.get_pixel(32, 32)[0] as i32, img.get_pixel(33, 32)[0] as i32);
        assert!((a - b).abs() > 20, "{a} {b}");
    }

    #[test]
    fn vibrance_holds_back_on_skin() {
        let gain = |c: [u8; 3]| {
            let mut img = RgbaImage::from_pixel(4, 4, image::Rgba([c[0], c[1], c[2], 255]));
            apply_tone(&mut img, &Look { vibrance: 1.0, ..Look::default() });
            let p = img.get_pixel(1, 1);
            (p[0] as i32 - p[2] as i32) - (c[0] as i32 - c[2] as i32)
        };
        // A skin tone and a blue-green of similar saturation.
        let skin = gain([200, 150, 120]);
        let other = [120u8, 170, 200];
        let mut img = RgbaImage::from_pixel(4, 4, image::Rgba([other[0], other[1], other[2], 255]));
        apply_tone(&mut img, &Look { vibrance: 1.0, ..Look::default() });
        let p = img.get_pixel(1, 1);
        let cool = (p[2] as i32 - p[0] as i32) - (other[2] as i32 - other[0] as i32);
        assert!(skin > 0 && skin < cool, "{skin} {cool}");
    }

    #[test]
    fn green_shift_turns_leaves_teal() {
        let mut img = RgbaImage::from_pixel(4, 4, image::Rgba([60, 140, 50, 255]));
        let before = *img.get_pixel(0, 0);
        apply_tone(&mut img, &Look { green_shift: 1.0, ..Look::default() });
        let after = *img.get_pixel(0, 0);
        assert!(after[2] > before[2] + 40, "blue rises: {after:?}");
        assert!(after[0] < before[0], "red drops: {after:?}");
        // Gray and sky-blue pixels are left alone, even with the green curve.
        let look = Look { green_shift: 1.0, green_curve: 0.6, ..Look::default() };
        for c in [[120u8, 120, 120], [90, 140, 220], [240, 170, 90]] {
            let mut px = RgbaImage::from_pixel(2, 2, image::Rgba([c[0], c[1], c[2], 255]));
            apply_tone(&mut px, &look);
            assert_eq!(*px.get_pixel(0, 0), image::Rgba([c[0], c[1], c[2], 255]));
        }
    }

    #[test]
    fn grain_differs_per_frame_and_zero_does_nothing() {
        // A full-size photo (short side 3000 px).
        let base = RgbaImage::from_pixel(3000, 3000, image::Rgba([128, 128, 128, 255]));
        let mut a = base.clone();
        let mut b = base.clone();
        apply_grain(&mut a, 0.5, 1.0, 0.5, 1);
        apply_grain(&mut b, 0.5, 1.0, 0.5, 2);
        assert_ne!(a, base);
        assert_ne!(a, b);
        let mut c = base.clone();
        apply_grain(&mut c, 0.0, 1.0, 0.5, 1);
        assert_eq!(c, base);
        // Roughness changes the pattern but not the overall strength much.
        let spread = |img: &RgbaImage| {
            img.pixels().map(|p| (p[0] as f32 - 128.0).abs()).sum::<f32>() / img.pixels().len() as f32
        };
        let mut even = base.clone();
        let mut rough = base.clone();
        apply_grain(&mut even, 0.5, 1.0, 0.0, 1);
        apply_grain(&mut rough, 0.5, 1.0, 1.0, 1);
        assert_ne!(even, rough);
        assert!((spread(&even) - spread(&rough)).abs() < spread(&even) * 0.5);
        // A small preview of the same photo gets fainter grain, as the
        // full-size grain looks when shrunk to that size.
        let mut preview = RgbaImage::from_pixel(720, 540, image::Rgba([128, 128, 128, 255]));
        apply_grain(&mut preview, 0.5, 1.0, 0.0, 1);
        assert!(spread(&preview) < spread(&even) * 0.5, "{} {}", spread(&preview), spread(&even));
    }

    #[test]
    fn texture_and_sharpening_raise_edge_contrast() {
        // A soft vertical edge from dark to light, in a full-size photo.
        let base = RgbaImage::from_fn(3000, 3000, |x, _| {
            let v = (60.0 + 140.0 / (1.0 + (-(x as f32 - 32.0) / 2.0).exp())) as u8;
            image::Rgba([v, v, v, 255])
        });
        let step = |img: &RgbaImage| img.get_pixel(34, 32)[0] as i32 - img.get_pixel(30, 32)[0] as i32;
        for look in [Look { texture: 1.0, ..Look::default() }, Look { sharpen: 1.0, ..Look::default() }] {
            let mut img = base.clone();
            apply_tone(&mut img, &look);
            assert!(step(&img) > step(&base), "{look:?}");
        }
    }

    #[test]
    fn vignette_outline_and_lightening() {
        let base = RgbaImage::from_pixel(200, 100, image::Rgba([128, 128, 128, 255]));
        let corner = |look: &Look| {
            let mut img = base.clone();
            apply_tone(&mut img, look);
            (img.get_pixel(0, 0)[0], img.get_pixel(100, 50)[0], img.get_pixel(199, 50)[0])
        };
        let dark = Look { vignette: 1.0, ..Look::default() };
        let (c, mid, _) = corner(&dark);
        assert!(c < 100 && mid == 128, "dark corners, clear center");
        let (c, _, _) = corner(&Look { vignette: -1.0, ..Look::default() });
        assert!(c > 160, "negative lightens the corners");
        // A later midpoint leaves more of the picture clear.
        let (_, _, edge_early) = corner(&Look { vignette_mid: 0.0, ..dark });
        let (_, _, edge_late) = corner(&Look { vignette_mid: 1.0, ..dark });
        assert!(edge_late > edge_early);
        // Outline settings do nothing while the vignette is off.
        assert!(Look { vignette_round: 1.0, vignette_feather: 0.0, ..Look::default() }.is_neutral());
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
