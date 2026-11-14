//! Quick automatic guess of the regions that usually move in landscape
//! photo animations (sky and water). This is a color-and-position estimate,
//! not a learned segmentation, so the person is
//! expected to touch it up with the brush.

use image::RgbaImage;

pub fn guess_sky_and_water(img: &RgbaImage) -> Vec<f32> {
    let (w, h) = img.dimensions();
    let mut score = vec![0.0f32; (w * h) as usize];
    for y in 0..h {
        let vy = y as f32 / h as f32;
        for x in 0..w {
            let p = img.get_pixel(x, y).0;
            let (r, g, b) = (p[0] as f32 / 255.0, p[1] as f32 / 255.0, p[2] as f32 / 255.0);
            let max = r.max(g).max(b);
            let min = r.min(g).min(b);
            let lum = 0.299 * r + 0.587 * g + 0.114 * b;
            let sat = if max > 0.0 { (max - min) / max } else { 0.0 };
            let blue = b - 0.5 * (r + g);

            // Sky: blue or bright-and-gray (clouds), stronger near the top.
            let top = (1.0 - vy / 0.65).clamp(0.0, 1.0);
            let sky = (blue * 6.0).clamp(0.0, 1.0).max(((lum - 0.6) * 4.0).clamp(0.0, 1.0) * (1.0 - sat))
                * top.sqrt();

            // Water: bluish/cyan reflections in the lower part.
            let bottom = ((vy - 0.45) / 0.3).clamp(0.0, 1.0);
            let cyan = (b + g) * 0.5 - r;
            let water = ((blue * 5.0).max(cyan * 4.0)).clamp(0.0, 1.0) * bottom;

            score[(y * w + x) as usize] = sky.max(water);
        }
    }
    let smooth = crate::scene::blur_mask(&score, w, h, (w.max(h) / 90).max(2));
    smooth
        .into_iter()
        .map(|s| {
            let t = ((s - 0.35) / 0.25).clamp(0.0, 1.0);
            t * t * (3.0 - 2.0 * t)
        })
        .collect()
}
