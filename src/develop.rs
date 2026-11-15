//! Photo-editor style controls that match common raw-developer settings:
//! point tone curves, per-color hue/saturation/luminance (HSL), and
//! three-way color grading. Used by `finish::apply_tone`.

/// Up to 16 points of a tone curve, both coordinates 0..1, sorted by x.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Curve {
    pub pts: [[f32; 2]; 16],
    pub n: u8,
}

pub const MAX_POINTS: usize = 16;

impl Default for Curve {
    fn default() -> Self {
        let mut pts = [[0.0; 2]; 16];
        pts[1] = [1.0, 1.0];
        Self { pts, n: 2 }
    }
}

impl Curve {
    pub fn points(&self) -> &[[f32; 2]] {
        &self.pts[..self.n as usize]
    }

    pub fn from_points(points: &[[f32; 2]]) -> Self {
        let mut v: Vec<[f32; 2]> = points
            .iter()
            .map(|p| [p[0].clamp(0.0, 1.0), p[1].clamp(0.0, 1.0)])
            .collect();
        v.sort_by(|a, b| a[0].total_cmp(&b[0]));
        v.dedup_by(|a, b| (a[0] - b[0]).abs() < 1e-4);
        if v.len() < 2 {
            return Self::default();
        }
        // Keep at most MAX_POINTS, spread evenly.
        if v.len() > MAX_POINTS {
            let step = (v.len() - 1) as f32 / (MAX_POINTS - 1) as f32;
            v = (0..MAX_POINTS).map(|i| v[((i as f32 * step).round() as usize).min(v.len() - 1)]).collect();
        }
        let mut pts = [[0.0; 2]; 16];
        for (i, p) in v.iter().enumerate() {
            pts[i] = *p;
        }
        Self { pts, n: v.len() as u8 }
    }

    pub fn is_identity(&self) -> bool {
        self.points().iter().all(|p| (p[0] - p[1]).abs() < 1e-4)
    }

    /// Smooth value at `x` (monotone cubic through the points).
    pub fn eval(&self, x: f32) -> f32 {
        let p = self.points();
        let n = p.len();
        if n == 0 {
            return x;
        }
        if x <= p[0][0] {
            return p[0][1];
        }
        if x >= p[n - 1][0] {
            return p[n - 1][1];
        }
        // Slopes of the segments and tangents at the points (Fritsch-Carlson).
        let d: Vec<f32> = (0..n - 1)
            .map(|i| (p[i + 1][1] - p[i][1]) / (p[i + 1][0] - p[i][0]).max(1e-6))
            .collect();
        let mut m = vec![0.0f32; n];
        m[0] = d[0];
        m[n - 1] = d[n - 2];
        for i in 1..n - 1 {
            m[i] = if d[i - 1] * d[i] <= 0.0 { 0.0 } else { (d[i - 1] + d[i]) / 2.0 };
        }
        for i in 0..n - 1 {
            if d[i].abs() < 1e-9 {
                m[i] = 0.0;
                m[i + 1] = 0.0;
            } else {
                let a = m[i] / d[i];
                let b = m[i + 1] / d[i];
                let s = a * a + b * b;
                if s > 9.0 {
                    let t = 3.0 / s.sqrt();
                    m[i] = t * a * d[i];
                    m[i + 1] = t * b * d[i];
                }
            }
        }
        let i = (0..n - 1).find(|&i| x <= p[i + 1][0]).unwrap_or(n - 2);
        let h = (p[i + 1][0] - p[i][0]).max(1e-6);
        let t = (x - p[i][0]) / h;
        let (t2, t3) = (t * t, t * t * t);
        let y = (2.0 * t3 - 3.0 * t2 + 1.0) * p[i][1]
            + (t3 - 2.0 * t2 + t) * h * m[i]
            + (-2.0 * t3 + 3.0 * t2) * p[i + 1][1]
            + (t3 - t2) * h * m[i + 1];
        y.clamp(0.0, 1.0)
    }

    /// The curve moved toward a straight line: 0 = straight, 1 = unchanged.
    pub fn scaled(&self, t: f32) -> Self {
        let mut c = *self;
        for p in c.pts.iter_mut().take(c.n as usize) {
            p[1] = p[0] + (p[1] - p[0]) * t;
        }
        c
    }
}

/// The eight HSL color bands and their center hues (degrees).
pub const HSL_NAMES: [&str; 8] = ["Red", "Orange", "Yellow", "Green", "Aqua", "Blue", "Purple", "Magenta"];
pub const HSL_CENTERS: [f32; 8] = [0.0, 30.0, 60.0, 120.0, 180.0, 240.0, 270.0, 300.0];
/// Swatch colors for the bands (for the controls).
pub const HSL_SWATCH: [[u8; 3]; 8] = [
    [225, 60, 60],
    [235, 140, 50],
    [230, 210, 60],
    [70, 180, 80],
    [60, 200, 200],
    [60, 110, 230],
    [140, 80, 220],
    [220, 70, 190],
];

/// How much each band applies to `hue`; the weights add up to 1.
pub fn band_weights(hue: f32) -> [f32; 8] {
    let h = hue.rem_euclid(360.0);
    let mut w = [0.0f32; 8];
    for i in 0..8 {
        let c = HSL_CENTERS[i];
        let prev = HSL_CENTERS[(i + 7) % 8];
        let next = HSL_CENTERS[(i + 1) % 8];
        // Distances measured around the color wheel.
        let span_prev = (c - prev).rem_euclid(360.0);
        let span_next = (next - c).rem_euclid(360.0);
        let d = (h - c + 540.0).rem_euclid(360.0) - 180.0; // -180..180
        w[i] = if d < 0.0 { (1.0 + d / span_prev).max(0.0) } else { (1.0 - d / span_next).max(0.0) };
    }
    w
}

pub fn rgb_to_hsl(c: [f32; 3]) -> [f32; 3] {
    let (r, g, b) = (c[0], c[1], c[2]);
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let l = (max + min) / 2.0;
    if (max - min).abs() < 1e-6 {
        return [0.0, 0.0, l];
    }
    let d = max - min;
    let s = if l > 0.5 { d / (2.0 - max - min) } else { d / (max + min) };
    let h = if max == r {
        ((g - b) / d).rem_euclid(6.0)
    } else if max == g {
        (b - r) / d + 2.0
    } else {
        (r - g) / d + 4.0
    } * 60.0;
    [h, s, l]
}

pub fn hsl_to_rgb(hsl: [f32; 3]) -> [f32; 3] {
    let [h, s, l] = hsl;
    if s <= 0.0 {
        return [l, l, l];
    }
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let hp = h.rem_euclid(360.0) / 60.0;
    let x = c * (1.0 - (hp % 2.0 - 1.0).abs());
    let (r, g, b) = match hp as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let m = l - c / 2.0;
    [r + m, g + m, b + m]
}

/// Per-color hue (±30° at ±1), saturation and luminance changes.
pub fn apply_hsl(c: [f32; 3], hsl: &[[f32; 3]; 8]) -> [f32; 3] {
    let [h, s, l] = rgb_to_hsl([c[0].clamp(0.0, 1.0), c[1].clamp(0.0, 1.0), c[2].clamp(0.0, 1.0)]);
    if s < 1e-3 {
        return c;
    }
    let w = band_weights(h);
    let (mut dh, mut ds, mut dl) = (0.0, 0.0, 0.0);
    for i in 0..8 {
        dh += w[i] * hsl[i][0];
        ds += w[i] * hsl[i][1];
        dl += w[i] * hsl[i][2];
    }
    let h2 = h + dh * 30.0;
    let s2 = (s * (1.0 + ds)).clamp(0.0, 1.0);
    // Luminance change fades out for grayish colors.
    let l2 = (l + dl * 0.3 * s.min(1.0)).clamp(0.0, 1.0);
    hsl_to_rgb([h2, s2, l2])
}

/// A tint for color grading: a hue (degrees) at a saturation (0..1),
/// as small RGB offsets centered on zero.
pub fn tone_color(hue: f32, sat: f32, scale: f32) -> [f32; 3] {
    let rgb = hsl_to_rgb([hue, 1.0, 0.5]);
    let mean = (rgb[0] + rgb[1] + rgb[2]) / 3.0;
    let s = sat.clamp(0.0, 1.0) * scale;
    [(rgb[0] - mean) * s, (rgb[1] - mean) * s, (rgb[2] - mean) * s]
}

/// Color grading offset for a pixel of luminance `l`.
/// `grade` holds (hue, saturation, luminance) for shadows, midtones,
/// highlights and the whole picture. `balance` (-1..1) moves the split
/// toward shadows or highlights; `blend` (0..1) sets how much the three
/// ranges overlap (0: sharply separated, 1: wide and soft).
pub fn grade_offset(l: f32, grade: &[[f32; 3]; 4], balance: f32, blend: f32) -> [f32; 3] {
    // Positive balance gives the highlight colors more of the range.
    let ls = (l + balance * 0.25).clamp(0.0, 1.0);
    // At the default blend (0.5) the exponent is 2 and the three weights
    // add up to 1.
    let k = 2f32.powf(2.0 * (1.0 - blend.clamp(0.0, 1.0)));
    let ws = (1.0 - ls).powf(k);
    let wh = ls.powf(k);
    let wm = 0.5 * (4.0 * ls * (1.0 - ls)).powf(k * 0.5);
    let mut out = [0.0f32; 3];
    for (i, (g, w)) in grade.iter().zip([ws, wm, wh, 1.0]).enumerate() {
        let mut t = [0.0f32; 3];
        if g[1] > 0.0 {
            t = tone_color(g[0], g[1], if i == 3 { 0.15 } else { 0.2 });
        }
        let lift = g[2] * 0.2;
        for c in 0..3 {
            out[c] += (t[c] + lift) * w;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn straight_curve_changes_nothing() {
        let c = Curve::default();
        assert!(c.is_identity());
        for x in [0.0, 0.2, 0.5, 0.9, 1.0] {
            assert!((c.eval(x) - x).abs() < 1e-5);
        }
    }

    #[test]
    fn s_shaped_curve_passes_through_its_points() {
        let c = Curve::from_points(&[[0.0, 0.05], [0.25, 0.18], [0.75, 0.84], [1.0, 1.0]]);
        assert!((c.eval(0.25) - 0.18).abs() < 1e-4);
        assert!((c.eval(0.75) - 0.84).abs() < 1e-4);
        assert!((c.eval(0.0) - 0.05).abs() < 1e-4);
        assert!(c.eval(0.5) > 0.4 && c.eval(0.5) < 0.6);
        let half = c.scaled(0.5);
        assert!((half.eval(0.25) - 0.215).abs() < 1e-3);
    }

    #[test]
    fn band_weights_add_up_and_pick_the_right_color() {
        for h in [0.0f32, 15.0, 45.0, 100.0, 170.0, 250.0, 290.0, 330.0, 359.0] {
            let w = band_weights(h);
            assert!((w.iter().sum::<f32>() - 1.0).abs() < 1e-4, "{h}: {w:?}");
        }
        assert!((band_weights(120.0)[3] - 1.0).abs() < 1e-6);
        assert!((band_weights(0.0)[0] - 1.0).abs() < 1e-6);
    }

    #[test]
    fn hsl_round_trip_and_red_desaturation() {
        let c = [0.8, 0.3, 0.2];
        let back = hsl_to_rgb(rgb_to_hsl(c));
        for k in 0..3 {
            assert!((back[k] - c[k]).abs() < 1e-4);
        }
        let mut hsl = [[0.0f32; 3]; 8];
        hsl[0][1] = -1.0; // remove red saturation
        let out = apply_hsl([0.9, 0.1, 0.1], &hsl);
        assert!((out[0] - out[1]).abs() < 0.02, "{out:?}");
        // Blue is untouched by a red change.
        let blue = apply_hsl([0.1, 0.2, 0.9], &hsl);
        assert!((blue[2] - 0.9).abs() < 1e-3);
    }

    #[test]
    fn grading_tints_shadows_and_highlights_separately() {
        let grade = [[220.0, 1.0, 0.0], [0.0, 0.0, 0.0], [40.0, 1.0, 0.0], [0.0; 3]];
        let dark = grade_offset(0.05, &grade, 0.0, 0.5);
        let bright = grade_offset(0.95, &grade, 0.0, 0.5);
        assert!(dark[2] > dark[0], "blue shadows {dark:?}");
        assert!(bright[0] > bright[2], "warm highlights {bright:?}");
    }

    #[test]
    fn grading_luminance_global_and_blending() {
        // Shadow luminance brightens dark pixels much more than bright ones.
        let lum = [[0.0, 0.0, 1.0], [0.0; 3], [0.0; 3], [0.0; 3]];
        assert!(grade_offset(0.05, &lum, 0.0, 0.5)[0] > 0.15);
        assert!(grade_offset(0.95, &lum, 0.0, 0.5)[0] < 0.01);
        // The global wheel tints every brightness the same.
        let global = [[0.0; 3], [0.0; 3], [0.0; 3], [220.0, 1.0, 0.0]];
        assert_eq!(grade_offset(0.1, &global, 0.0, 0.5), grade_offset(0.9, &global, 0.0, 0.5));
        // More blending lets the shadow tint reach further into bright pixels.
        let sh = [[220.0, 1.0, 0.0], [0.0; 3], [0.0; 3], [0.0; 3]];
        let narrow = grade_offset(0.6, &sh, 0.0, 0.0)[2];
        let wide = grade_offset(0.6, &sh, 0.0, 1.0)[2];
        assert!(wide > narrow, "{narrow} {wide}");
        // Positive balance favors the highlight color, negative the shadow color.
        let both = [[220.0, 1.0, 0.0], [0.0; 3], [40.0, 1.0, 0.0], [0.0; 3]];
        let warmth = |b: f32| {
            let o = grade_offset(0.5, &both, b, 0.5);
            o[0] - o[2]
        };
        assert!(warmth(1.0) > warmth(0.0) && warmth(0.0) > warmth(-1.0));
    }
}
