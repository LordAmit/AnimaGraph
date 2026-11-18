//! Rotating a picture by any angle (straightening). The picture keeps its
//! size and is zoomed in just enough that no empty corners show.

use image::{Rgba, RgbaImage};

/// Zoom needed so a `w` x `h` picture turned by `degrees` still fills a
/// `w` x `h` frame.
pub fn fill_scale(w: f32, h: f32, degrees: f32) -> f32 {
    let t = degrees.to_radians();
    let (c, s) = (t.cos().abs(), t.sin().abs());
    ((w * c + h * s) / w).max((w * s + h * c) / h)
}

/// The picture turned by `degrees` (positive = clockwise), zoomed to fill
/// the same size. Smooth (bilinear) sampling.
pub fn rotate_fill(img: &RgbaImage, degrees: f32) -> RgbaImage {
    let (w, h) = img.dimensions();
    if degrees.abs() < 1e-4 || w < 2 || h < 2 {
        return img.clone();
    }
    let (wf, hf) = (w as f32, h as f32);
    let scale = fill_scale(wf, hf, degrees);
    let t = degrees.to_radians();
    let (cos, sin) = (t.cos() / scale, t.sin() / scale);
    let (cx, cy) = (wf / 2.0, hf / 2.0);
    let src = img.as_raw();
    let stride = w as usize * 4;
    let mut out = vec![0u8; src.len()];
    crate::render::par_chunks(&mut out, stride, |y, row| {
        let dy = y as f32 + 0.5 - cy;
        for x in 0..w as usize {
            let dx = x as f32 + 0.5 - cx;
            // Where this output pixel comes from in the source.
            let sx = (cos * dx + sin * dy + cx - 0.5).clamp(0.0, wf - 1.0);
            let sy = (-sin * dx + cos * dy + cy - 0.5).clamp(0.0, hf - 1.0);
            let (x0, y0) = (sx.floor() as usize, sy.floor() as usize);
            let (x1, y1) = ((x0 + 1).min(w as usize - 1), (y0 + 1).min(h as usize - 1));
            let (fx, fy) = (sx - x0 as f32, sy - y0 as f32);
            let at = |xx: usize, yy: usize, k: usize| src[yy * stride + xx * 4 + k] as f32;
            for k in 0..4 {
                let top = at(x0, y0, k) * (1.0 - fx) + at(x1, y0, k) * fx;
                let bottom = at(x0, y1, k) * (1.0 - fx) + at(x1, y1, k) * fx;
                row[x * 4 + k] = (top * (1.0 - fy) + bottom * fy).round() as u8;
            }
        }
    });
    RgbaImage::from_raw(w, h, out).unwrap_or_else(|| RgbaImage::from_pixel(w, h, Rgba([0, 0, 0, 255])))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_angle_changes_nothing() {
        let img = RgbaImage::from_fn(20, 10, |x, y| Rgba([x as u8 * 10, y as u8 * 20, 0, 255]));
        assert_eq!(rotate_fill(&img, 0.0), img);
    }

    #[test]
    fn fill_scale_grows_with_the_angle() {
        assert!((fill_scale(400.0, 300.0, 0.0) - 1.0).abs() < 1e-6);
        let a = fill_scale(400.0, 300.0, 5.0);
        let b = fill_scale(400.0, 300.0, 15.0);
        assert!(a > 1.0 && b > a);
        // Same zoom for the turned (portrait) picture.
        assert!((fill_scale(300.0, 400.0, 5.0) - a).abs() < 1e-5);
    }

    #[test]
    fn corners_stay_filled_and_the_turn_is_clockwise() {
        // Left half dark, right half light.
        let img = RgbaImage::from_fn(200, 100, |x, _| if x < 100 { Rgba([0, 0, 0, 255]) } else { Rgba([255, 255, 255, 255]) });
        let out = rotate_fill(&img, 10.0);
        assert_eq!(out.dimensions(), (200, 100));
        assert!(out.pixels().all(|p| p[3] == 255));
        // Turning clockwise moves the dividing line to the left at the bottom.
        let row = |y: u32| (0..200).find(|&x| out.get_pixel(x, y)[0] > 128).unwrap();
        assert!(row(95) < row(5), "{} {}", row(95), row(5));
    }
}
