//! The editable project: the photo, the painted motion region and the
//! motion hints (arrows and pins). Positions are stored in 0..1 image
//! coordinates so the same hints work at any render size.

use image::RgbaImage;

/// Longest side of the editing copy of the photo.
pub const WORK_SIZE: u32 = 720;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Arrow {
    pub from: [f32; 2],
    pub to: [f32; 2],
}

#[derive(Clone)]
pub struct Snapshot {
    mask: Vec<f32>,
    arrows: Vec<Arrow>,
    pins: Vec<[f32; 2]>,
    crop: crate::crop::CropRect,
    aspect: crate::crop::Aspect,
}

/// One undo step: a saved state, or a quarter turn (undone by turning back).
#[derive(Clone)]
enum Step {
    State(Snapshot),
    Turn { clockwise: bool },
}

/// Edit settings kept per file (crop and rotation live on the scene itself).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Edits {
    pub look: crate::finish::Look,
    pub border: crate::border::Border,
    pub caption: crate::caption::Caption,
}

static NEXT_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

pub struct Scene {
    /// Unique per opened file, so loops can follow its crop.
    pub id: u64,
    pub name: String,
    /// Part of the picture kept when saving, and its locked shape.
    pub crop: crate::crop::CropRect,
    pub aspect: crate::crop::Aspect,
    /// The crop was confirmed: the canvas shows only the kept part.
    pub crop_applied: bool,
    /// This file's look, border and caption.
    pub edits: Edits,
    /// The photo at its original resolution.
    pub full: RgbaImage,
    /// A smaller copy used for editing and previews.
    pub work: RgbaImage,
    /// Motion region at work resolution, 0 = still, 1 = moving.
    pub mask: Vec<f32>,
    pub arrows: Vec<Arrow>,
    pub pins: Vec<[f32; 2]>,
    pub mask_version: u64,
    /// Set when the scene was opened from a video clip.
    pub video: Option<std::sync::Arc<crate::video::VideoSource>>,
    pub still_index: usize,
    /// Video trim: first and last frame used (inclusive).
    pub trim: [usize; 2],
    undo: Vec<Step>,
    redo: Vec<Step>,
}

impl Scene {
    pub fn new(name: String, full: RgbaImage) -> Self {
        let work = downscale(&full, WORK_SIZE);
        let n = (work.width() * work.height()) as usize;
        Self {
            id: NEXT_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
            name,
            crop: crate::crop::CropRect::FULL,
            aspect: crate::crop::Aspect::Free,
            crop_applied: false,
            edits: Edits::default(),
            full,
            work,
            mask: vec![0.0; n],
            arrows: Vec::new(),
            pins: Vec::new(),
            mask_version: 1,
            video: None,
            still_index: 0,
            trim: [0, 0],
            undo: Vec::new(),
            redo: Vec::new(),
        }
    }

    /// A scene from a clip; the frozen frame starts as the first frame.
    pub fn from_video(name: String, video: crate::video::VideoSource) -> Self {
        let mut s = Self::new(name, video.frames[0].clone());
        s.work = video.work[0].clone();
        s.mask = vec![0.0; (s.work.width() * s.work.height()) as usize];
        s.trim = [0, video.frames.len() - 1];
        s.video = Some(std::sync::Arc::new(video));
        s
    }

    /// Choose which video frame is frozen.
    pub fn set_still(&mut self, index: usize) {
        if let Some(v) = &self.video {
            let i = index.min(v.frames.len() - 1);
            self.still_index = i;
            self.full = v.frames[i].clone();
            self.work = v.work[i].clone();
        }
    }

    pub fn is_video(&self) -> bool {
        self.video.is_some()
    }

    /// Fill (or clear) a rectangle given by two 0..1 corners.
    pub fn fill_rect(&mut self, a: [f32; 2], b: [f32; 2], value: f32, feather: f32) {
        let [w, h] = self.work_size();
        let x0 = (a[0].min(b[0]).clamp(0.0, 1.0) * w as f32) as i32;
        let x1 = (a[0].max(b[0]).clamp(0.0, 1.0) * w as f32) as i32;
        let y0 = (a[1].min(b[1]).clamp(0.0, 1.0) * h as f32) as i32;
        let y1 = (a[1].max(b[1]).clamp(0.0, 1.0) * h as f32) as i32;
        let f = feather.max(0.0);
        let pad = f.ceil() as i32;
        for y in (y0 - pad).max(0)..(y1 + pad).min(h as i32) {
            for x in (x0 - pad).max(0)..(x1 + pad).min(w as i32) {
                // Distance outside the box, 0 inside.
                let dx = (x0 - x).max(x - (x1 - 1)).max(0) as f32;
                let dy = (y0 - y).max(y - (y1 - 1)).max(0) as f32;
                let d = (dx * dx + dy * dy).sqrt();
                let a = if f <= 0.0 { if d == 0.0 { 1.0 } else { 0.0 } } else { (1.0 - d / f).clamp(0.0, 1.0) };
                if a <= 0.0 {
                    continue;
                }
                let m = &mut self.mask[(y as u32 * w + x as u32) as usize];
                if value > 0.5 {
                    *m = m.max(a);
                } else {
                    *m = m.min(1.0 - a);
                }
            }
        }
        self.mask_version += 1;
    }

    pub fn work_size(&self) -> [u32; 2] {
        [self.work.width(), self.work.height()]
    }

    /// Turn the picture a quarter turn (`clockwise` = right). The painted
    /// area, arrows, pins and crop turn with it. Undo turns it back.
    pub fn rotate(&mut self, clockwise: bool) {
        self.turn(clockwise);
        self.push_undo(Step::Turn { clockwise });
        self.redo.clear();
    }

    fn turn(&mut self, clockwise: bool) {
        let turn = |img: &RgbaImage| {
            if clockwise {
                image::imageops::rotate90(img)
            } else {
                image::imageops::rotate270(img)
            }
        };
        // A point in 0..1 coordinates after the turn.
        let point = |p: [f32; 2]| if clockwise { [1.0 - p[1], p[0]] } else { [p[1], 1.0 - p[0]] };

        let [w, h] = self.work_size();
        let mut mask = vec![0.0f32; self.mask.len()];
        for y in 0..h {
            for x in 0..w {
                let (nx, ny) = if clockwise { (h - 1 - y, x) } else { (y, w - 1 - x) };
                mask[(ny * h + nx) as usize] = self.mask[(y * w + x) as usize];
            }
        }
        self.mask = mask;
        self.full = turn(&self.full);
        self.work = turn(&self.work);
        for a in &mut self.arrows {
            a.from = point(a.from);
            a.to = point(a.to);
        }
        for p in &mut self.pins {
            *p = point(*p);
        }
        let c = self.crop;
        self.crop = crate::crop::CropRect::from_corners(point([c.x, c.y]), point([c.x + c.w, c.y + c.h]));
        use crate::crop::Aspect;
        if !matches!(self.aspect, Aspect::Free | Aspect::Original | Aspect::Square) {
            self.aspect = Aspect::Free;
        }
        if let Some(v) = &self.video {
            let frames: Vec<RgbaImage> = v.frames.iter().map(turn).collect();
            let work: Vec<RgbaImage> = v.work.iter().map(turn).collect();
            let log_frames = v.log_frames.as_ref().map(|l| std::sync::Arc::new(l.iter().map(turn).collect()));
            self.video = Some(std::sync::Arc::new(crate::video::VideoSource {
                frames,
                work,
                fps: v.fps,
                apple_log: v.apple_log,
                log_frames,
                log_converted: v.log_converted,
            }));
        }
        self.mask_version += 1;
    }

    /// Store the current state so the next edit can be undone.
    pub fn checkpoint(&mut self) {
        let s = self.snapshot();
        self.push_undo(Step::State(s));
        self.redo.clear();
    }

    /// Forget the last saved step (an edit that turned out to change nothing).
    pub fn discard_checkpoint(&mut self) {
        self.undo.pop();
    }

    fn push_undo(&mut self, step: Step) {
        self.undo.push(step);
        if self.undo.len() > 60 {
            self.undo.remove(0);
        }
    }

    fn snapshot(&self) -> Snapshot {
        Snapshot {
            mask: self.mask.clone(),
            arrows: self.arrows.clone(),
            pins: self.pins.clone(),
            crop: self.crop,
            aspect: self.aspect,
        }
    }

    fn restore(&mut self, s: Snapshot) {
        self.mask = s.mask;
        self.arrows = s.arrows;
        self.pins = s.pins;
        if s.crop != self.crop {
            // A crop that comes back is shown confirmed; no crop shows everything.
            self.crop_applied = !s.crop.is_full();
        }
        self.crop = s.crop;
        self.aspect = s.aspect;
        self.mask_version += 1;
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    /// Undo one step. Returns true when the picture was turned, so the
    /// caller can redraw everything that shows the picture.
    pub fn undo(&mut self) -> bool {
        match self.undo.pop() {
            Some(Step::State(s)) => {
                let now = self.snapshot();
                self.redo.push(Step::State(now));
                self.restore(s);
                false
            }
            Some(Step::Turn { clockwise }) => {
                self.turn(!clockwise);
                self.redo.push(Step::Turn { clockwise });
                true
            }
            None => false,
        }
    }

    /// Redo one step; returns true when the picture was turned.
    pub fn redo(&mut self) -> bool {
        match self.redo.pop() {
            Some(Step::State(s)) => {
                let now = self.snapshot();
                self.push_undo(Step::State(now));
                self.restore(s);
                false
            }
            Some(Step::Turn { clockwise }) => {
                self.turn(clockwise);
                self.push_undo(Step::Turn { clockwise });
                true
            }
            None => false,
        }
    }

    /// Paint (or erase) a soft round dab at a 0..1 position.
    /// `radius` is in work pixels, `hardness` in 0..1.
    pub fn dab(&mut self, at: [f32; 2], radius: f32, hardness: f32, erase: bool) {
        let [w, h] = self.work_size();
        let cx = at[0] * w as f32;
        let cy = at[1] * h as f32;
        let x0 = (cx - radius).floor().max(0.0) as u32;
        let y0 = (cy - radius).floor().max(0.0) as u32;
        let x1 = ((cx + radius).ceil() as u32).min(w.saturating_sub(1));
        let y1 = ((cy + radius).ceil() as u32).min(h.saturating_sub(1));
        let inner = radius * hardness.clamp(0.0, 0.98);
        for y in y0..=y1 {
            for x in x0..=x1 {
                let d = ((x as f32 + 0.5 - cx).powi(2) + (y as f32 + 0.5 - cy).powi(2)).sqrt();
                if d > radius {
                    continue;
                }
                let a = if d <= inner {
                    1.0
                } else {
                    let t = (radius - d) / (radius - inner).max(1e-3);
                    t * t * (3.0 - 2.0 * t)
                };
                let i = (y * w + x) as usize;
                let m = &mut self.mask[i];
                if erase {
                    *m = m.min(1.0 - a);
                } else {
                    *m = m.max(a);
                }
            }
        }
        self.mask_version += 1;
    }

    pub fn fill_mask(&mut self, value: f32) {
        self.mask.iter_mut().for_each(|m| *m = value);
        self.mask_version += 1;
    }

    pub fn invert_mask(&mut self) {
        self.mask.iter_mut().for_each(|m| *m = 1.0 - *m);
        self.mask_version += 1;
    }

    pub fn set_mask(&mut self, mask: Vec<f32>) {
        self.mask = mask;
        self.mask_version += 1;
    }

    pub fn mask_coverage(&self) -> f32 {
        if self.mask.is_empty() {
            return 0.0;
        }
        self.mask.iter().sum::<f32>() / self.mask.len() as f32
    }

    /// Mask value at a 0..1 position (nearest pixel).
    pub fn mask_at(&self, p: [f32; 2]) -> f32 {
        let [w, h] = self.work_size();
        let x = ((p[0] * w as f32) as u32).min(w - 1);
        let y = ((p[1] * h as f32) as u32).min(h - 1);
        self.mask[(y * w + x) as usize]
    }

    /// Remove the arrow or pin nearest to `p` within `max_dist` (0..1 units).
    pub fn remove_hint_near(&mut self, p: [f32; 2], max_dist: f32) -> bool {
        let mut best: Option<(f32, bool, usize)> = None;
        for (i, a) in self.arrows.iter().enumerate() {
            let d = crate::motion::dist_to_segment(p, a.from, a.to);
            if d < max_dist && best.is_none_or(|b| d < b.0) {
                best = Some((d, true, i));
            }
        }
        for (i, q) in self.pins.iter().enumerate() {
            let d = ((p[0] - q[0]).powi(2) + (p[1] - q[1]).powi(2)).sqrt();
            if d < max_dist && best.is_none_or(|b| d < b.0) {
                best = Some((d, false, i));
            }
        }
        match best {
            Some((_, true, i)) => {
                self.arrows.remove(i);
                true
            }
            Some((_, false, i)) => {
                self.pins.remove(i);
                true
            }
            None => false,
        }
    }
}

/// Resize so the longest side is at most `max_side` (never upscales).
pub fn downscale(img: &RgbaImage, max_side: u32) -> RgbaImage {
    let (w, h) = img.dimensions();
    let longest = w.max(h);
    if longest <= max_side {
        return img.clone();
    }
    let s = max_side as f32 / longest as f32;
    let nw = ((w as f32 * s).round() as u32).max(1);
    let nh = ((h as f32 * s).round() as u32).max(1);
    image::imageops::resize(img, nw, nh, image::imageops::FilterType::Triangle)
}

/// Resample a mask to a new size with bilinear filtering.
pub fn resize_mask(mask: &[f32], src: [u32; 2], dst: [u32; 2]) -> Vec<f32> {
    let [sw, sh] = src;
    let [dw, dh] = dst;
    let mut out = vec![0.0; (dw * dh) as usize];
    for y in 0..dh {
        let fy = ((y as f32 + 0.5) * sh as f32 / dh as f32 - 0.5).clamp(0.0, (sh - 1) as f32);
        let y0 = fy.floor() as u32;
        let y1 = (y0 + 1).min(sh - 1);
        let ty = fy - y0 as f32;
        for x in 0..dw {
            let fx = ((x as f32 + 0.5) * sw as f32 / dw as f32 - 0.5).clamp(0.0, (sw - 1) as f32);
            let x0 = fx.floor() as u32;
            let x1 = (x0 + 1).min(sw - 1);
            let tx = fx - x0 as f32;
            let g = |xx: u32, yy: u32| mask[(yy * sw + xx) as usize];
            let top = g(x0, y0) * (1.0 - tx) + g(x1, y0) * tx;
            let bot = g(x0, y1) * (1.0 - tx) + g(x1, y1) * tx;
            out[(y * dw + x) as usize] = top * (1.0 - ty) + bot * ty;
        }
    }
    out
}

/// Separable box blur, used to feather mask edges.
pub fn blur_mask(mask: &[f32], w: u32, h: u32, radius: u32) -> Vec<f32> {
    if radius == 0 {
        return mask.to_vec();
    }
    let (w, h, r) = (w as i32, h as i32, radius as i32);
    let mut tmp = vec![0.0f32; mask.len()];
    let mut out = vec![0.0f32; mask.len()];
    let norm = 1.0 / (2 * r + 1) as f32;
    for y in 0..h {
        let row = (y * w) as usize;
        let mut acc = 0.0;
        for k in -r..=r {
            acc += mask[row + k.clamp(0, w - 1) as usize];
        }
        for x in 0..w {
            tmp[row + x as usize] = acc * norm;
            let add = (x + r + 1).clamp(0, w - 1);
            let sub = (x - r).clamp(0, w - 1);
            acc += mask[row + add as usize] - mask[row + sub as usize];
        }
    }
    for x in 0..w {
        let mut acc = 0.0;
        for k in -r..=r {
            acc += tmp[(k.clamp(0, h - 1) * w + x) as usize];
        }
        for y in 0..h {
            out[(y * w + x) as usize] = acc * norm;
            let add = (y + r + 1).clamp(0, h - 1);
            let sub = (y - r).clamp(0, h - 1);
            acc += tmp[(add * w + x) as usize] - tmp[(sub * w + x) as usize];
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rotating_turns_picture_mask_and_hints_together() {
        let mut img = RgbaImage::new(40, 20);
        img.put_pixel(0, 0, image::Rgba([255, 0, 0, 255])); // red top-left
        let mut s = Scene::new("t".into(), img);
        s.mask[0] = 1.0; // top-left painted
        s.arrows.push(crate::scene::Arrow { from: [0.1, 0.2], to: [0.3, 0.2] });
        s.rotate(true);
        assert_eq!(s.full.dimensions(), (20, 40));
        // Top-left goes to top-right after a right turn.
        assert_eq!(s.full.get_pixel(19, 0)[0], 255);
        assert_eq!(s.mask[19], 1.0);
        let a = s.arrows[0];
        assert!((a.from[0] - 0.8).abs() < 1e-6 && (a.from[1] - 0.1).abs() < 1e-6);
        s.rotate(false);
        assert_eq!(s.full.dimensions(), (40, 20));
        assert_eq!(s.mask[0], 1.0);
        assert!((s.arrows[0].from[0] - 0.1).abs() < 1e-6);
    }

    #[test]
    fn crop_and_rotation_undo_in_order() {
        let mut s = Scene::new("t".into(), RgbaImage::new(40, 20));
        s.checkpoint();
        s.dab([0.1, 0.1], 3.0, 0.5, false);
        let painted = s.mask.clone();
        s.checkpoint();
        s.crop = crate::crop::CropRect { x: 0.25, y: 0.0, w: 0.5, h: 1.0 };
        s.crop_applied = true;
        s.rotate(true);
        assert_eq!(s.full.dimensions(), (20, 40));
        // Undo the turn, then the crop, then the painting.
        assert!(s.undo());
        assert_eq!(s.full.dimensions(), (40, 20));
        assert_eq!(s.crop, crate::crop::CropRect { x: 0.25, y: 0.0, w: 0.5, h: 1.0 });
        assert!(!s.undo());
        assert!(s.crop.is_full() && !s.crop_applied);
        assert_eq!(s.mask, painted);
        assert!(!s.undo());
        assert_eq!(s.mask_coverage(), 0.0);
        // And forward again.
        s.redo();
        s.redo();
        assert!(s.redo());
        assert_eq!(s.full.dimensions(), (20, 40));
        assert!(!s.crop.is_full());
    }

    #[test]
    fn a_painted_stroke_undoes_and_redoes() {
        let mut s = Scene::new("t".into(), RgbaImage::new(64, 64));
        s.checkpoint();
        for i in 0..10 {
            s.dab([0.2 + i as f32 * 0.05, 0.5], 6.0, 0.5, false);
        }
        let painted = s.mask_coverage();
        assert!(painted > 0.0);
        let v = s.mask_version;
        s.undo();
        assert_eq!(s.mask_coverage(), 0.0);
        assert!(s.mask_version > v, "overlay must refresh after undo");
        s.redo();
        assert_eq!(s.mask_coverage(), painted);
    }
}
