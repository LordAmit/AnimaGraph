//! Vectorscope: where the picture's colours sit by hue (angle) and
//! saturation (distance from the centre), as on video grading scopes.
//! Colours are measured as Cb / Cr (Rec. 709) from the shown picture.

use eframe::egui::{self, Color32};
use image::RgbaImage;

/// Side of the scope picture, in pixels.
pub const SIZE: usize = 220;

/// Cb and Cr (each about -0.5 to 0.5) of an sRGB colour in 0..1.
pub fn chroma(r: f32, g: f32, b: f32) -> [f32; 2] {
    let y = 0.2126 * r + 0.7152 * g + 0.0722 * b;
    [(b - y) / 1.8556, (r - y) / 1.5748]
}

/// Where a Cb / Cr value lands in the scope picture (0..SIZE), with Cr up.
/// `zoom` 1 puts full saturation (chroma 0.5) at the edge of the circle.
pub fn to_scope(c: [f32; 2], zoom: f32) -> [f32; 2] {
    let radius = SIZE as f32 / 2.0 - 4.0;
    let k = radius / 0.5 * zoom;
    [SIZE as f32 / 2.0 + c[0] * k, SIZE as f32 / 2.0 - c[1] * k]
}

/// The colour targets at 75% (as on a broadcast scope): label and Cb / Cr.
pub fn targets() -> [(&'static str, [f32; 2]); 6] {
    let c = |r: f32, g: f32, b: f32| chroma(r * 0.75, g * 0.75, b * 0.75);
    [
        ("R", c(1.0, 0.0, 0.0)),
        ("Mg", c(1.0, 0.0, 1.0)),
        ("B", c(0.0, 0.0, 1.0)),
        ("Cy", c(0.0, 1.0, 1.0)),
        ("G", c(0.0, 1.0, 0.0)),
        ("Yl", c(1.0, 1.0, 0.0)),
    ]
}

/// Angle of the skin tone line, from the Cb axis towards Cr.
pub const SKIN_ANGLE: f32 = 123.0;
/// How far, on each side of the line, natural skin tones usually fall.
pub const SKIN_SPREAD: f32 = 12.0;

/// Direction of the skin tone line.
pub fn skin_direction() -> [f32; 2] {
    let a = SKIN_ANGLE.to_radians();
    [a.cos(), a.sin()]
}

/// The skin tone band as directions (Cb, Cr) from one side to the other,
/// for drawing a wedge from the centre.
pub fn skin_band() -> Vec<[f32; 2]> {
    (0..=12)
        .map(|i| {
            let a = (SKIN_ANGLE - SKIN_SPREAD + 2.0 * SKIN_SPREAD * i as f32 / 12.0).to_radians();
            [a.cos(), a.sin()]
        })
        .collect()
}

#[cfg(test)]
/// Is a Cb / Cr colour inside the skin tone band?
pub fn in_skin_band(c: [f32; 2]) -> bool {
    let a = c[1].atan2(c[0]).to_degrees();
    (a - SKIN_ANGLE).abs() <= SKIN_SPREAD
}

/// The scope trace for a picture: each spot is coloured with the average
/// colour of the pixels that land there, brighter where more pixels land.
/// Transparent where nothing lands, so a graticule can be drawn under it.
pub fn trace(img: &RgbaImage, zoom: f32) -> egui::ColorImage {
    let (w, h) = img.dimensions();
    let mut count = vec![0u32; SIZE * SIZE];
    let mut sum = vec![[0f32; 3]; SIZE * SIZE];
    // About 60 000 samples, whatever the picture size.
    let step = (((w as f32 * h as f32) / 60_000.0).sqrt().floor() as u32).max(1);
    for y in (0..h).step_by(step as usize) {
        for x in (0..w).step_by(step as usize) {
            let p = img.get_pixel(x, y);
            let (r, g, b) = (p[0] as f32 / 255.0, p[1] as f32 / 255.0, p[2] as f32 / 255.0);
            let [sx, sy] = to_scope(chroma(r, g, b), zoom);
            if sx < 0.0 || sy < 0.0 || sx >= SIZE as f32 || sy >= SIZE as f32 {
                continue;
            }
            let i = sy as usize * SIZE + sx as usize;
            count[i] += 1;
            sum[i][0] += r;
            sum[i][1] += g;
            sum[i][2] += b;
        }
    }
    let max = count.iter().copied().max().unwrap_or(0).max(1) as f32;
    let pixels = count
        .iter()
        .zip(&sum)
        .map(|(&n, s)| {
            if n == 0 {
                return Color32::TRANSPARENT;
            }
            let n_f = n as f32;
            let avg = [s[0] / n_f, s[1] / n_f, s[2] / n_f];
            // Full-strength version of the colour, so grey stays light grey.
            let top = avg[0].max(avg[1]).max(avg[2]).max(0.35);
            let level = 0.4 + 0.6 * ((1.0 + n_f).ln() / (1.0 + max).ln());
            let ch = |v: f32| ((v / top) * level * 255.0).round().clamp(0.0, 255.0) as u8;
            Color32::from_rgb(ch(avg[0]), ch(avg[1]), ch(avg[2]))
        })
        .collect();
    egui::ColorImage::new([SIZE, SIZE], pixels)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grey_sits_in_the_middle_and_red_up_left() {
        let [cb, cr] = chroma(0.5, 0.5, 0.5);
        assert!(cb.abs() < 1e-6 && cr.abs() < 1e-6);
        let [x, y] = to_scope(chroma(1.0, 0.0, 0.0), 1.0);
        let mid = SIZE as f32 / 2.0;
        assert!(x < mid && y < mid, "red should be up and to the left: {x} {y}");
        // Blue is to the right.
        assert!(to_scope(chroma(0.0, 0.0, 1.0), 1.0)[0] > mid);
    }

    #[test]
    fn skin_tones_fall_in_the_band() {
        let rgb = |r: u8, g: u8, b: u8| chroma(r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0);
        // Light to dark skin.
        for (r, g, b) in [(241, 194, 167), (232, 190, 172), (194, 140, 108), (160, 108, 78), (110, 70, 50), (74, 48, 36)] {
            assert!(in_skin_band(rgb(r, g, b)), "{r} {g} {b}");
        }
        // Strong red and yellow, and blue, are not skin.
        for (r, g, b) in [(255, 0, 0), (255, 255, 0), (40, 60, 200)] {
            assert!(!in_skin_band(rgb(r, g, b)), "{r} {g} {b}");
        }
        // The drawn band is the same band, with the line in its middle.
        let band = skin_band();
        assert!(band.iter().all(|&d| in_skin_band([d[0] * 0.999, d[1] * 0.999])));
        let mid = band[band.len() / 2];
        let line = skin_direction();
        assert!((mid[0] - line[0]).abs() < 1e-6 && (mid[1] - line[1]).abs() < 1e-6);
    }

    #[test]
    fn trace_marks_where_the_colours_are() {
        let img = RgbaImage::from_pixel(50, 50, image::Rgba([200, 40, 40, 255]));
        let t = trace(&img, 1.0);
        let lit: Vec<usize> = t.pixels.iter().enumerate().filter(|(_, c)| c.a() > 0).map(|(i, _)| i).collect();
        assert_eq!(lit.len(), 1);
        let [x, y] = to_scope(chroma(200.0 / 255.0, 40.0 / 255.0, 40.0 / 255.0), 1.0);
        assert_eq!(lit[0], y as usize * SIZE + x as usize);
    }
}
