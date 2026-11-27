//! Saving an edited video: the whole file, every frame with the photo edits
//! (look, masks, quarter turns, straighten, crop, caption and border), at the
//! chosen size, with the original timing and sound. No loop is made.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::mpsc::channel;

use image::RgbaImage;

use crate::app::{StudioApp, Task, TaskKind, TaskResult};
use crate::export::Format;

use crate::crop::CropRect;
use crate::export_size::{ExportSize, natural_size};
use crate::render::Progress;
use crate::scene::Edits;

pub struct VideoJob {
    pub source: PathBuf,
    pub target: PathBuf,
    pub mov: bool,
    pub crop: CropRect,
    pub edits: Edits,
    pub size: ExportSize,
    /// Quarter turns to the right, then the straighten angle.
    pub quarter_turns: u8,
    pub angle: f32,
    /// Convert Apple Log to normal colours.
    pub from_log: bool,
}

impl VideoJob {
    /// One frame as it is saved.
    pub fn finish(&self, frame: RgbaImage) -> RgbaImage {
        let mut dims = frame.dimensions();
        if self.quarter_turns % 2 == 1 {
            dims = (dims.1, dims.0);
        }
        let natural = natural_size(dims, self.crop, &self.edits.border);
        let long = self.size.source_long(dims, natural);
        let mut img = crate::scene::downscale(&frame, long);
        drop(frame);
        if self.from_log {
            crate::applelog::to_display(&mut img);
        }
        for _ in 0..self.quarter_turns {
            img = image::imageops::rotate90(&img);
        }
        if self.angle.abs() >= 1e-3 {
            img = crate::straighten::rotate_fill(&img, self.angle);
        }
        let img = crate::app::framed_still(&img, self.crop, &self.edits, long);
        self.size.exact(img, natural)
    }

    /// Save the video; returns where, and a note about the sound when it
    /// could not be kept.
    pub fn run(self, progress: &Progress) -> Result<(PathBuf, Option<String>), String> {
        #[cfg(target_os = "macos")]
        {
            use crate::macvideo_stream::{Sound, transcode};
            let sound = transcode(&self.source, &self.target, self.mov, progress, &mut |f| self.finish(f))?;
            let note = match sound {
                Sound::Dropped(why) => Some(format!("Saved without sound: {why}")),
                Sound::Kept | Sound::None => None,
            };
            Ok((self.target, note))
        }
        #[cfg(not(target_os = "macos"))]
        {
            let _ = progress;
            Err("Saving a whole edited video needs macOS.".into())
        }
    }
}

impl StudioApp {
    /// The open video file, upright, when the whole video can be saved.
    pub fn video_origin(&self) -> Option<crate::video::Origin> {
        let s = self.scene.as_ref()?;
        let mut o = s.video.as_ref()?.origin.clone()?;
        if s.quarter_turns % 2 == 1 {
            o.size = (o.size.1, o.size.0);
        }
        Some(o)
    }

    /// Does Export save the whole edited video now?
    pub fn saves_whole_video(&self) -> bool {
        self.export_format.is_video() && self.video_whole && self.video_origin().is_some() && !self.busy()
    }

    /// Ask where, then save the whole video with the current edits.
    pub fn export_whole_video(&mut self) {
        let Some(scene) = self.scene.as_ref().filter(|_| self.video_origin().is_some()) else { return };
        let format = self.export_format;
        let ext = if format == Format::Mov { "mov" } else { "mp4" };
        let base = scene.name.rsplit_once('.').map_or(scene.name.as_str(), |(a, _)| a);
        let Some(target) = rfd::FileDialog::new()
            .set_title("Save edited video")
            .set_file_name(format!("{base}_edited.{ext}"))
            .add_filter(format.label(), &[ext])
            .save_file()
        else {
            return;
        };
        self.save_whole_video(target);
    }

    /// Save the whole video to `target` with the current edits.
    pub fn save_whole_video(&mut self, target: PathBuf) {
        let (Some(origin), Some(scene)) = (self.video_origin(), &self.scene) else { return };
        let format = self.export_format;
        if target == origin.path {
            self.error = Some("Choose another name: the video cannot be saved over the file it is read from.".into());
            return;
        }
        let Some((scene, edits)) = self.scene_and_edits(scene.id) else { return };
        let job = VideoJob {
            source: origin.path,
            target,
            mov: format == Format::Mov,
            crop: scene.crop,
            edits,
            size: self.export_size,
            quarter_turns: scene.quarter_turns,
            angle: scene.angle,
            from_log: scene.video.as_ref().is_some_and(|v| v.log_converted),
        };
        let progress = Arc::new(Progress::new());
        let p = progress.clone();
        let (tx, rx) = channel();
        std::thread::spawn(move || {
            let _ = tx.send(TaskResult::VideoSaved(job.run(&p)));
        });
        self.status = "Saving the edited video…".into();
        self.task = Some(Task { progress, rx, kind: TaskKind::Export, label: "Saving video".into() });
    }

    pub fn video_saved(&mut self, result: Result<(PathBuf, Option<String>), String>) {
        match result {
            Ok((path, note)) => {
                self.status = format!("Saved {}", path.display());
                if let Some(n) = note {
                    self.error = Some(n);
                }
                self.last_export = Some(path);
            }
            Err(e) if e == "Canceled" => self.status = "Saving the video was canceled.".into(),
            Err(e) => {
                self.status = "Saving the video failed.".into();
                self.error = Some(e);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn job(quarter_turns: u8, crop: CropRect, size: ExportSize) -> VideoJob {
        VideoJob {
            source: PathBuf::new(),
            target: PathBuf::new(),
            mov: false,
            crop,
            edits: Edits::default(),
            size,
            quarter_turns,
            angle: 0.0,
            from_log: false,
        }
    }

    #[test]
    fn frames_are_turned_cropped_and_sized_like_the_still() {
        let frame = RgbaImage::from_pixel(400, 200, image::Rgba([90, 90, 90, 255]));
        assert_eq!(job(0, CropRect::FULL, ExportSize::ORIGINAL).finish(frame.clone()).dimensions(), (400, 200));
        assert_eq!(job(1, CropRect::FULL, ExportSize::ORIGINAL).finish(frame.clone()).dimensions(), (200, 400));
        let half = CropRect { x: 0.0, y: 0.0, w: 0.5, h: 1.0 };
        assert_eq!(job(0, half, ExportSize::ORIGINAL).finish(frame.clone()).dimensions(), (200, 200));
        let wide = ExportSize { side: crate::export_size::Side::Width, px: 100 };
        assert_eq!(job(0, CropRect::FULL, wide).finish(frame).dimensions(), (100, 50));
    }

    /// A short video with edits saved through the whole path, then read back.
    #[cfg(target_os = "macos")]
    #[test]
    fn a_whole_video_is_saved_with_its_edits() {
        let dir = std::env::temp_dir().join(format!("animagraph_video_job_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let source = dir.join("in.mp4");
        let frames: Vec<RgbaImage> =
            (0..36u8).map(|i| RgbaImage::from_pixel(320, 180, image::Rgba([120, 120 + i, 120, 255]))).collect();
        crate::macexport::write_video(&frames, 12, &source, false, 1).unwrap();
        let mut edits = Edits::default();
        edits.look.exposure = 1.0;
        let crop = CropRect { x: 0.0, y: 0.0, w: 0.5, h: 1.0 };
        let target = dir.join("out.mov");
        let job = VideoJob { source, target: target.clone(), mov: true, edits, ..job(1, crop, ExportSize::ORIGINAL) };
        let (saved, note) = job.run(&Progress::new()).unwrap();
        assert_eq!(saved, target);
        assert_eq!(note, None, "a video without sound has nothing to drop");
        let back = crate::macvideo::decode(&target, 10.0, 12).unwrap();
        // Every frame, turned (180 x 320), cropped to the left half (90 x 320).
        assert!((34..=36).contains(&back.frames.len()), "{} frames", back.frames.len());
        assert_eq!(back.frames[0].dimensions(), (90, 320));
        // Brighter than the 120 grey it started as.
        assert!(back.frames[5].get_pixel(45, 160)[0] > 150, "{:?}", back.frames[5].get_pixel(45, 160));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
