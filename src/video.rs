//! Video input: a loop made from a short clip. One frame is
//! frozen and the clip plays only inside (or only outside) the painted area.
//! Videos are read with ffmpeg when it is installed; animated GIFs and image
//! sequences are read directly.

use std::path::{Path, PathBuf};
use std::process::Command;

use image::RgbaImage;

use crate::render::{Progress, par_chunks};

/// Longest side kept for video frames (memory grows with every frame).
pub const MAX_SIDE: u32 = 1280;
/// Upper limit on frames read from any source.
pub const MAX_FRAMES: usize = 300;

pub struct VideoSource {
    /// Frames at up to `MAX_SIDE`.
    pub frames: Vec<RgbaImage>,
    /// The same frames at editing size.
    pub work: Vec<RgbaImage>,
    pub fps: f32,
    /// Shot in Apple Log (iPhone "Log" video).
    pub apple_log: bool,
    /// For Apple Log clips, the frames as read (still log-encoded, flat),
    /// kept so the conversion can be switched on and off.
    pub log_frames: Option<std::sync::Arc<Vec<RgbaImage>>>,
    /// The frames in use are converted from Apple Log to normal colours.
    pub log_converted: bool,
}

impl VideoSource {
    pub fn new(frames: Vec<RgbaImage>, fps: f32) -> Result<Self, String> {
        if frames.len() < 2 {
            return Err("The clip needs at least two frames.".into());
        }
        let (w, h) = frames[0].dimensions();
        let frames: Vec<RgbaImage> = frames
            .into_iter()
            .map(|f| {
                let f = if f.dimensions() == (w, h) {
                    f
                } else {
                    image::imageops::resize(&f, w, h, image::imageops::FilterType::Triangle)
                };
                crate::scene::downscale(&f, MAX_SIDE)
            })
            .collect();
        let work = frames
            .iter()
            .map(|f| crate::scene::downscale(f, crate::scene::WORK_SIZE))
            .collect();
        Ok(Self { frames, work, fps: fps.clamp(1.0, 60.0), apple_log: false, log_frames: None, log_converted: false })
    }

    /// For an Apple Log clip as read, keep the log frames and use them
    /// converted to normal colours (`convert`) or as they are.
    pub fn with_log_choice(mut self, convert: bool) -> Self {
        if !self.apple_log {
            return self;
        }
        let log = std::sync::Arc::new(std::mem::take(&mut self.frames));
        Self::from_log(log, self.fps, convert)
    }

    /// The same Apple Log clip with the conversion switched on or off.
    pub fn log_switched(&self, convert: bool) -> Option<Self> {
        let log = self.log_frames.clone()?;
        Some(Self::from_log(log, self.fps, convert))
    }

    fn from_log(log: std::sync::Arc<Vec<RgbaImage>>, fps: f32, convert: bool) -> Self {
        let frames: Vec<RgbaImage> = std::thread::scope(|s| {
            let handles: Vec<_> = log
                .chunks(log.len().div_ceil(8).max(1))
                .map(|chunk| {
                    s.spawn(move || {
                        chunk
                            .iter()
                            .map(|f| {
                                let mut f = f.clone();
                                if convert {
                                    crate::applelog::to_display(&mut f);
                                }
                                f
                            })
                            .collect::<Vec<_>>()
                    })
                })
                .collect();
            handles.into_iter().flat_map(|h| h.join().unwrap_or_default()).collect()
        });
        let work = frames.iter().map(|f| crate::scene::downscale(f, crate::scene::WORK_SIZE)).collect();
        Self { frames, work, fps, apple_log: true, log_frames: Some(log), log_converted: convert }
    }

    pub fn seconds(&self) -> f32 {
        self.frames.len() as f32 / self.fps
    }
}

pub fn is_video_file(path: &Path) -> bool {
    matches!(
        ext(path).as_deref(),
        Some("mp4" | "mov" | "m4v" | "webm" | "avi" | "mkv" | "mts" | "3gp")
    )
}

pub fn is_gif(path: &Path) -> bool {
    ext(path).as_deref() == Some("gif")
}

fn ext(path: &Path) -> Option<String> {
    path.extension().and_then(|e| e.to_str()).map(|e| e.to_ascii_lowercase())
}

/// Find ffmpeg on PATH or in the usual install folders (apps started from
/// the Dock don't see the shell's PATH).
pub fn ffmpeg_path() -> Option<PathBuf> {
    let mut candidates = vec![PathBuf::from("ffmpeg")];
    for dir in ["/opt/homebrew/bin", "/usr/local/bin", "/usr/bin", "/opt/local/bin"] {
        candidates.push(Path::new(dir).join("ffmpeg"));
    }
    candidates.into_iter().find(|c| {
        Command::new(c)
            .arg("-version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    })
}

/// Read the first `seconds` of any video file. On macOS the system's own
/// video support is used first (MP4, MOV, M4V, including iPhone HEVC);
/// ffmpeg, when installed, covers the rest (WebM, MKV, AVI...).
pub fn decode_video(path: &Path, seconds: f32, fps: u32) -> Result<VideoSource, String> {
    #[cfg(target_os = "macos")]
    let first_error = match crate::macvideo::decode(path, seconds, fps) {
        Ok(v) => return Ok(v),
        Err(e) => e,
    };
    #[cfg(not(target_os = "macos"))]
    let first_error = String::from("This video format needs ffmpeg.");
    match ffmpeg_path() {
        Some(ffmpeg) => decode_with_ffmpeg(&ffmpeg, path, seconds, fps),
        None => Err(format!(
            "{first_error}\n\nFor WebM, MKV or AVI files install ffmpeg (on a Mac: brew install ffmpeg), \
             or convert the clip to MP4 or MOV."
        )),
    }
}

/// Read the first `seconds` of a video at `fps` frames per second.
pub fn decode_with_ffmpeg(ffmpeg: &Path, path: &Path, seconds: f32, fps: u32) -> Result<VideoSource, String> {
    let tmp = std::env::temp_dir().join(format!(
        "animagraph_decode_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0)
    ));
    std::fs::create_dir_all(&tmp).map_err(|e| e.to_string())?;
    let filter = format!(
        "fps={fps},scale={MAX_SIDE}:{MAX_SIDE}:force_original_aspect_ratio=decrease:force_divisible_by=2"
    );
    let out = Command::new(ffmpeg)
        .args(["-y", "-loglevel", "error", "-i"])
        .arg(path)
        .arg("-t")
        .arg(format!("{seconds:.2}"))
        .args(["-vf", &filter, "-frames:v"])
        .arg(MAX_FRAMES.to_string())
        .arg(tmp.join("f_%04d.png"))
        .output()
        .map_err(|e| format!("Could not start ffmpeg: {e}"))?;
    if !out.status.success() {
        let _ = std::fs::remove_dir_all(&tmp);
        return Err(String::from_utf8_lossy(&out.stderr).trim().to_string());
    }
    let mut files: Vec<PathBuf> = std::fs::read_dir(&tmp)
        .map_err(|e| e.to_string())?
        .flatten()
        .map(|e| e.path())
        .collect();
    files.sort();
    let frames: Result<Vec<RgbaImage>, String> = files
        .iter()
        .map(|f| image::open(f).map(|i| i.to_rgba8()).map_err(|e| e.to_string()))
        .collect();
    let _ = std::fs::remove_dir_all(&tmp);
    VideoSource::new(frames?, fps as f32)
}

/// Read an animated GIF, keeping at most `seconds` of it.
pub fn decode_gif(path: &Path, seconds: f32) -> Result<VideoSource, String> {
    use image::AnimationDecoder;
    let file = std::fs::File::open(path).map_err(|e| e.to_string())?;
    let decoder = image::codecs::gif::GifDecoder::new(std::io::BufReader::new(file)).map_err(|e| e.to_string())?;
    let mut frames = Vec::new();
    let mut total_ms = 0.0f32;
    for f in decoder.into_frames() {
        let f = f.map_err(|e| e.to_string())?;
        let (n, d) = f.delay().numer_denom_ms();
        let ms = if d == 0 { 100.0 } else { (n as f32 / d as f32).max(20.0) };
        if !frames.is_empty() && (total_ms + ms) / 1000.0 > seconds {
            break;
        }
        total_ms += ms;
        frames.push(f.into_buffer());
        if frames.len() >= MAX_FRAMES {
            break;
        }
    }
    let fps = if total_ms > 0.0 { frames.len() as f32 * 1000.0 / total_ms } else { 10.0 };
    VideoSource::new(frames, fps)
}

/// Several photos (for example a burst) played as a clip, in name order.
pub fn from_images(paths: &[PathBuf], fps: u32) -> Result<VideoSource, String> {
    let mut paths = paths.to_vec();
    paths.sort();
    let frames: Result<Vec<RgbaImage>, String> = paths
        .iter()
        .take(MAX_FRAMES)
        .map(|p| {
            image::open(p)
                .map(|i| crate::scene::downscale(&i.to_rgba8(), MAX_SIDE))
                .map_err(|e| format!("{}: {e}", p.display()))
        })
        .collect();
    VideoSource::new(frames?, fps as f32)
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VideoLoop {
    /// true: the painted area shows the frozen frame and the rest plays.
    pub freeze_painted: bool,
    pub still_index: usize,
    /// Frames blended at the end so the loop has no jump.
    pub crossfade: usize,
    /// Trim: first and last frame used (inclusive).
    pub start: usize,
    pub end: usize,
    /// Steady a handheld clip against the still frame.
    pub stabilize: bool,
}

impl VideoLoop {
    #[cfg(test)]
    pub fn simple(still_index: usize, crossfade: usize, n: usize) -> Self {
        Self { freeze_painted: true, still_index, crossfade, start: 0, end: n - 1, stabilize: false }
    }
}

/// Full pipeline for a clip: trim, steady (optional) and composite.
/// `frames` may be any size; `work` is the editing-size copy used to
/// measure drift. `mask` is at `frames` size.
pub fn make_loop(
    frames: &[RgbaImage],
    work: &[RgbaImage],
    mask: &[f32],
    s: VideoLoop,
    progress: &Progress,
) -> Option<Vec<RgbaImage>> {
    let n = frames.len();
    let end = s.end.min(n - 1);
    let start = s.start.min(end.saturating_sub(1));
    let still_i = s.still_index.min(n - 1);
    let (w, h) = frames[0].dimensions();
    if !s.stabilize {
        return composite_loop(&frames[start..=end], &frames[still_i], mask, s.freeze_painted, s.crossfade, progress);
    }
    let offs = crate::stabilize::offsets(work, still_i);
    let scale = w as f32 / work[0].width() as f32;
    let offs: Vec<[f32; 2]> = offs.iter().map(|o| [o[0] * scale, o[1] * scale]).collect();
    let zoom = crate::stabilize::zoom_for(&offs[start..=end], w, h);
    let steady: Vec<RgbaImage> = (start..=end)
        .map(|i| crate::stabilize::transform(&frames[i], offs[i], zoom))
        .collect();
    let still = crate::stabilize::transform(&frames[still_i], offs[still_i], zoom);
    let mask = crate::stabilize::zoom_mask(mask, w, h, zoom);
    composite_loop(&steady, &still, &mask, s.freeze_painted, s.crossfade, progress)
}

/// One live-preview frame: `frame` where it plays, `still` where it is frozen.
pub fn live_frame(frame: &RgbaImage, still: &RgbaImage, mask: &[f32], freeze_painted: bool) -> RgbaImage {
    let (w, h) = (frame.width() as usize, frame.height() as usize);
    let (a, st) = (frame.as_raw(), still.as_raw());
    let mut buf = vec![0u8; w * h * 4];
    par_chunks(&mut buf, w * 4, |y, row| {
        for x in 0..w {
            let i = y * w + x;
            let m = mask[i].clamp(0.0, 1.0);
            let mv = if freeze_painted { 1.0 - m } else { m };
            for c in 0..3 {
                row[x * 4 + c] = (a[i * 4 + c] as f32 * mv + st[i * 4 + c] as f32 * (1.0 - mv)).round() as u8;
            }
            row[x * 4 + 3] = 255;
        }
    });
    RgbaImage::from_raw(w as u32, h as u32, buf).expect("size")
}

/// Composite the loop: moving parts from each frame, frozen parts from the
/// still frame, with a crossfade from the end back to the start.
/// `mask` is at frame size. Returns `None` when canceled.
pub fn composite_loop(
    frames: &[RgbaImage],
    still: &RgbaImage,
    mask: &[f32],
    freeze_painted: bool,
    crossfade: usize,
    progress: &Progress,
) -> Option<Vec<RgbaImage>> {
    let n = frames.len();
    let (w, h) = frames[0].dimensions();
    let (w, h) = (w as usize, h as usize);
    let k = crossfade.min(n / 2);
    let out_len = n - k;
    let still = still.as_raw();
    let soft = crate::scene::blur_mask(mask, w as u32, h as u32, 1);
    let moving: Vec<f32> = soft
        .iter()
        .map(|m| {
            let m = m.clamp(0.0, 1.0);
            if freeze_painted { 1.0 - m } else { m }
        })
        .collect();
    let mut out = Vec::with_capacity(out_len);
    for t in 0..out_len {
        if progress.cancel.load(std::sync::atomic::Ordering::Relaxed) {
            return None;
        }
        let a = frames[t].as_raw();
        // Early frames fade in from the tail of the clip.
        let (b, wb) = if t < k {
            (Some(frames[n - k + t].as_raw()), 1.0 - (t as f32 + 1.0) / (k as f32 + 1.0))
        } else {
            (None, 0.0)
        };
        let mut buf = vec![0u8; w * h * 4];
        par_chunks(&mut buf, w * 4, |y, row| {
            for x in 0..w {
                let i = y * w + x;
                let mv = moving[i];
                for c in 0..3 {
                    let mut v = a[i * 4 + c] as f32;
                    if let Some(b) = b {
                        v = v * (1.0 - wb) + b[i * 4 + c] as f32 * wb;
                    }
                    let s = still[i * 4 + c] as f32;
                    row[x * 4 + c] = (v * mv + s * (1.0 - mv)).round().clamp(0.0, 255.0) as u8;
                }
                row[x * 4 + 3] = 255;
            }
        });
        out.push(RgbaImage::from_raw(w as u32, h as u32, buf).expect("frame size"));
        progress
            .permille
            .store(((t + 1) * 1000 / out_len) as u32, std::sync::atomic::Ordering::Relaxed);
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn clip() -> Vec<RgbaImage> {
        (0..10u8).map(|i| RgbaImage::from_pixel(8, 4, image::Rgba([i * 20, 0, 0, 255]))).collect()
    }

    #[test]
    fn painted_area_freezes_and_rest_plays() {
        let frames = clip();
        // Left half painted.
        let mask: Vec<f32> = (0..32).map(|i| if i % 8 < 3 { 1.0 } else { 0.0 }).collect();
        let s = VideoLoop::simple(2, 0, frames.len());
        let out = make_loop(&frames, &frames, &mask, s, &Progress::new()).unwrap();
        assert_eq!(out.len(), 10);
        assert_eq!(out[7].get_pixel(0, 0)[0], 40); // frozen at frame 2
        assert_eq!(out[7].get_pixel(7, 0)[0], 140); // plays
        let s = VideoLoop { freeze_painted: false, ..s };
        let out = make_loop(&frames, &frames, &mask, s, &Progress::new()).unwrap();
        assert_eq!(out[7].get_pixel(0, 0)[0], 140);
        assert_eq!(out[7].get_pixel(7, 0)[0], 40);
    }

    #[test]
    fn crossfade_shortens_and_smooths_the_loop() {
        let frames = clip();
        let mask = vec![0.0; 32];
        let s = VideoLoop::simple(0, 4, frames.len());
        let out = make_loop(&frames, &frames, &mask, s, &Progress::new()).unwrap();
        assert_eq!(out.len(), 6);
        let v = |i: usize| out[i].get_pixel(4, 2)[0] as i32;
        // Wrap-around step (last -> first) is no bigger than a normal step.
        assert!((v(0) - v(5)).abs() <= 20, "{} {}", v(0), v(5));
    }

    #[test]
    fn trim_keeps_only_the_chosen_frames() {
        let frames = clip();
        let mask = vec![0.0; 32];
        let s = VideoLoop { start: 3, end: 6, ..VideoLoop::simple(0, 0, frames.len()) };
        let out = make_loop(&frames, &frames, &mask, s, &Progress::new()).unwrap();
        let v: Vec<u8> = out.iter().map(|f| f.get_pixel(5, 1)[0]).collect();
        assert_eq!(v, vec![60, 80, 100, 120]);
    }

    #[test]
    fn gif_round_trip() {
        let frames = clip();
        let p = std::env::temp_dir().join(format!("cg_vid_{}.gif", std::process::id()));
        crate::export::save(crate::export::Format::Gif, &frames, 10, &p, 1).unwrap();
        let src = decode_gif(&p, 3.0).unwrap();
        assert_eq!(src.frames.len(), 10);
        assert!((src.fps - 10.0).abs() < 0.5);
        let _ = std::fs::remove_file(p);
    }

    /// Writes a demo clip for manual checks: `DEMO_GIF=path cargo test -- --ignored`.
    #[test]
    #[ignore]
    fn write_demo_gif() {
        let Ok(path) = std::env::var("DEMO_GIF") else { return };
        let photo = crate::sample::landscape(480, 270);
        let job = crate::render::RenderJob {
            photo,
            mask: vec![1.0; 480 * 270],
            arrows: vec![crate::scene::Arrow { from: [0.2, 0.3], to: [0.5, 0.3] }],
            pins: vec![],
            speed: 1.0,
            frames: 36,
        };
        let mut frames = crate::render::render_loop(&job, &Progress::new()).unwrap();
        // A sweeping bright bar so playing and frozen areas are easy to tell apart.
        for (i, f) in frames.iter_mut().enumerate() {
            let x0 = (i * 12) as u32 % 480;
            for y in 0..270 {
                for x in x0..(x0 + 20).min(480) {
                    f.put_pixel(x, y, image::Rgba([255, 230, 120, 255]));
                }
            }
        }
        crate::export::save(crate::export::Format::Gif, &frames, 12, std::path::Path::new(&path), 1).unwrap();
    }
}
