//! Layouts: several finished photos (each with its own colour edits, crop,
//! border and caption) placed together on one picture, from a template.
//! Each photo fills its box; it can be moved and zoomed inside the box.

use image::{Rgba, RgbaImage};

use crate::crop::Aspect;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Template {
    /// Two side by side.
    Side2,
    /// Two, one above the other.
    Stack2,
    /// Three in a row.
    Row3,
    /// One large on the left, two small stacked on the right.
    BigLeft3,
    /// Two by two.
    Grid4,
    /// Three across, two down.
    Grid6,
}

impl Template {
    pub const ALL: [Template; 6] =
        [Template::Side2, Template::Stack2, Template::Row3, Template::BigLeft3, Template::Grid4, Template::Grid6];

    pub fn label(self) -> &'static str {
        match self {
            Template::Side2 => "2 side by side",
            Template::Stack2 => "2 stacked",
            Template::Row3 => "3 in a row",
            Template::BigLeft3 => "1 large + 2",
            Template::Grid4 => "2 × 2 grid",
            Template::Grid6 => "3 × 2 grid",
        }
    }

    /// The template that suits `n` photos.
    pub fn for_count(n: usize) -> Self {
        match n {
            0..=2 => Template::Side2,
            3 => Template::BigLeft3,
            4 => Template::Grid4,
            _ => Template::Grid6,
        }
    }

    /// Boxes as fractions (x, y, w, h) of the area inside the margin,
    /// before gaps are taken out.
    fn units(self) -> Vec<[f32; 4]> {
        let grid = |cols: usize, rows: usize| {
            let mut v = Vec::new();
            for r in 0..rows {
                for c in 0..cols {
                    v.push([c as f32 / cols as f32, r as f32 / rows as f32, 1.0 / cols as f32, 1.0 / rows as f32]);
                }
            }
            v
        };
        match self {
            Template::Side2 => grid(2, 1),
            Template::Stack2 => grid(1, 2),
            Template::Row3 => grid(3, 1),
            Template::BigLeft3 => vec![[0.0, 0.0, 0.6, 1.0], [0.6, 0.0, 0.4, 0.5], [0.6, 0.5, 0.4, 0.5]],
            Template::Grid4 => grid(2, 2),
            Template::Grid6 => grid(3, 2),
        }
    }

    pub fn count(self) -> usize {
        self.units().len()
    }
}

/// One box of the layout: which file shows there and how it sits.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Cell {
    /// Scene id of the file shown, or none for an empty box.
    pub source: Option<u64>,
    /// 1 fills the box; below 1 shows more of the photo (down to all of it).
    pub zoom: f32,
    /// Point of the photo (0..1) at the middle of the box.
    pub center: [f32; 2],
}

impl Cell {
    pub fn new(source: Option<u64>) -> Self {
        Self { source, zoom: 1.0, center: [0.5, 0.5] }
    }
}

pub const ZOOM_MAX: f32 = 4.0;

#[derive(Clone, PartialEq, Debug)]
pub struct Layout {
    pub template: Template,
    /// Shape of the whole picture.
    pub aspect: Aspect,
    /// Space between boxes and around the edge, as fractions of the
    /// picture's shorter side.
    pub gap: f32,
    pub margin: f32,
    pub color: [u8; 3],
    pub cells: Vec<Cell>,
    /// Every file picked for this layout, in order (more than the boxes
    /// when a smaller template is chosen).
    pub files: Vec<u64>,
}

/// Shapes offered for the whole picture.
pub const SHAPES: [Aspect; 7] = [
    Aspect::Square,
    Aspect::Portrait45,
    Aspect::Portrait34,
    Aspect::Story,
    Aspect::Classic43,
    Aspect::Photo32,
    Aspect::Wide,
];

impl Layout {
    /// A layout for these files, in order.
    pub fn new(sources: &[u64]) -> Self {
        let template = Template::for_count(sources.len());
        let mut l = Self {
            template,
            aspect: Aspect::Square,
            gap: 0.02,
            margin: 0.03,
            color: [255, 255, 255],
            cells: Vec::new(),
            files: sources.to_vec(),
        };
        l.set_template(template);
        l
    }

    /// Change the template. Boxes keep their photos in order; new boxes
    /// get the layout's files not shown yet.
    pub fn set_template(&mut self, template: Template) {
        let mut cells: Vec<Cell> = self.cells.iter().copied().take(template.count()).collect();
        while cells.len() < template.count() {
            cells.push(Cell::new(None));
        }
        let used: Vec<u64> = cells.iter().filter_map(|c| c.source).collect();
        let mut unused = self.files.iter().copied().filter(|f| !used.contains(f));
        for c in cells.iter_mut() {
            if c.source.is_none() {
                c.source = unused.next();
            }
        }
        self.template = template;
        self.cells = cells;
    }

    /// Width ÷ height of the whole picture.
    pub fn ratio(&self) -> f32 {
        self.aspect.ratio(1, 1).unwrap_or(1.0)
    }

    /// Picture size for a longest side of `long` pixels.
    pub fn size(&self, long: u32) -> (u32, u32) {
        let r = self.ratio();
        if r >= 1.0 {
            (long, ((long as f32 / r).round() as u32).max(1))
        } else {
            (((long as f32 * r).round() as u32).max(1), long)
        }
    }

    /// The boxes in pixels (x, y, w, h) for a `w` x `h` picture.
    pub fn rects(&self, w: f32, h: f32) -> Vec<[f32; 4]> {
        let short = w.min(h);
        let m = self.margin.max(0.0) * short;
        let g = self.gap.max(0.0) * short;
        let (iw, ih) = ((w - 2.0 * m).max(1.0), (h - 2.0 * m).max(1.0));
        self.template
            .units()
            .iter()
            .map(|u| {
                // Half a gap on each inner side.
                let left = if u[0] > 1e-4 { g / 2.0 } else { 0.0 };
                let right = if u[0] + u[2] < 1.0 - 1e-4 { g / 2.0 } else { 0.0 };
                let top = if u[1] > 1e-4 { g / 2.0 } else { 0.0 };
                let bottom = if u[1] + u[3] < 1.0 - 1e-4 { g / 2.0 } else { 0.0 };
                [
                    m + u[0] * iw + left,
                    m + u[1] * ih + top,
                    (u[2] * iw - left - right).max(1.0),
                    (u[3] * ih - top - bottom).max(1.0),
                ]
            })
            .collect()
    }
}

/// Smallest zoom for a photo of `iw` x `ih` in a `cw` x `ch` box: the whole
/// photo shows.
pub fn min_zoom(iw: f32, ih: f32, cw: f32, ch: f32) -> f32 {
    let fill = (cw / iw).max(ch / ih);
    let fit = (cw / iw).min(ch / ih);
    (fit / fill).min(1.0)
}

/// Pixels of photo per pixel of box, for a cell.
pub fn cell_scale(cell: &Cell, iw: f32, ih: f32, cw: f32, ch: f32) -> f32 {
    (cw / iw).max(ch / ih) * cell.zoom
}

/// Keep the photo covering the box where it can: the middle point may only
/// move as far as the photo reaches.
pub fn clamp_center(cell: &mut Cell, iw: f32, ih: f32, cw: f32, ch: f32) {
    cell.zoom = cell.zoom.clamp(min_zoom(iw, ih, cw, ch), ZOOM_MAX);
    let s = cell_scale(cell, iw, ih, cw, ch);
    let half_w = cw / s / 2.0 / iw;
    let half_h = ch / s / 2.0 / ih;
    cell.center[0] = if half_w >= 0.5 { 0.5 } else { cell.center[0].clamp(half_w, 1.0 - half_w) };
    cell.center[1] = if half_h >= 0.5 { 0.5 } else { cell.center[1].clamp(half_h, 1.0 - half_h) };
}

/// Draw the layout at `w` x `h`. `photo` gives the finished picture of a
/// file by its scene id.
pub fn render<'a>(layout: &Layout, w: u32, h: u32, photo: &dyn Fn(u64) -> Option<&'a RgbaImage>) -> RgbaImage {
    let [r, g, b] = layout.color;
    let bg = Rgba([r, g, b, 255]);
    let mut out = RgbaImage::from_pixel(w, h, bg);
    for (cell, rect) in layout.cells.iter().zip(layout.rects(w as f32, h as f32)) {
        let Some(src) = cell.source.and_then(photo) else { continue };
        draw_cell(&mut out, rect, src, cell, bg);
    }
    out
}

/// Paint one photo into its box (smooth sampling; outside the photo shows
/// the background).
fn draw_cell(out: &mut RgbaImage, rect: [f32; 4], src: &RgbaImage, cell: &Cell, bg: Rgba<u8>) {
    let (iw, ih) = (src.width() as f32, src.height() as f32);
    if iw < 2.0 || ih < 2.0 {
        return;
    }
    let [rx, ry, rw, rh] = rect;
    let mut c = *cell;
    clamp_center(&mut c, iw, ih, rw, rh);
    let s = cell_scale(&c, iw, ih, rw, rh);
    let (cx, cy) = (c.center[0] * iw, c.center[1] * ih);
    let (ow, oh) = (out.width() as i64, out.height() as i64);
    let x0 = (rx.round() as i64).clamp(0, ow);
    let x1 = ((rx + rw).round() as i64).clamp(0, ow);
    let y0 = (ry.round() as i64).clamp(0, oh);
    let y1 = ((ry + rh).round() as i64).clamp(0, oh);
    let raw = src.as_raw();
    let stride = src.width() as usize * 4;
    let w_out = out.width() as usize;
    let data: &mut [u8] = out;
    crate::render::par_chunks(&mut data[(y0 as usize * w_out * 4)..(y1 as usize * w_out * 4)], w_out * 4, |row_i, row| {
        let y = y0 as f32 + row_i as f32 + 0.5;
        let sy = cy + (y - (ry + rh / 2.0)) / s - 0.5;
        for x in x0..x1 {
            let xf = x as f32 + 0.5;
            let sx = cx + (xf - (rx + rw / 2.0)) / s - 0.5;
            let px = &mut row[x as usize * 4..x as usize * 4 + 4];
            if sx < -0.5 || sy < -0.5 || sx > iw - 0.5 || sy > ih - 0.5 {
                px.copy_from_slice(&bg.0);
                continue;
            }
            let (sx, sy) = (sx.clamp(0.0, iw - 1.0), sy.clamp(0.0, ih - 1.0));
            let (xa, ya) = (sx.floor() as usize, sy.floor() as usize);
            let (xb, yb) = ((xa + 1).min(iw as usize - 1), (ya + 1).min(ih as usize - 1));
            let (fx, fy) = (sx - xa as f32, sy - ya as f32);
            for k in 0..3 {
                let at = |xx: usize, yy: usize| raw[yy * stride + xx * 4 + k] as f32;
                let top = at(xa, ya) * (1.0 - fx) + at(xb, ya) * fx;
                let bottom = at(xa, yb) * (1.0 - fx) + at(xb, yb) * fx;
                px[k] = (top * (1.0 - fy) + bottom * fy).round() as u8;
            }
            px[3] = 255;
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn boxes_fill_the_inside_with_gaps() {
        let mut l = Layout::new(&[1, 2]);
        l.margin = 0.1;
        l.gap = 0.1;
        let r = l.rects(100.0, 100.0);
        assert_eq!(r.len(), 2);
        // 10 px margin, 10 px gap: boxes 35 wide.
        assert!((r[0][0] - 10.0).abs() < 1e-3 && (r[0][2] - 35.0).abs() < 1e-3);
        assert!((r[1][0] - 55.0).abs() < 1e-3);
        assert!((r[0][3] - 80.0).abs() < 1e-3);
    }

    #[test]
    fn photo_fills_its_box_and_background_shows_in_the_gaps() {
        let red = RgbaImage::from_pixel(300, 200, Rgba([255, 0, 0, 255]));
        let blue = RgbaImage::from_pixel(100, 400, Rgba([0, 0, 255, 255]));
        let mut l = Layout::new(&[1, 2]);
        l.color = [0, 255, 0];
        let out = render(&l, 200, 100, &|id| match id {
            1 => Some(&red),
            2 => Some(&blue),
            _ => None,
        });
        let r = l.rects(200.0, 100.0);
        let mid = |i: usize| out.get_pixel((r[i][0] + r[i][2] / 2.0) as u32, (r[i][1] + r[i][3] / 2.0) as u32).0;
        assert_eq!(mid(0), [255, 0, 0, 255]);
        assert_eq!(mid(1), [0, 0, 255, 255]);
        // Corners of a filled box are photo, not background.
        assert_eq!(out.get_pixel(r[1][0] as u32 + 1, r[1][1] as u32 + 1).0, [0, 0, 255, 255]);
        assert_eq!(out.get_pixel(0, 0).0, [0, 255, 0, 255]);
    }

    #[test]
    fn zoom_and_center_are_kept_in_range() {
        let mut c = Cell::new(Some(1));
        c.zoom = 0.1;
        c.center = [0.0, 0.9];
        clamp_center(&mut c, 400.0, 200.0, 100.0, 100.0);
        // Whole photo shows: zoom 0.5, nothing to move.
        assert!((c.zoom - 0.5).abs() < 1e-5);
        assert_eq!(c.center, [0.5, 0.5]);
        c.zoom = 1.0;
        c.center = [0.0, 0.5];
        clamp_center(&mut c, 400.0, 200.0, 100.0, 100.0);
        assert!((c.center[0] - 0.25).abs() < 1e-5);
    }

    #[test]
    fn template_change_keeps_photo_order() {
        let mut l = Layout::new(&[1, 2, 3]);
        l.set_template(Template::Side2);
        assert_eq!(l.cells.iter().map(|c| c.source).collect::<Vec<_>>(), vec![Some(1), Some(2)]);
        // Going back to more boxes brings the third photo back; the fourth box is empty.
        l.set_template(Template::Grid4);
        assert_eq!(l.cells[2].source, Some(3));
        assert_eq!(l.cells[3].source, None);
    }
}
