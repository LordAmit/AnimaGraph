//! The main colors of a picture, for the color swatches (k-means on a
//! small copy of the picture).

use image::RgbaImage;

fn dist2(a: [f32; 3], b: [f32; 3]) -> f32 {
    (a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)
}

/// Up to `n` distinct main colors, most common first. Near-white and
/// near-black colors are left out, since white and black have their own
/// swatches.
pub fn dominant_colors(img: &RgbaImage, n: usize) -> Vec<[u8; 3]> {
    let small = crate::scene::downscale(img, 64);
    let pts: Vec<[f32; 3]> = small.pixels().map(|p| [p[0] as f32, p[1] as f32, p[2] as f32]).collect();
    if pts.is_empty() || n == 0 {
        return Vec::new();
    }
    // Start from spread-out points (deterministic): each new center is the
    // pixel farthest from the centers chosen so far.
    let k = (n + 4).min(pts.len());
    let mut centers = vec![pts[pts.len() / 2]];
    while centers.len() < k {
        let far = pts
            .iter()
            .max_by(|a, b| {
                let da = centers.iter().map(|c| dist2(**a, *c)).fold(f32::MAX, f32::min);
                let db = centers.iter().map(|c| dist2(**b, *c)).fold(f32::MAX, f32::min);
                da.total_cmp(&db)
            })
            .copied()
            .unwrap_or([0.0; 3]);
        centers.push(far);
    }
    let mut counts = vec![0usize; k];
    for _ in 0..12 {
        let mut sums = vec![[0.0f32; 3]; k];
        counts = vec![0; k];
        for p in &pts {
            let (i, _) = centers
                .iter()
                .enumerate()
                .map(|(i, c)| (i, dist2(*p, *c)))
                .min_by(|a, b| a.1.total_cmp(&b.1))
                .unwrap();
            for j in 0..3 {
                sums[i][j] += p[j];
            }
            counts[i] += 1;
        }
        for i in 0..k {
            if counts[i] > 0 {
                centers[i] = [sums[i][0] / counts[i] as f32, sums[i][1] / counts[i] as f32, sums[i][2] / counts[i] as f32];
            }
        }
    }
    let mut order: Vec<usize> = (0..k).filter(|&i| counts[i] > 0).collect();
    order.sort_by(|a, b| counts[*b].cmp(&counts[*a]));
    let white = [255.0; 3];
    let black = [0.0; 3];
    let mut out: Vec<[f32; 3]> = Vec::new();
    for i in order {
        let c = centers[i];
        let too_plain = dist2(c, white) < 40.0f32.powi(2) || dist2(c, black) < 40.0f32.powi(2);
        let too_close = out.iter().any(|o| dist2(*o, c) < 45.0f32.powi(2));
        if !too_plain && !too_close {
            out.push(c);
        }
        if out.len() == n {
            break;
        }
    }
    out.into_iter()
        .map(|c| [c[0].round() as u8, c[1].round() as u8, c[2].round() as u8])
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_the_main_colors_largest_first() {
        // Half blue, a third orange, a strip of green, plus white and black.
        let img = RgbaImage::from_fn(120, 60, |x, _| match x {
            0..=59 => image::Rgba([30, 80, 200, 255]),
            60..=99 => image::Rgba([240, 140, 40, 255]),
            100..=109 => image::Rgba([40, 160, 70, 255]),
            110..=114 => image::Rgba([255, 255, 255, 255]),
            _ => image::Rgba([0, 0, 0, 255]),
        });
        let c = dominant_colors(&img, 3);
        assert_eq!(c.len(), 3, "{c:?}");
        let near = |a: [u8; 3], b: [u8; 3]| (0..3).all(|i| (a[i] as i32 - b[i] as i32).abs() < 20);
        assert!(near(c[0], [30, 80, 200]), "{c:?}");
        assert!(near(c[1], [240, 140, 40]), "{c:?}");
        assert!(near(c[2], [40, 160, 70]), "{c:?}");
    }
}
