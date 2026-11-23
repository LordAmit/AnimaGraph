//! Composition guides drawn over the crop: lines as fractions (0..1) of the
//! crop, so they follow it at any size.

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Guide {
    Off,
    Thirds,
    GoldenSection,
    GoldenTriangle,
    SpiralSection,
    GoldenSpiral,
    HarmoniousTriangles,
    Diagonals,
    VArrangement,
    CompoundCurve,
    Pyramid,
    Circular,
    Center,
    Grid,
}

type Line = Vec<[f32; 2]>;

impl Guide {
    pub const ALL: [Guide; 14] = [
        Guide::Thirds,
        Guide::GoldenSection,
        Guide::GoldenTriangle,
        Guide::SpiralSection,
        Guide::GoldenSpiral,
        Guide::HarmoniousTriangles,
        Guide::Diagonals,
        Guide::VArrangement,
        Guide::CompoundCurve,
        Guide::Pyramid,
        Guide::Circular,
        Guide::Center,
        Guide::Grid,
        Guide::Off,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Guide::Off => "None",
            Guide::Thirds => "Thirds",
            Guide::GoldenSection => "Golden section",
            Guide::GoldenTriangle => "Golden triangle",
            Guide::SpiralSection => "Spiral section",
            Guide::GoldenSpiral => "Golden spiral",
            Guide::HarmoniousTriangles => "Harmonious triangles",
            Guide::Diagonals => "Diagonals",
            Guide::VArrangement => "V arrangement",
            Guide::CompoundCurve => "Compound curve",
            Guide::Pyramid => "Pyramid",
            Guide::Circular => "Circular",
            Guide::Center => "Center",
            Guide::Grid => "Grid",
        }
    }

    pub fn hint(self) -> &'static str {
        match self {
            Guide::Off => "No guide lines",
            Guide::Thirds => "Rule of thirds: place the subject on a line or a crossing",
            Guide::GoldenSection => "Lines at the golden ratio (1 : 1.618): like thirds, closer to the middle",
            Guide::GoldenTriangle => "A diagonal, and lines from the other two corners that meet it at a right angle. Click again to flip",
            Guide::SpiralSection => "The squares the golden spiral runs through. Click again to flip",
            Guide::GoldenSpiral => "Lead the eye along the curve to its inner end. Click again to flip",
            Guide::HarmoniousTriangles => "A diagonal, and lines at a right angle to it that run from edge to edge: four similar triangles. Click again to flip",
            Guide::Diagonals => "Corner to corner",
            Guide::VArrangement => "Two lines that meet at the middle of the bottom edge, as in a valley",
            Guide::CompoundCurve => "An S curve, for roads, rivers and poses. Click again to flip",
            Guide::Pyramid => "A triangle on a wide base, for steady subjects and groups",
            Guide::Circular => "A circle around the middle, for round or enclosed subjects",
            Guide::Center => "Middle lines, for symmetry",
            Guide::Grid => "Even grid, for horizons and walls",
        }
    }

    /// How many different ways the guide can be mirrored (1 = none).
    pub fn flips(self) -> u8 {
        match self {
            Guide::GoldenSpiral | Guide::SpiralSection => 4,
            Guide::GoldenTriangle | Guide::HarmoniousTriangles | Guide::CompoundCurve => 2,
            _ => 1,
        }
    }

    /// The guide's lines for a crop of `aspect` (width ÷ height). `flip`
    /// 0 to 3 mirrors left-right (1), top-bottom (2) or both (3).
    pub fn lines(self, aspect: f32, flip: u8) -> Vec<Line> {
        let across = |at: &[f32]| -> Vec<Line> {
            at.iter().flat_map(|&t| [vec![[t, 0.0], [t, 1.0]], vec![[0.0, t], [1.0, t]]]).collect()
        };
        let mut lines = match self {
            Guide::Off => Vec::new(),
            Guide::Thirds => across(&[1.0 / 3.0, 2.0 / 3.0]),
            Guide::GoldenSection => across(&[1.0 - 1.0 / PHI, 1.0 / PHI]),
            Guide::Center => across(&[0.5]),
            Guide::Grid => across(&[1.0 / 6.0, 2.0 / 6.0, 0.5, 4.0 / 6.0, 5.0 / 6.0]),
            Guide::Diagonals => vec![vec![[0.0, 0.0], [1.0, 1.0]], vec![[1.0, 0.0], [0.0, 1.0]]],
            Guide::GoldenTriangle => {
                let a2 = aspect * aspect;
                let (far, near) = (a2 / (a2 + 1.0), 1.0 / (a2 + 1.0));
                vec![vec![[0.0, 0.0], [1.0, 1.0]], vec![[1.0, 0.0], [far, far]], vec![[0.0, 1.0], [near, near]]]
            }
            Guide::HarmoniousTriangles => {
                // Where the right-angled lines reach the opposite edge: the
                // long edge, so top and bottom, or left and right when tall.
                let a2 = aspect * aspect;
                let (p, q) = if aspect >= 1.0 {
                    ([1.0 - 1.0 / a2, 1.0], [1.0 / a2, 0.0])
                } else {
                    ([0.0, a2], [1.0, 1.0 - a2])
                };
                vec![vec![[0.0, 0.0], [1.0, 1.0]], vec![[1.0, 0.0], p], vec![[0.0, 1.0], q]]
            }
            Guide::SpiralSection => spiral_section(aspect < 1.0),
            Guide::GoldenSpiral => vec![spiral(aspect < 1.0)],
            Guide::VArrangement => vec![vec![[0.0, 0.0], [0.5, 1.0], [1.0, 0.0]]],
            Guide::Pyramid => vec![vec![[0.0, 1.0], [0.5, 0.0], [1.0, 1.0], [0.0, 1.0]]],
            Guide::CompoundCurve => vec![compound_curve()],
            Guide::Circular => {
                let (rx, ry) = (0.5 * (1.0 / aspect).min(1.0), 0.5 * aspect.min(1.0));
                let point = |i: usize| {
                    let a = i as f32 / 96.0 * std::f32::consts::TAU;
                    [0.5 + rx * a.cos(), 0.5 + ry * a.sin()]
                };
                vec![(0..=96).map(point).collect()]
            }
        };
        if self.flips() > 1 {
            for p in lines.iter_mut().flatten() {
                if flip & 1 != 0 {
                    p[0] = 1.0 - p[0];
                }
                if flip & 2 != 0 {
                    p[1] = 1.0 - p[1];
                }
            }
        }
        lines
    }
}

const PHI: f32 = 1.618_034;

/// One square cut off a golden rectangle: its corner and side, and the
/// rectangle (x, y, w, h) that is left.
struct Cut {
    square: [f32; 3],
    rest: [f32; 4],
}

/// Squares cut off in turn at the left, top, right and bottom of a golden
/// rectangle (PHI wide, 1 high).
fn cuts(n: usize) -> Vec<Cut> {
    let (mut x, mut y, mut w, mut h) = (0.0f32, 0.0f32, PHI, 1.0f32);
    (0..n)
        .map(|turn| {
            let s = if turn % 2 == 0 { h } else { w };
            let corner = match turn % 4 {
                0 | 1 => [x, y],
                2 => [x + w - s, y],
                _ => [x, y + h - s],
            };
            match turn % 4 {
                0 => (x, w) = (x + s, w - s),
                1 => (y, h) = (y + s, h - s),
                2 => w -= s,
                _ => h -= s,
            }
            Cut { square: [corner[0], corner[1], s], rest: [x, y, w, h] }
        })
        .collect()
}

/// From the golden rectangle to the crop (stretched; on its side when tall).
fn to_crop(p: [f32; 2], tall: bool) -> [f32; 2] {
    let p = [p[0] / PHI, p[1]];
    if tall { [p[1], p[0]] } else { p }
}

/// Quarter circles through the squares.
fn spiral(tall: bool) -> Line {
    let mut points = Vec::new();
    for (turn, Cut { square: [x, y, s], .. }) in cuts(9).into_iter().enumerate() {
        // Centre of the quarter circle and the angle it starts at.
        let (cx, cy, start) = match turn % 4 {
            0 => (x + s, y + s, 180.0f32),
            1 => (x, y + s, 270.0),
            2 => (x, y, 0.0),
            _ => (x + s, y, 90.0),
        };
        for i in 0..=16 {
            let a = (start + 90.0 * i as f32 / 16.0).to_radians();
            points.push(to_crop([cx + s * a.cos(), cy + s * a.sin()], tall));
        }
    }
    points
}

/// The line between each square and the rectangle that is left.
fn spiral_section(tall: bool) -> Vec<Line> {
    cuts(8)
        .into_iter()
        .enumerate()
        .map(|(turn, Cut { rest: [x, y, w, h], .. })| {
            let (a, b) = match turn % 4 {
                0 => ([x, y], [x, y + h]),
                1 => ([x, y], [x + w, y]),
                2 => ([x + w, y], [x + w, y + h]),
                _ => ([x, y + h], [x + w, y + h]),
            };
            vec![to_crop(a, tall), to_crop(b, tall)]
        })
        .collect()
}

/// Two opposite bends joined smoothly at the middle.
fn compound_curve() -> Line {
    let bend = |from: [f32; 2], pull: [f32; 2], to: [f32; 2]| {
        (0..=24).map(move |i| {
            let t = i as f32 / 24.0;
            let (a, b, c) = ((1.0 - t) * (1.0 - t), 2.0 * t * (1.0 - t), t * t);
            [a * from[0] + b * pull[0] + c * to[0], a * from[1] + b * pull[1] + c * to[1]]
        })
    };
    bend([1.0, 0.0], [0.0, 0.0], [0.5, 0.5]).chain(bend([0.5, 0.5], [1.0, 1.0], [0.0, 1.0])).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lines_stay_inside_the_crop() {
        for g in Guide::ALL {
            for aspect in [0.56, 1.0, 1.78] {
                for flip in 0..4 {
                    for p in g.lines(aspect, flip).iter().flatten() {
                        assert!(p.iter().all(|v| (-1e-3..=1.001).contains(v)), "{g:?} {p:?}");
                    }
                }
            }
        }
    }

    #[test]
    fn the_spiral_is_one_unbroken_line_to_its_centre() {
        let s = &Guide::GoldenSpiral.lines(1.6, 0)[0];
        for pair in s.windows(2) {
            assert!((pair[0][0] - pair[1][0]).hypot(pair[0][1] - pair[1][1]) < 0.16, "{pair:?}");
        }
        assert!((s[0][0]).abs() < 1e-3 && (s[0][1] - 1.0).abs() < 1e-3);
        let end = s[s.len() - 1];
        assert!((end[0] - 0.724).abs() < 0.03 && (end[1] - 0.724).abs() < 0.03, "{end:?}");
    }

    #[test]
    fn the_spiral_passes_through_its_sections() {
        // Every section line has the spiral touching one of its ends.
        for (tall, aspect) in [(false, 1.5), (true, 0.7)] {
            let s = &Guide::GoldenSpiral.lines(aspect, 0)[0];
            for l in spiral_section(tall) {
                let touches = |e: [f32; 2]| s.iter().any(|p| (p[0] - e[0]).hypot(p[1] - e[1]) < 1e-3);
                assert!(touches(l[0]) || touches(l[1]), "{l:?}");
            }
        }
    }

    #[test]
    fn triangles_meet_the_diagonal_at_a_right_angle() {
        for g in [Guide::GoldenTriangle, Guide::HarmoniousTriangles] {
            for a in [0.6, 1.0, 1.5] {
                for l in &g.lines(a, 0)[1..] {
                    let v = [(l[1][0] - l[0][0]) * a, l[1][1] - l[0][1]];
                    assert!((v[0] * a + v[1]).abs() < 1e-4, "{g:?} {a}");
                }
            }
        }
    }

    #[test]
    fn the_circle_is_round() {
        for a in [0.6, 1.0, 1.5] {
            for p in &Guide::Circular.lines(a, 0)[0] {
                let r = ((p[0] - 0.5) * a).hypot(p[1] - 0.5);
                assert!((r - 0.5 * f32::min(a, 1.0)).abs() < 1e-4);
            }
        }
    }

    #[test]
    fn every_flip_looks_different() {
        let set = |g: Guide, flip| {
            let mut v: Vec<(i32, i32)> =
                g.lines(1.5, flip).iter().flatten().map(|p| ((p[0] * 1e3).round() as i32, (p[1] * 1e3).round() as i32)).collect();
            v.sort();
            v
        };
        for g in Guide::ALL {
            for i in 0..g.flips() {
                for j in 0..i {
                    assert_ne!(set(g, i), set(g, j), "{g:?}");
                }
            }
        }
    }
}
