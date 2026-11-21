//! Photo corners inside a border: rounded corners on the photo, and paper
//! corner mounts (small triangles holding the photo, as in an album).
//! The same shapes are drawn on saved pictures and on the canvas preview.

use image::RgbaImage;

/// The four mount triangles for a photo at `rect` (x, y, w, h): each has its
/// right angle just outside a photo corner and its long side across the
/// photo. `size` is the length along each photo edge.
pub fn mount_triangles(rect: [f32; 4], size: f32) -> [[[f32; 2]; 3]; 4] {
    let [x, y, w, h] = rect;
    let e = size * 0.18; // how far the mount reaches past the photo's edge
    let tri = |cx: f32, cy: f32, sx: f32, sy: f32| {
        // (sx, sy) point into the photo from this corner.
        let o = [cx - sx * e, cy - sy * e];
        [o, [o[0] + sx * (size + e), o[1]], [o[0], o[1] + sy * (size + e)]]
    };
    [tri(x, y, 1.0, 1.0), tri(x + w, y, -1.0, 1.0), tri(x, y + h, 1.0, -1.0), tri(x + w, y + h, -1.0, -1.0)]
}

/// How much of the pixel at (px, py) (its centre) lies inside the photo
/// with rounded corners of `radius`, 0 to 1.
pub fn rounded_coverage(px: f32, py: f32, rect: [f32; 4], radius: f32) -> f32 {
    let [x, y, w, h] = rect;
    let r = radius.min(w / 2.0).min(h / 2.0).max(0.0);
    if r <= 0.0 {
        return 1.0;
    }
    // Distance from the nearest corner circle's centre, only in the corners.
    let cx = px.clamp(x + r, x + w - r);
    let cy = py.clamp(y + r, y + h - r);
    let (dx, dy) = (px - cx, py - cy);
    if dx == 0.0 || dy == 0.0 {
        return 1.0;
    }
    let d = (dx * dx + dy * dy).sqrt();
    (r - d + 0.5).clamp(0.0, 1.0)
}

/// How much of the pixel at (px, py) a triangle covers, 0 to 1 (smooth on
/// the long side, sharp on the two straight sides).
fn triangle_coverage(px: f32, py: f32, t: &[[f32; 2]; 3]) -> f32 {
    let [o, a, b] = *t;
    let (sx, sy) = ((a[0] - o[0]).signum(), (b[1] - o[1]).signum());
    let along_x = (px - o[0]) * sx;
    let along_y = (py - o[1]) * sy;
    let len = (a[0] - o[0]).abs();
    if along_x < -0.5 || along_y < -0.5 {
        return 0.0;
    }
    let edge = |v: f32| (v + 0.5).clamp(0.0, 1.0);
    let hyp = (len - along_x - along_y) / std::f32::consts::SQRT_2;
    edge(along_x) * edge(along_y) * (hyp + 0.5).clamp(0.0, 1.0)
}

/// Round the photo's corners (filling with the border colour) and draw the
/// corner mounts, on a picture where the photo sits at `rect`.
pub fn apply(out: &mut RgbaImage, rect: [f32; 4], border: &crate::border::Border) {
    let short = rect[2].min(rect[3]);
    let radius = border.radius.max(0.0) * short;
    let mounts = border.mounts.then(|| mount_triangles(rect, border.mount_size.max(0.0) * short));
    if radius <= 0.0 && mounts.is_none() {
        return;
    }
    let bg = border.color;
    let mc = border.mount_color;
    let (w, h) = out.dimensions();
    // Only the pixels near the four corners can change.
    let reach = radius.max(mounts.map_or(0.0, |_| border.mount_size * short * 1.3)) + 2.0;
    let corners = [[rect[0], rect[1]], [rect[0] + rect[2], rect[1]], [rect[0], rect[1] + rect[3]], [rect[0] + rect[2], rect[1] + rect[3]]];
    for c in corners {
        let x0 = ((c[0] - reach).floor().max(0.0)) as u32;
        let x1 = ((c[0] + reach).ceil().max(0.0) as u32).min(w);
        let y0 = ((c[1] - reach).floor().max(0.0)) as u32;
        let y1 = ((c[1] + reach).ceil().max(0.0) as u32).min(h);
        for y in y0..y1 {
            for x in x0..x1 {
                let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
                let p = out.get_pixel_mut(x, y);
                let inside_photo = px >= rect[0] && px < rect[0] + rect[2] && py >= rect[1] && py < rect[1] + rect[3];
                if radius > 0.0 && inside_photo {
                    let a = rounded_coverage(px, py, rect, radius);
                    if a < 1.0 {
                        for k in 0..3 {
                            p[k] = (p[k] as f32 * a + bg[k] as f32 * (1.0 - a)).round() as u8;
                        }
                    }
                }
                if let Some(tris) = &mounts {
                    let a = tris.iter().map(|t| triangle_coverage(px, py, t)).fold(0.0f32, f32::max);
                    if a > 0.0 {
                        for k in 0..3 {
                            p[k] = (p[k] as f32 * (1.0 - a) + mc[k] as f32 * a).round() as u8;
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rounded_corner_is_outside_only_at_the_corner() {
        let rect = [0.0, 0.0, 100.0, 60.0];
        assert_eq!(rounded_coverage(0.5, 0.5, rect, 20.0), 0.0);
        assert_eq!(rounded_coverage(50.0, 0.5, rect, 20.0), 1.0);
        assert_eq!(rounded_coverage(20.0, 20.0, rect, 20.0), 1.0);
        assert_eq!(rounded_coverage(0.5, 0.5, rect, 0.0), 1.0);
    }

    #[test]
    fn mounts_cover_the_photo_corner_but_not_its_middle() {
        let tris = mount_triangles([10.0, 10.0, 100.0, 80.0], 20.0);
        // Top-left: covers the corner, reaches a little past the edge.
        assert!(triangle_coverage(11.0, 11.0, &tris[0]) > 0.99);
        assert!(triangle_coverage(9.0, 9.0, &tris[0]) > 0.99);
        assert_eq!(triangle_coverage(40.0, 40.0, &tris[0]), 0.0);
        // Bottom-right mirrors it.
        assert!(triangle_coverage(109.0, 89.0, &tris[3]) > 0.99);
        assert_eq!(triangle_coverage(80.0, 60.0, &tris[3]), 0.0);
    }
}
