//! Sky detection for the Sky mask.
//!
//! 1. The sky is grown from the top edge of the picture through smooth
//!    areas, stopping at real edges (the horizon, trees, buildings), so it
//!    reaches down to the horizon wherever that is.
//! 2. Sky seen through gaps (between branches, under arches) is added when
//!    it is smooth and matches the sky's colour at that height.
//! 3. The edge is fitted to the picture's own edges with a guided filter,
//!    so no halo is left around objects.
//!
//! This is a colour-and-edge estimate, not a learned segmentation: very
//! dark skies or skies that touch similar-looking walls may need touching
//! up with the brush.

use std::collections::VecDeque;

use image::RgbaImage;

/// Sky coverage (0..1) for each pixel of `img`, row by row.
pub fn sky_mask(img: &RgbaImage) -> Vec<f32> {
    let (w, h) = (img.width() as usize, img.height() as usize);
    if w < 4 || h < 4 {
        return vec![0.0; w * h];
    }
    let rgb: Vec<[f32; 3]> = img.pixels().map(|p| [p[0] as f32 / 255.0, p[1] as f32 / 255.0, p[2] as f32 / 255.0]).collect();
    let lum: Vec<f32> = rgb.iter().map(|c| 0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2]).collect();
    let grad = gradient(&box_blur(&lum, w, h, 1), w, h);

    // How strong an edge stops the sky: well above the texture of the sky
    // itself (clouds), measured along the top of the picture.
    let mut top: Vec<f32> = grad[..w * (h / 10).max(1)].to_vec();
    top.sort_by(|a, b| a.total_cmp(b));
    let median = top[top.len() / 2];
    let edge = (median * 4.0).clamp(0.035, 0.12);

    // 1. Grow smooth regions through gradually changing colour, starting
    //    anywhere in the top quarter (a thin dark line along the top edge, a
    //    dark corner or a notch must not stop it). A region counts as sky
    //    when it is a large part of the top quarter and looks like sky:
    //    blue, or bright (clouds, overcast, sunset).
    let band_end = (h / 4).max(2);
    let skip = (h / 100).max(1); // thin lines along the very top edge
    let band_area = (band_end - skip) * w;
    let mut label = vec![u32::MAX; w * h];
    let mut sky = vec![false; w * h];
    let step = 0.07f32;
    let mut next = 0u32;
    for y in skip..band_end {
        for x in 0..w {
            let start = y * w + x;
            if label[start] != u32::MAX || grad[start] >= edge || lum[start] < 0.06 {
                continue;
            }
            // One region.
            let id = next;
            next += 1;
            label[start] = id;
            let mut members = vec![start];
            let mut queue = VecDeque::from([start]);
            let (mut in_band, mut sum) = (0usize, [0f32; 3]);
            while let Some(i) = queue.pop_front() {
                let (px, py) = (i % w, i / w);
                if py >= skip && py < band_end {
                    in_band += 1;
                    for k in 0..3 {
                        sum[k] += rgb[i][k];
                    }
                }
                let neighbours = [
                    (px > 0).then(|| i - 1),
                    (px + 1 < w).then(|| i + 1),
                    (py > 0).then(|| i - w),
                    (py + 1 < h).then(|| i + w),
                ];
                for j in neighbours.into_iter().flatten() {
                    if label[j] == u32::MAX && grad[j] < edge && dist(rgb[i], rgb[j]) < step {
                        label[j] = id;
                        members.push(j);
                        queue.push_back(j);
                    }
                }
            }
            let n = in_band.max(1) as f32;
            let mean = [sum[0] / n, sum[1] / n, sum[2] / n];
            if in_band * 100 >= band_area * 3 && looks_like_sky(mean) {
                for i in members {
                    sky[i] = true;
                }
            }
        }
    }

    // 1b. Sky cut off by thin things (wires, rails, poles): a smooth,
    //     sky-coloured area with sky just above it, across no more than a
    //     thin line, at least as bright as that sky (hazy sky near the
    //     horizon is paler; the sea and the ground are darker, or separated
    //     by more than a thin line).
    add_sky_behind_thin_things(&mut sky, &rgb, &lum, &grad, edge, w, h);

    // 2. Sky through gaps: smooth pixels matching the sky colour of their row
    //    (rows without sky take the nearest row above), no lower than the
    //    lowest sky found.
    let mut row_colour: Vec<Option<[f32; 3]>> = vec![None; h];
    let mut lowest = 0;
    for y in 0..h {
        let (mut sum, mut n) = ([0f32; 3], 0f32);
        for x in 0..w {
            let i = y * w + x;
            if sky[i] {
                for k in 0..3 {
                    sum[k] += rgb[i][k];
                }
                n += 1.0;
            }
        }
        if n >= 3.0 {
            row_colour[y] = Some([sum[0] / n, sum[1] / n, sum[2] / n]);
            lowest = y;
        }
    }
    let mut last = None;
    for c in row_colour.iter_mut() {
        if c.is_some() {
            last = *c;
        } else {
            *c = last;
        }
    }
    let mut mask: Vec<f32> = sky.iter().map(|&s| if s { 1.0 } else { 0.0 }).collect();
    for y in 0..=lowest {
        let Some(c) = row_colour[y] else { continue };
        for x in 0..w {
            let i = y * w + x;
            // Smooth and close in colour, or a very close match even next to
            // an edge (sky seen between chains, cables and railings).
            let d = dist(rgb[i], c);
            if !sky[i] && ((grad[i] < edge && d < 0.09) || d < 0.05) {
                mask[i] = 1.0;
            }
        }
    }

    // Thin cracks along sharp cloud edges are sky too.
    close_cracks(&mut mask, w, h, 2);
    // Small holes enclosed by sky (bright cloud tops, birds) are sky too.
    fill_small_holes(&mut mask, w, h, (w * h) / 300);

    // 3. Fit the edge to the picture's edges.
    let r = (w.max(h) / 256).max(2);
    let fitted = guided_filter(&lum, &mask, w, h, r, 0.0008);
    fitted
        .into_iter()
        .map(|v| {
            let t = ((v - 0.3) / 0.4).clamp(0.0, 1.0);
            t * t * (3.0 - 2.0 * t)
        })
        .collect()
}

fn add_sky_behind_thin_things(sky: &mut [bool], rgb: &[[f32; 3]], lum: &[f32], grad: &[f32], edge: f32, w: usize, h: usize) {
    // Repeat: haze split by several wires is reached one piece at a time.
    for _ in 0..8 {
        let add = behind_thin_things_pass(sky, rgb, lum, grad, edge, w, h);
        if add.is_empty() {
            break;
        }
        for i in add {
            sky[i] = true;
        }
    }
}

/// One pass of `add_sky_behind_thin_things`: the pixels to add.
fn behind_thin_things_pass(sky: &[bool], rgb: &[[f32; 3]], lum: &[f32], grad: &[f32], edge: f32, w: usize, h: usize) -> Vec<usize> {
    let thin = (h / 40).max(10); // the most non-sky pixels to look across
    let min_area = (w * h) / 1000;
    let mut seen = vec![false; w * h];
    let step = 0.07f32;
    let mut add = Vec::new();
    for start in 0..w * h {
        if seen[start] || sky[start] || grad[start] >= edge {
            continue;
        }
        // One smooth region outside the sky found so far.
        seen[start] = true;
        let mut members = vec![start];
        let mut queue = VecDeque::from([start]);
        let mut sum = [0f32; 3];
        while let Some(i) = queue.pop_front() {
            for k in 0..3 {
                sum[k] += rgb[i][k];
            }
            let (x, y) = (i % w, i / w);
            let neighbours = [(x > 0).then(|| i - 1), (x + 1 < w).then(|| i + 1), (y > 0).then(|| i - w), (y + 1 < h).then(|| i + w)];
            for j in neighbours.into_iter().flatten() {
                if !seen[j] && !sky[j] && grad[j] < edge && dist(rgb[i], rgb[j]) < step {
                    seen[j] = true;
                    members.push(j);
                    queue.push_back(j);
                }
            }
        }
        if members.len() < min_area {
            continue;
        }
        let n = members.len() as f32;
        let mean = [sum[0] / n, sum[1] / n, sum[2] / n];
        if !looks_like_sky(mean) {
            continue;
        }
        let mean_lum = 0.2126 * mean[0] + 0.7152 * mean[1] + 0.0722 * mean[2];
        // The top of the region in each column, and the sky just above it.
        let mut top = vec![usize::MAX; w];
        for &i in &members {
            let (x, y) = (i % w, i / w);
            top[x] = top[x].min(y);
        }
        let (mut columns, mut found, mut sky_sum, mut sky_lum) = (0usize, 0usize, [0f32; 3], 0f32);
        for (x, &t) in top.iter().enumerate() {
            if t == usize::MAX {
                continue;
            }
            columns += 1;
            for d in 1..=thin {
                if t < d {
                    break;
                }
                let j = (t - d) * w + x;
                if sky[j] {
                    found += 1;
                    for k in 0..3 {
                        sky_sum[k] += rgb[j][k];
                    }
                    sky_lum += lum[j];
                    break;
                }
            }
        }
        if found == 0 || found * 10 < columns * 4 {
            continue;
        }
        let f = found as f32;
        let above = [sky_sum[0] / f, sky_sum[1] / f, sky_sum[2] / f];
        if dist(mean, above) < 0.25 && mean_lum >= sky_lum / f - 0.03 {
            add.extend(members);
        }
    }
    add
}

/// Fill cracks narrower than about 2r pixels inside the sky (dilate, then
/// erode, a binary mask).
fn close_cracks(mask: &mut [f32], w: usize, h: usize, r: usize) {
    let bin: Vec<f32> = mask.iter().map(|&v| if v >= 0.5 { 1.0 } else { 0.0 }).collect();
    let grown = extreme(&bin, w, h, r, true);
    let closed = extreme(&grown, w, h, r, false);
    for (m, c) in mask.iter_mut().zip(closed) {
        *m = m.max(c);
    }
}

/// Max (`max` true) or min over a (2r+1) square around each pixel.
fn extreme(v: &[f32], w: usize, h: usize, r: usize, max: bool) -> Vec<f32> {
    let pick = |a: f32, b: f32| if max { a.max(b) } else { a.min(b) };
    let mut tmp = vec![0.0; w * h];
    for y in 0..h {
        for x in 0..w {
            let (a, b) = (x.saturating_sub(r), (x + r).min(w - 1));
            tmp[y * w + x] = (a..=b).map(|k| v[y * w + k]).fold(if max { 0.0 } else { 1.0 }, pick);
        }
    }
    let mut out = vec![0.0; w * h];
    for y in 0..h {
        let (a, b) = (y.saturating_sub(r), (y + r).min(h - 1));
        for x in 0..w {
            out[y * w + x] = (a..=b).map(|k| tmp[k * w + x]).fold(if max { 0.0 } else { 1.0 }, pick);
        }
    }
    out
}

/// Mark as sky every patch of non-sky smaller than `max_area` pixels that
/// does not reach the left, right or bottom edge of the picture.
fn fill_small_holes(mask: &mut [f32], w: usize, h: usize, max_area: usize) {
    let mut seen = vec![false; w * h];
    for start in 0..w * h {
        if seen[start] || mask[start] >= 0.5 {
            continue;
        }
        let mut patch = vec![start];
        let mut queue = VecDeque::from([start]);
        seen[start] = true;
        let mut open_edge = false;
        while let Some(i) = queue.pop_front() {
            let (x, y) = (i % w, i / w);
            if x == 0 || x + 1 == w || y + 1 == h {
                open_edge = true;
            }
            let neighbours = [
                (x > 0).then(|| i - 1),
                (x + 1 < w).then(|| i + 1),
                (y > 0).then(|| i - w),
                (y + 1 < h).then(|| i + w),
            ];
            for j in neighbours.into_iter().flatten() {
                if !seen[j] && mask[j] < 0.5 {
                    seen[j] = true;
                    queue.push_back(j);
                    patch.push(j);
                }
            }
        }
        if !open_edge && patch.len() <= max_area {
            for i in patch {
                mask[i] = 1.0;
            }
        }
    }
}

/// Blue sky (blue clearly above red, at least as blue as green), or a
/// bright area (clouds, haze, overcast, sunset glow).
fn looks_like_sky(c: [f32; 3]) -> bool {
    let lum = 0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2];
    let blue = c[2] > c[0] + 0.05 && c[2] >= c[1] - 0.02;
    (blue && lum > 0.08) || lum > 0.45
}

fn dist(a: [f32; 3], b: [f32; 3]) -> f32 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
}

/// Edge strength (Sobel) of a single-channel picture.
fn gradient(v: &[f32], w: usize, h: usize) -> Vec<f32> {
    let at = |x: isize, y: isize| v[(y.clamp(0, h as isize - 1) as usize) * w + x.clamp(0, w as isize - 1) as usize];
    let mut out = vec![0.0; w * h];
    for y in 0..h as isize {
        for x in 0..w as isize {
            let gx = at(x + 1, y - 1) + 2.0 * at(x + 1, y) + at(x + 1, y + 1) - at(x - 1, y - 1) - 2.0 * at(x - 1, y) - at(x - 1, y + 1);
            let gy = at(x - 1, y + 1) + 2.0 * at(x, y + 1) + at(x + 1, y + 1) - at(x - 1, y - 1) - 2.0 * at(x, y - 1) - at(x + 1, y - 1);
            out[y as usize * w + x as usize] = (gx * gx + gy * gy).sqrt() / 4.0;
        }
    }
    out
}

/// Mean over a (2r+1) square around each pixel (edges repeat).
fn box_blur(v: &[f32], w: usize, h: usize, r: usize) -> Vec<f32> {
    let pass = |src: &[f32], horizontal: bool| -> Vec<f32> {
        let (len, lines) = if horizontal { (w, h) } else { (h, w) };
        let idx = |line: usize, k: usize| if horizontal { line * w + k } else { k * w + line };
        let mut out = vec![0.0; w * h];
        let n = (2 * r + 1) as f32;
        for line in 0..lines {
            let get = |k: isize| src[idx(line, k.clamp(0, len as isize - 1) as usize)];
            let mut sum: f32 = (-(r as isize)..=r as isize).map(get).sum();
            for k in 0..len {
                out[idx(line, k)] = sum / n;
                sum += get(k as isize + r as isize + 1) - get(k as isize - r as isize);
            }
        }
        out
    };
    pass(&pass(v, true), false)
}

/// Guided filter (He, Sun and Tang): smooths `p` while following the edges
/// of the guide `g`.
fn guided_filter(g: &[f32], p: &[f32], w: usize, h: usize, r: usize, eps: f32) -> Vec<f32> {
    let mul = |a: &[f32], b: &[f32]| a.iter().zip(b).map(|(x, y)| x * y).collect::<Vec<f32>>();
    let mean_g = box_blur(g, w, h, r);
    let mean_p = box_blur(p, w, h, r);
    let corr_g = box_blur(&mul(g, g), w, h, r);
    let corr_gp = box_blur(&mul(g, p), w, h, r);
    let mut a = vec![0.0; w * h];
    let mut b = vec![0.0; w * h];
    for i in 0..w * h {
        let var = corr_g[i] - mean_g[i] * mean_g[i];
        let cov = corr_gp[i] - mean_g[i] * mean_p[i];
        a[i] = cov / (var + eps);
        b[i] = mean_p[i] - a[i] * mean_g[i];
    }
    let mean_a = box_blur(&a, w, h, r);
    let mean_b = box_blur(&b, w, h, r);
    (0..w * h).map(|i| (mean_a[i] * g[i] + mean_b[i]).clamp(0.0, 1.0)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::Rgba;

    /// A sunset-like sky (blue at the top, pale and warm at the horizon at
    /// 75% of the height), dark ground below, and a dark tree with a gap
    /// of sky inside its crown.
    fn scene() -> RgbaImage {
        RgbaImage::from_fn(200, 120, |x, y| {
            let t = y as f32 / 90.0;
            let sky = Rgba([
                (90.0 + 150.0 * t) as u8,
                (140.0 + 80.0 * t) as u8,
                (230.0 - 40.0 * t) as u8,
                255,
            ]);
            let ground = Rgba([40, 55, 35, 255]);
            if y >= 90 {
                return ground;
            }
            // Tree: trunk and a crown (x 120..170, y 20..70) with a hole.
            let in_crown = (120..170).contains(&x) && (20..70).contains(&y);
            let in_hole = (140..150).contains(&x) && (38..48).contains(&y);
            let in_trunk = (140..150).contains(&x) && y >= 70;
            if (in_crown && !in_hole) || in_trunk { Rgba([25, 40, 25, 255]) } else { sky }
        })
    }

    #[test]
    fn sky_reaches_the_horizon_with_no_band() {
        let m = sky_mask(&scene());
        let at = |x: usize, y: usize| m[y * 200 + x];
        // Pale sky just above a low horizon is included.
        for y in [60, 80, 86] {
            assert!(at(40, y) > 0.9, "row {y}: {}", at(40, y));
        }
        // Ground is not.
        assert!(at(40, 100) < 0.05);
    }

    #[test]
    fn no_halo_around_objects_and_gaps_are_found() {
        let m = sky_mask(&scene());
        let at = |x: usize, y: usize| m[y * 200 + x];
        // Inside the tree, right next to its edge: not sky.
        assert!(at(122, 40) < 0.1, "{}", at(122, 40));
        assert!(at(145, 22) < 0.1, "{}", at(145, 22));
        // Sky right next to the tree: sky, no untouched ring.
        assert!(at(117, 40) > 0.9, "{}", at(117, 40));
        assert!(at(145, 17) > 0.9, "{}", at(145, 17));
        // The gap inside the crown is found too.
        assert!(at(145, 43) > 0.8, "{}", at(145, 43));
    }

    /// Plain deep-blue sky under a thin dark line along the top edge (as in
    /// phone screenshots), haze under a thin wire, and sea below the horizon.
    fn plain_sky_scene() -> RgbaImage {
        RgbaImage::from_fn(200, 150, |_, y| {
            let t = y as f32 / 105.0;
            if y < 2 {
                Rgba([45, 49, 54, 255]) // dark line along the top
            } else if y == 50 || y == 51 {
                Rgba([30, 30, 30, 255]) // a wire across the sky
            } else if y < 105 {
                // Deep blue, paler (hazier) lower down.
                Rgba([(25.0 + 120.0 * t) as u8, (60.0 + 110.0 * t) as u8, (150.0 + 60.0 * t) as u8, 255])
            } else {
                Rgba([20, 95, 110, 255]) // the sea, darker than the sky
            }
        })
    }

    #[test]
    fn plain_blue_sky_with_a_dark_top_edge_is_found() {
        let m = sky_mask(&plain_sky_scene());
        let at = |x: usize, y: usize| m[y * 200 + x];
        let covered = (5..100).filter(|&y| y != 50 && y != 51).filter(|&y| at(100, y) > 0.8).count();
        assert!(covered > 85, "only {covered} of 93 sky rows");
        // Haze below the wire, down to the horizon.
        assert!(at(30, 80) > 0.9 && at(170, 100) > 0.8, "{} {}", at(30, 80), at(170, 100));
        // The sea is not sky.
        assert!(at(100, 130) < 0.05, "{}", at(100, 130));
    }

    #[test]
    fn the_old_guess_had_the_band_this_fixes() {
        // The animation tool's guess fades out low in the picture.
        let old = crate::automask::guess_sky_and_water(&scene());
        assert!(old[86 * 200 + 40] < 0.5);
    }
}
