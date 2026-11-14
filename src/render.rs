//! Built-in animator. Warps the photo along the motion field and blends a
//! forward-moving copy with a "from the future" copy so the last frame
//! flows straight back into the first (a seamless loop).

use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

use image::RgbaImage;

use crate::scene::Arrow;

#[derive(Clone)]
pub struct RenderJob {
    pub photo: RgbaImage,
    /// Motion region at `photo` size.
    pub mask: Vec<f32>,
    pub arrows: Vec<Arrow>,
    pub pins: Vec<[f32; 2]>,
    pub speed: f32,
    pub frames: usize,
}

/// Progress in thousandths, shared with the UI thread.
pub struct Progress {
    pub permille: AtomicU32,
    pub cancel: AtomicBool,
}

impl Progress {
    pub fn new() -> Self {
        Self {
            permille: AtomicU32::new(0),
            cancel: AtomicBool::new(false),
        }
    }
    fn set(&self, done: usize, total: usize) {
        let v = (done as f32 / total.max(1) as f32 * 1000.0) as u32;
        self.permille.store(v.min(1000), Ordering::Relaxed);
    }
    pub fn fraction(&self) -> f32 {
        self.permille.load(Ordering::Relaxed) as f32 / 1000.0
    }
    fn canceled(&self) -> bool {
        self.cancel.load(Ordering::Relaxed)
    }
}

/// Run `f(row_index, row)` over rows of `data` on all cores.
pub fn par_chunks<T: Send>(data: &mut [T], row_len: usize, f: impl Fn(usize, &mut [T]) + Sync) {
    let threads = std::thread::available_parallelism().map_or(4, |n| n.get()).max(1);
    let rows = data.len() / row_len.max(1);
    let rows_per = rows.div_ceil(threads).max(1);
    std::thread::scope(|s| {
        for (ci, chunk) in data.chunks_mut(rows_per * row_len).enumerate() {
            let f = &f;
            s.spawn(move || {
                for (ri, row) in chunk.chunks_mut(row_len).enumerate() {
                    f(ci * rows_per + ri, row);
                }
            });
        }
    });
}

#[inline]
fn sample_field(field: &[[f32; 2]], w: usize, h: usize, x: f32, y: f32) -> [f32; 2] {
    let x = x.clamp(0.0, (w - 1) as f32);
    let y = y.clamp(0.0, (h - 1) as f32);
    let x0 = x.floor() as usize;
    let y0 = y.floor() as usize;
    let x1 = (x0 + 1).min(w - 1);
    let y1 = (y0 + 1).min(h - 1);
    let (tx, ty) = (x - x0 as f32, y - y0 as f32);
    let a = field[y0 * w + x0];
    let b = field[y0 * w + x1];
    let c = field[y1 * w + x0];
    let d = field[y1 * w + x1];
    let mut out = [0.0; 2];
    for k in 0..2 {
        out[k] = (a[k] * (1.0 - tx) + b[k] * tx) * (1.0 - ty) + (c[k] * (1.0 - tx) + d[k] * tx) * ty;
    }
    out
}

#[inline]
fn sample_rgb(img: &[u8], w: usize, h: usize, x: f32, y: f32) -> [f32; 3] {
    let x = x.clamp(0.0, (w - 1) as f32);
    let y = y.clamp(0.0, (h - 1) as f32);
    let x0 = x.floor() as usize;
    let y0 = y.floor() as usize;
    let x1 = (x0 + 1).min(w - 1);
    let y1 = (y0 + 1).min(h - 1);
    let (tx, ty) = (x - x0 as f32, y - y0 as f32);
    let px = |xx: usize, yy: usize, k: usize| img[(yy * w + xx) * 4 + k] as f32;
    let mut out = [0.0; 3];
    for (k, o) in out.iter_mut().enumerate() {
        let top = px(x0, y0, k) * (1.0 - tx) + px(x1, y0, k) * tx;
        let bot = px(x0, y1, k) * (1.0 - tx) + px(x1, y1, k) * tx;
        *o = top * (1.0 - ty) + bot * ty;
    }
    out
}

/// Advance every traced position one step along (`dir` = +1) or against
/// (`dir` = -1) the field.
fn step_trace(trace: &mut [[f32; 2]], field: &[[f32; 2]], w: usize, h: usize, dir: f32) {
    par_chunks(trace, w, |_, row| {
        for p in row.iter_mut() {
            let v = sample_field(field, w, h, p[0], p[1]);
            p[0] += dir * v[0];
            p[1] += dir * v[1];
        }
    });
}

fn identity_trace(w: usize, h: usize) -> Vec<[f32; 2]> {
    let mut t = vec![[0.0f32; 2]; w * h];
    for y in 0..h {
        for x in 0..w {
            t[y * w + x] = [x as f32, y as f32];
        }
    }
    t
}

/// Render the loop. Returns `None` when canceled.
pub fn render_loop(job: &RenderJob, progress: &Progress) -> Option<Vec<RgbaImage>> {
    let (w, h) = (job.photo.width() as usize, job.photo.height() as usize);
    let n = job.frames.max(2);
    let src = job.photo.as_raw();
    let field = crate::motion::dense_field(w as u32, h as u32, &job.arrows, &job.pins, &job.mask, job.speed);
    let still_mask = crate::scene::blur_mask(&job.mask, w as u32, h as u32, 1);
    let total_steps = 2 * n;
    let mut done = 0;

    // Pass 1: copies traced forward 1..n-1 steps ("where content will be").
    let mut future: Vec<Vec<u8>> = Vec::with_capacity(n);
    future.push(Vec::new()); // index 0 unused
    let mut trace = identity_trace(w, h);
    for _ in 1..n {
        if progress.canceled() {
            return None;
        }
        step_trace(&mut trace, &field, w, h, 1.0);
        let mut img = vec![0u8; w * h * 3];
        par_chunks(&mut img, w * 3, |y, row| {
            for x in 0..w {
                let p = trace[y * w + x];
                let c = sample_rgb(src, w, h, p[0], p[1]);
                for k in 0..3 {
                    row[x * 3 + k] = c[k].round() as u8;
                }
            }
        });
        future.push(img);
        done += 1;
        progress.set(done, total_steps);
    }

    // Pass 2: content traced backward t steps, blended with the future copy.
    let mut frames = Vec::with_capacity(n);
    frames.push(job.photo.clone());
    let mut trace = identity_trace(w, h);
    for t in 1..n {
        if progress.canceled() {
            return None;
        }
        step_trace(&mut trace, &field, w, h, -1.0);
        let alpha = t as f32 / n as f32;
        let fut = &future[n - t];
        let mut out = vec![0u8; w * h * 4];
        par_chunks(&mut out, w * 4, |y, row| {
            for x in 0..w {
                let i = y * w + x;
                let p = trace[i];
                let a = sample_rgb(src, w, h, p[0], p[1]);
                let m = still_mask[i].clamp(0.0, 1.0);
                for k in 0..3 {
                    let moving = a[k] * (1.0 - alpha) + fut[i * 3 + k] as f32 * alpha;
                    let still = src[i * 4 + k] as f32;
                    row[x * 4 + k] = (moving * m + still * (1.0 - m)).round().clamp(0.0, 255.0) as u8;
                }
                row[x * 4 + 3] = 255;
            }
        });
        frames.push(RgbaImage::from_raw(w as u32, h as u32, out).expect("frame size"));
        done += 1;
        progress.set(done + 1, total_steps);
    }
    progress.set(total_steps, total_steps);
    Some(frames)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loop_moves_masked_area_and_keeps_rest_still() {
        let photo = crate::sample::landscape(160, 90);
        let (w, h) = photo.dimensions();
        let mut mask = vec![0.0; (w * h) as usize];
        for y in 0..h / 2 {
            for x in 0..w {
                mask[(y * w + x) as usize] = 1.0;
            }
        }
        let job = RenderJob {
            photo: photo.clone(),
            mask,
            arrows: vec![Arrow { from: [0.2, 0.2], to: [0.6, 0.2] }],
            pins: vec![],
            speed: 1.0,
            frames: 24,
        };
        let frames = render_loop(&job, &Progress::new()).unwrap();
        assert_eq!(frames.len(), 24);
        assert_eq!(frames[0], photo);
        let diff = |a: &RgbaImage, b: &RgbaImage, y0: u32, y1: u32| {
            let mut d = 0u64;
            for y in y0..y1 {
                for x in 0..w {
                    for k in 0..3 {
                        d += (a.get_pixel(x, y)[k] as i32 - b.get_pixel(x, y)[k] as i32).unsigned_abs() as u64;
                    }
                }
            }
            d
        };
        // The sky moves mid-loop.
        assert!(diff(&frames[12], &photo, 0, h / 2 - 4) > 0);
        // The lower half stays still.
        assert_eq!(diff(&frames[12], &photo, h / 2 + 4, h), 0);
        // The last frame is close to the first, so the loop has no visible jump.
        let jump = diff(&frames[23], &frames[0], 0, h);
        let mid = diff(&frames[12], &frames[0], 0, h);
        assert!(jump * 3 < mid, "jump {jump} vs mid {mid}");
    }
}
