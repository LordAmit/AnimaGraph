//! Application state, window layout and background jobs.

use std::path::PathBuf;
use std::sync::mpsc::{Receiver, channel};
use std::sync::Arc;

use eframe::egui::{self, Color32, TextureHandle, TextureOptions};
use image::RgbaImage;

use crate::export::Format;
use crate::render::{Progress, RenderJob};
use crate::scene::{Arrow, Scene};
use crate::theme;
use crate::crop::CropRect;
use crate::finish::Look;
use crate::video::{VideoLoop, VideoSource};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Tool {
    /// Pointer: look and compare without changing anything.
    Select,
    Brush,
    Eraser,
    Arrow,
    Pin,
    Rect,
    Crop,
    Hand,
}

/// Default values of the adjustable settings (used by the reset buttons).
pub mod defaults {
    pub const BRUSH_RADIUS: f32 = 28.0;
    pub const BRUSH_HARDNESS: f32 = 0.5;
    pub const SPEED: f32 = 1.0;
    pub const LOOP_SECONDS: f32 = 3.0;
    pub const FPS: u32 = 24;
    pub const AUTO_ANGLE: f32 = 0.0;
    pub const ARROW_LEN: f32 = 0.12;
    pub const VIDEO_SPEED: f32 = 1.0;
    pub const CROSSFADE: f32 = 0.5;
    pub const VIDEO_SECONDS: f32 = 3.0;
    pub const REPEATS: u32 = 3;
}

/// How the canvas compares the edited picture with the original.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Compare {
    /// The edited picture (normal working view).
    Edited,
    /// The untouched photo: no look, crop, caption or border.
    Original,
    /// Original on the left of a movable divider, edited on the right.
    Split,
}

/// Which color the eyedropper fills in.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PickTarget {
    Border,
    Caption,
}

/// What a drag on the crop box is doing.
#[derive(Clone, Copy, Debug)]
pub enum CropDrag {
    /// Resize from a corner; the opposite corner stays put.
    Corner { anchor: [f32; 2] },
    /// Move one side (free shape only): 0 left, 1 right, 2 top, 3 bottom.
    Side { side: u8, start: CropRect },
    Move { grab: [f32; 2], start: CropRect },
    New { anchor: [f32; 2] },
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum View {
    Edit,
    Preview,
}

/// What a clip was made from.
#[derive(Clone)]
pub enum Source {
    Photo(Arc<RgbaImage>),
    Video { clip: Arc<VideoSource>, settings: VideoLoop },
}

/// The inputs a clip was made from, kept so it can be re-rendered larger.
#[derive(Clone)]
pub struct ClipRecipe {
    pub source: Source,
    pub name: String,
    pub look: Look,
    pub bounce: bool,
    pub crop: CropRect,
    pub scene_id: u64,
    pub mask: Vec<f32>,
    pub mask_size: [u32; 2],
    pub arrows: Vec<Arrow>,
    pub pins: Vec<[f32; 2]>,
    pub speed: f32,
    pub frames: usize,
    pub fps: u32,
}

pub struct Clip {
    pub number: usize,
    pub frames: Vec<RgbaImage>,
    pub textures: Vec<TextureHandle>,
    pub recipe: ClipRecipe,
}

impl Clip {
    pub fn seconds(&self) -> f32 {
        self.frames.len() as f32 / self.recipe.fps as f32
    }
}

pub enum TaskKind {
    Preview(ClipRecipe),
    Export,
    Decode,
}

pub enum TaskResult {
    Frames(Option<Vec<RgbaImage>>),
    Saved(Result<PathBuf, String>),
    Video(Result<VideoSource, String>, String),
}

pub struct Task {
    pub progress: Arc<Progress>,
    pub rx: Receiver<TaskResult>,
    pub kind: TaskKind,
    pub label: String,
}

/// One opened photo, video or photo sequence in the Files panel. The active
/// file's scene lives in `StudioApp::scene`; the others are kept here with
/// their painted areas and arrows.
pub struct FileEntry {
    pub name: String,
    pub detail: String,
    pub is_video: bool,
    pub thumb: TextureHandle,
    pub scene: Option<Scene>,
}

pub struct StudioApp {
    /// The file being edited (taken out of `files[active_file]`).
    pub scene: Option<Scene>,
    pub files: Vec<FileEntry>,
    pub active_file: Option<usize>,
    /// Interface choices kept between sessions.
    pub prefs: crate::settings::Prefs,
    prefs_saved: crate::settings::Prefs,
    /// Caption on the output, its preview texture and the fonts to choose from.
    pub caption: crate::caption::Caption,
    pub caption_tex: Option<TextureHandle>,
    caption_key: String,
    pub fonts_added: Vec<crate::fonts::FontEntry>,
    pub fonts_installed: Vec<crate::fonts::FontEntry>,
    pub font_filter: String,
    /// Font names drawn in their own font for the font list; `None` = no preview.
    pub font_previews: std::collections::HashMap<String, Option<TextureHandle>>,
    /// Several photos picked at once, waiting for "separate or one clip".
    pub pending_open: Option<Vec<PathBuf>>,
    /// Files waiting to be opened one after another.
    pub open_queue: Vec<PathBuf>,
    /// A message shown in a dialog until the person closes it.
    pub error: Option<String>,
    pub photo_tex: Option<TextureHandle>,
    pub mask_tex: Option<TextureHandle>,
    pub mask_tex_version: u64,

    pub tool: Tool,
    pub view: View,
    pub brush_radius: f32,
    pub brush_hardness: f32,
    pub show_mask: bool,
    pub show_hints: bool,
    pub zoom: f32,
    pub pan: egui::Vec2,
    pub stroke_last: Option<[f32; 2]>,
    pub arrow_start: Option<[f32; 2]>,
    pub rect_start: Option<[f32; 2]>,

    /// Video clips: true keeps the painted area frozen, false plays only it.
    pub freeze_painted: bool,
    pub crossfade_seconds: f32,
    pub video_seconds: f32,
    /// Video playback.
    pub video_speed: f32,
    pub stabilize: bool,
    /// Play the clip on the canvas while editing the area.
    pub live: bool,
    pub live_pos: f32,
    pub live_tex: Option<TextureHandle>,

    /// Finishing, for photos and videos.
    pub bounce: bool,
    pub look: Look,
    pub crop_drag: Option<CropDrag>,
    pub compare: Compare,
    /// Eyedropper: waiting for a click on the picture.
    pub picking: Option<PickTarget>,
    /// The still as shown (with the look), kept for the eyedropper.
    pub shown_still: Option<RgbaImage>,
    /// Main colors of the shown picture, for the color swatches.
    pub palette: Vec<[u8; 3]>,
    /// Divider position for the split view, 0 (left) to 1 (right).
    pub split: f32,
    /// The untouched still, for comparing.
    pub raw_tex: Option<TextureHandle>,
    pub border: crate::border::Border,
    /// Arrows always get this length (fraction of the picture width).
    pub arrow_fixed: bool,
    pub arrow_len: f32,
    pub crop_before: Option<CropRect>,
    photo_look: Option<Look>,

    pub speed: f32,
    pub loop_seconds: f32,
    pub fps: u32,
    pub auto_angle: f32,

    pub clips: Vec<Clip>,
    pub active_clip: Option<usize>,
    pub playing: bool,
    pub play_pos: f32,
    pub task: Option<Task>,
    /// Loops removed by the last delete, with their old positions.
    pub last_deleted: Vec<(usize, Clip)>,

    pub export_format: Format,
    pub export_size: u32,
    pub ffmpeg: bool,
    /// How many times a saved video plays the loop.
    pub video_repeats: u32,
    pub last_export: Option<PathBuf>,
    pub status: String,


    screenshot: Option<PathBuf>,
    frame_count: u64,
    screenshot_sent: bool,
}

impl StudioApp {
    pub fn new(cc: &eframe::CreationContext<'_>, screenshot: Option<PathBuf>, open: Vec<PathBuf>) -> Self {
        theme::apply(&cc.egui_ctx);
        egui_extras::install_image_loaders(&cc.egui_ctx);
        let mut app = Self {
            scene: None,
            files: Vec::new(),
            active_file: None,
            prefs: crate::settings::Prefs::default(),
            prefs_saved: crate::settings::Prefs::default(),
            caption: crate::caption::Caption::default(),
            caption_tex: None,
            caption_key: String::new(),
            fonts_added: crate::fonts::added_fonts(),
            fonts_installed: crate::fonts::installed_fonts(),
            font_filter: String::new(),
            font_previews: std::collections::HashMap::new(),
            pending_open: None,
            open_queue: Vec::new(),
            error: None,
            photo_tex: None,
            mask_tex: None,
            mask_tex_version: 0,
            tool: Tool::Brush,
            view: View::Edit,
            brush_radius: 28.0,
            brush_hardness: 0.5,
            show_mask: true,
            show_hints: true,
            zoom: 1.0,
            pan: egui::Vec2::ZERO,
            stroke_last: None,
            arrow_start: None,
            rect_start: None,
            freeze_painted: true,
            crossfade_seconds: 0.5,
            video_seconds: 3.0,
            video_speed: 1.0,
            stabilize: false,
            live: true,
            live_pos: 0.0,
            live_tex: None,
            bounce: false,
            look: Look::default(),
            crop_drag: None,
            compare: Compare::Edited,
            picking: None,
            shown_still: None,
            palette: Vec::new(),
            split: 0.5,
            raw_tex: None,
            border: crate::border::Border::default(),
            arrow_fixed: false,
            arrow_len: defaults::ARROW_LEN,
            crop_before: None,
            photo_look: None,
            speed: 1.0,
            loop_seconds: 3.0,
            fps: 24,
            auto_angle: 0.0,
            clips: Vec::new(),
            active_clip: None,
            playing: true,
            play_pos: 0.0,
            task: None,
            last_deleted: Vec::new(),
            export_format: Format::Gif,
            export_size: 1024,
            ffmpeg: crate::export::video_available(),
            video_repeats: 3,
            last_export: None,
            status: "Open a photo, or try the sample scene.".into(),
            screenshot,
            frame_count: 0,
            screenshot_sent: false,
        };
        app.prefs = crate::settings::Prefs::load();
        app.prefs_saved = app.prefs.clone();
        if !app.prefs.caption_font.is_empty() {
            let path = PathBuf::from(&app.prefs.caption_font);
            if let Some(f) = app.fonts_added.iter().chain(app.fonts_installed.iter()).find(|f| f.path.as_ref() == Some(&path)) {
                app.caption.font = f.clone();
            }
        }
        if !open.is_empty() {
            app.load_paths(&cc.egui_ctx, open);
        } else if app.screenshot.is_some() {
            app.load_sample(&cc.egui_ctx);
        }
        if app.screenshot.is_some() && app.scene.is_some() {
            app.start_preview(&cc.egui_ctx);
        }
        app
    }

    // ----- Loading -------------------------------------------------------

    /// Add a newly opened file to the Files panel and switch to it.
    pub fn set_scene(&mut self, ctx: &egui::Context, scene: Scene) {
        self.stash_active();
        let thumb_img = crate::scene::downscale(&scene.work, 160);
        let thumb = ctx.load_texture(
            format!("thumb_{}_{}", self.files.len(), scene.name),
            egui::ColorImage::from_rgba_unmultiplied(
                [thumb_img.width() as usize, thumb_img.height() as usize],
                thumb_img.as_raw(),
            ),
            TextureOptions::LINEAR,
        );
        let detail = match &scene.video {
            Some(v) => format!("Video · {} frames · {:.1}s", v.frames.len(), v.seconds()),
            None => format!("Photo · {}×{}", scene.full.width(), scene.full.height()),
        };
        self.files.push(FileEntry {
            name: scene.name.clone(),
            detail,
            is_video: scene.is_video(),
            thumb,
            scene: None,
        });
        self.active_file = Some(self.files.len() - 1);
        self.show_scene(ctx, scene);
    }

    /// Put the active scene back into its Files entry.
    fn stash_active(&mut self) {
        if let (Some(i), Some(s)) = (self.active_file, self.scene.take()) {
            if let Some(entry) = self.files.get_mut(i) {
                entry.scene = Some(s);
            }
        }
    }

    /// Switch to another opened file, keeping each file's edits.
    pub fn select_file(&mut self, ctx: &egui::Context, index: usize) {
        if self.active_file == Some(index) || index >= self.files.len() {
            return;
        }
        self.stash_active();
        if let Some(scene) = self.files[index].scene.take() {
            self.active_file = Some(index);
            self.status = format!("Working on {}.", scene.name);
            self.show_scene(ctx, scene);
        }
    }

    /// Drop a file from the Files panel. Loops made from it stay in Results.
    pub fn remove_file(&mut self, ctx: &egui::Context, index: usize) {
        if index >= self.files.len() {
            return;
        }
        let name = self.files[index].name.clone();
        let was_active = self.active_file == Some(index);
        if was_active {
            self.scene = None;
            self.active_file = None;
        }
        self.files.remove(index);
        if let Some(a) = self.active_file {
            if a > index {
                self.active_file = Some(a - 1);
            }
        }
        if was_active {
            self.photo_tex = None;
            self.mask_tex = None;
            self.view = View::Edit;
            self.active_clip = None;
            if !self.files.is_empty() {
                let next = index.min(self.files.len() - 1);
                if let Some(scene) = self.files[next].scene.take() {
                    self.active_file = Some(next);
                    self.show_scene(ctx, scene);
                }
            }
        }
        self.status = format!("Removed {name}. Loops already made stay in Results.");
    }

    pub fn remove_all_files(&mut self) {
        self.files.clear();
        self.active_file = None;
        self.scene = None;
        self.photo_tex = None;
        self.mask_tex = None;
        self.view = View::Edit;
        self.active_clip = None;
        self.status = "All files removed. Loops already made stay in Results.".into();
    }

    /// Show a scene on the canvas.
    fn show_scene(&mut self, _ctx: &egui::Context, scene: Scene) {
        self.photo_look = None;
        self.live_pos = 0.0;
        self.mask_tex = None;
        self.mask_tex_version = 0;
        self.scene = Some(scene);
        self.zoom = 1.0;
        self.pan = egui::Vec2::ZERO;
        self.view = View::Edit;
        self.active_clip = None;
        self.arrow_start = None;
        self.rect_start = None;
    }

    pub fn load_sample(&mut self, ctx: &egui::Context) {
        let img = crate::sample::landscape(1280, 720);
        let mut scene = Scene::new("Sample lake".into(), img);
        let [w, h] = scene.work_size();
        scene.set_mask(crate::sample::sample_mask(w, h));
        let (arrows, pins) = crate::sample::sample_arrows();
        scene.arrows = arrows;
        scene.pins = pins;
        self.set_scene(ctx, scene);
        self.status = "Sample loaded. Press Animate to see it move.".into();
    }

    pub fn open_photo_dialog(&mut self, ctx: &egui::Context) {
        let picked = rfd::FileDialog::new()
            .set_title("Open a photo, a short video, or several photos")
            .add_filter(
                "Photos and videos",
                &[
                    "jpg", "jpeg", "png", "webp", "bmp", "tif", "tiff", "gif", "mp4", "mov", "m4v", "webm",
                    "avi", "mkv",
                ],
            )
            .pick_files();
        if let Some(paths) = picked {
            self.load_paths(ctx, paths);
        }
    }

    /// One photo, one video/GIF, or several photos played as a clip.
    pub fn load_paths(&mut self, ctx: &egui::Context, paths: Vec<PathBuf>) {
        let Some(first) = paths.first().cloned() else { return };
        let name = first
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "Clip".into());
        let seconds = self.video_seconds;
        let fps = self.fps;
        let all_stills = paths
            .iter()
            .all(|p| !crate::video::is_video_file(p) && !crate::video::is_gif(p));
        if paths.len() > 1 && !all_stills {
            // A mix of videos and photos: open each as its own file, one at a time.
            self.open_queue.extend(paths);
        } else if paths.len() > 1 {
            self.pending_open = Some(paths);
        } else if crate::video::is_video_file(&first) {
            if self.task.is_some() {
                // Read it once the current job is done.
                self.open_queue.push(first);
                return;
            }
            self.start_decode(ctx, name, move || crate::video::decode_video(&first, seconds, fps));
        } else if crate::video::is_gif(&first) {
            let path = first.clone();
            match crate::video::decode_gif(&path, seconds) {
                Ok(v) => self.set_video(ctx, name, v),
                // A single-frame GIF is just a photo.
                Err(_) => self.load_photo(ctx, &first),
            }
        } else {
            self.load_photo(ctx, &first);
        }
    }

    /// Several photos as one clip (a burst played in name order).
    pub fn open_as_sequence(&mut self, ctx: &egui::Context, paths: Vec<PathBuf>) {
        let fps = self.fps;
        let label = format!("{} photos", paths.len());
        self.start_decode(ctx, label, move || crate::video::from_images(&paths, fps));
    }

    fn error_dialog(&mut self, ctx: &egui::Context) {
        let Some(msg) = self.error.clone() else { return };
        let mut close = false;
        let modal = egui::Modal::new(egui::Id::unique("error_dialog")).show(ctx, |ui| {
            ui.set_width(380.0);
            ui.label(egui::RichText::new("Error").size(16.0).strong());
            ui.label(egui::RichText::new(&msg).color(theme::TEXT_DIM));
            ui.add_space(6.0);
            let ok = egui::Button::new(egui::RichText::new("OK").color(Color32::WHITE).strong())
                .fill(theme::ACCENT)
                .min_size(egui::vec2(90.0, 30.0));
            if ui.add(ok).clicked() {
                close = true;
            }
        });
        if close || modal.should_close() {
            self.error = None;
        }
    }

    fn pending_open_dialog(&mut self, ctx: &egui::Context) {
        let Some(paths) = self.pending_open.clone() else { return };
        let mut choice = None;
        let modal = egui::Modal::new(egui::Id::unique("open_choice")).show(ctx, |ui| {
            ui.set_width(360.0);
            ui.label(egui::RichText::new(format!("Open {} photos", paths.len())).size(16.0).strong());
            ui.label(
                egui::RichText::new(
                    "Add them as separate files, or play them in name order as one short clip (for example a burst).",
                )
                .color(theme::TEXT_DIM),
            );
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                let sep = egui::Button::new(egui::RichText::new("Separate files").color(Color32::WHITE).strong())
                    .fill(theme::ACCENT)
                    .min_size(egui::vec2(130.0, 32.0));
                if ui.add(sep).clicked() {
                    choice = Some(true);
                }
                if ui.add(egui::Button::new("One clip").min_size(egui::vec2(110.0, 32.0))).clicked() {
                    choice = Some(false);
                }
                if ui.button("Cancel").clicked() {
                    self.pending_open = None;
                }
            });
        });
        if modal.should_close() {
            self.pending_open = None;
        }
        match choice {
            Some(true) => {
                self.pending_open = None;
                for p in paths {
                    self.load_photo(ctx, &p);
                }
            }
            Some(false) => {
                self.pending_open = None;
                self.open_as_sequence(ctx, paths);
            }
            None => {}
        }
    }

    fn start_decode(
        &mut self,
        ctx: &egui::Context,
        name: String,
        work: impl FnOnce() -> Result<VideoSource, String> + Send + 'static,
    ) {
        let progress = Arc::new(Progress::new());
        let (tx, rx) = channel();
        let ctx2 = ctx.clone();
        let n2 = name.clone();
        std::thread::spawn(move || {
            let _ = tx.send(TaskResult::Video(work(), n2));
            ctx2.request_repaint();
        });
        self.task = Some(Task {
            progress,
            rx,
            kind: TaskKind::Decode,
            label: format!("Reading {name}"),
        });
    }

    pub fn set_video(&mut self, ctx: &egui::Context, name: String, video: VideoSource) {
        let secs = video.seconds();
        let n = video.frames.len();
        self.set_scene(ctx, Scene::from_video(name.clone(), video));
        self.tool = Tool::Rect;
        self.status = format!(
            "Loaded {name} ({n} frames, {secs:.1}s). Drag a box or paint the area to freeze, then press Make loop."
        );
    }

    /// Choose the frozen frame of a video and show it on the canvas.
    pub fn set_still(&mut self, ctx: &egui::Context, index: usize) {
        let Some(scene) = &mut self.scene else { return };
        scene.set_still(index);
        self.photo_look = None;
        self.refresh_photo_texture(ctx);
    }

    /// Set the crop of the current file; loops made from it follow.
    pub fn set_crop(&mut self, rect: CropRect) {
        let Some(s) = &mut self.scene else { return };
        s.crop = rect;
        let id = s.id;
        for c in self.clips.iter_mut().filter(|c| c.recipe.scene_id == id) {
            c.recipe.crop = rect;
        }
    }

    /// Choose an aspect ratio; the crop box takes that shape at once.
    pub fn set_aspect(&mut self, aspect: crate::crop::Aspect) {
        let Some(s) = &mut self.scene else { return };
        s.checkpoint();
        s.aspect = aspect;
        let [w, h] = [s.full.width(), s.full.height()];
        let rect = match aspect.ratio(w, h) {
            None => s.crop,
            Some(r) if s.crop.is_full() => CropRect::largest(r, [0.5, 0.5], w, h),
            Some(_) => s.crop.with_aspect(aspect, w, h),
        };
        self.set_crop(rect);
        self.pick_tool(Tool::Crop);
    }

    /// Trim range of the current video (start, end), always at least 2 frames.
    pub fn trim_range(&self) -> [usize; 2] {
        let Some(s) = &self.scene else { return [0, 0] };
        let n = s.video.as_ref().map_or(1, |v| v.frames.len());
        let end = s.trim[1].min(n - 1).max(1);
        let start = s.trim[0].min(end - 1);
        [start, end]
    }

    /// The still shown on the canvas, with the current look applied.
    pub fn refresh_photo_texture(&mut self, ctx: &egui::Context) {
        let Some(scene) = &self.scene else { return };
        if self.photo_tex.is_some() && self.photo_look == Some(self.look) {
            return;
        }
        if self.photo_look.is_none() || self.raw_tex.is_none() {
            let raw = &scene.work;
            let c = egui::ColorImage::from_rgba_unmultiplied([raw.width() as usize, raw.height() as usize], raw.as_raw());
            match &mut self.raw_tex {
                Some(t) => t.set(c, TextureOptions::LINEAR),
                None => self.raw_tex = Some(ctx.load_texture("original", c, TextureOptions::LINEAR)),
            }
        }
        let mut img = scene.work.clone();
        crate::finish::apply_look(&mut img, &self.look);
        let color = egui::ColorImage::from_rgba_unmultiplied([img.width() as usize, img.height() as usize], img.as_raw());
        match &mut self.photo_tex {
            Some(t) => t.set(color, TextureOptions::LINEAR),
            None => self.photo_tex = Some(ctx.load_texture("photo", color, TextureOptions::LINEAR)),
        }
        self.palette = crate::palette::dominant_colors(&img, 3);
        self.shown_still = Some(img);
        self.photo_look = Some(self.look);
    }

    /// Advance and redraw the live preview of a video while editing.
    pub fn update_live(&mut self, ctx: &egui::Context, dt: f32) -> bool {
        let Some(scene) = &self.scene else { return false };
        let Some(video) = &scene.video else { return false };
        if !self.live {
            return false;
        }
        let [start, end] = self.trim_range();
        let span = (end - start + 1) as f32;
        // live_pos is the phase within one cycle; a bounce cycle plays forward then back.
        let cycle = if self.bounce { (2.0 * span - 2.0).max(1.0) } else { span };
        if self.playing {
            self.live_pos = (self.live_pos + dt * video.fps * self.video_speed).rem_euclid(cycle);
        }
        let phase = self.live_pos.rem_euclid(cycle);
        let rel = if phase < span { phase } else { cycle - phase };
        let idx = (start + rel as usize).min(end);
        let mut frame = crate::video::live_frame(&video.work[idx], &video.work[scene.still_index], &scene.mask, self.freeze_painted);
        crate::finish::apply_look(&mut frame, &self.look);
        let color = egui::ColorImage::from_rgba_unmultiplied([frame.width() as usize, frame.height() as usize], frame.as_raw());
        match &mut self.live_tex {
            Some(t) => t.set(color, TextureOptions::LINEAR),
            None => self.live_tex = Some(ctx.load_texture("live", color, TextureOptions::LINEAR)),
        }
        if self.playing {
            ctx.request_repaint();
        }
        true
    }

    /// Rotate the current file a quarter turn and refresh what shows it.
    pub fn rotate_current(&mut self, ctx: &egui::Context, clockwise: bool) {
        let Some(scene) = &mut self.scene else { return };
        scene.rotate(clockwise);
        self.after_turn(ctx);
        self.status = format!("Rotated {}.", if clockwise { "right" } else { "left" });
    }

    /// Switch between edited, original and split views. Looking at the
    /// original or the split picks the pointer, so clicks don't paint.
    pub fn set_compare(&mut self, mode: Compare) {
        self.compare = mode;
        if mode != Compare::Edited {
            self.pick_tool(Tool::Select);
        }
    }

    /// Undo the last step of the current file (painting, arrows, crop, rotation).
    pub fn undo(&mut self, ctx: &egui::Context) {
        let Some(scene) = &mut self.scene else { return };
        let turned = scene.undo();
        self.after_undo_redo(ctx, turned);
    }

    pub fn redo(&mut self, ctx: &egui::Context) {
        let Some(scene) = &mut self.scene else { return };
        let turned = scene.redo();
        self.after_undo_redo(ctx, turned);
    }

    fn after_undo_redo(&mut self, ctx: &egui::Context, turned: bool) {
        if turned {
            self.after_turn(ctx);
        }
        // Loops made from this file follow its crop.
        if let Some(c) = self.scene.as_ref().map(|s| s.crop) {
            self.set_crop(c);
        }
        self.crop_drag = None;
    }

    /// Redraw everything that shows the picture after a quarter turn.
    fn after_turn(&mut self, ctx: &egui::Context) {
        let Some(scene) = &self.scene else { return };
        let thumb_img = crate::scene::downscale(&scene.work, 160);
        let color = egui::ColorImage::from_rgba_unmultiplied(
            [thumb_img.width() as usize, thumb_img.height() as usize],
            thumb_img.as_raw(),
        );
        let detail = match &scene.video {
            Some(v) => format!("Video · {} frames · {:.1}s", v.frames.len(), v.seconds()),
            None => format!("Photo · {}×{}", scene.full.width(), scene.full.height()),
        };
        if let Some(entry) = self.active_file.and_then(|i| self.files.get_mut(i)) {
            entry.thumb.set(color, TextureOptions::LINEAR);
            entry.detail = detail;
        }
        self.photo_look = None;
        self.live_tex = None;
        self.mask_tex = None;
        self.zoom = 1.0;
        self.pan = egui::Vec2::ZERO;
        self.view = View::Edit;
        self.refresh_photo_texture(ctx);
    }

    pub fn load_photo(&mut self, ctx: &egui::Context, path: &std::path::Path) {
        match open_upright(path) {
            Ok(img) => {
                let name = path
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| "Photo".into());
                self.set_scene(ctx, Scene::new(name.clone(), img.to_rgba8()));
                self.status = format!(
                    "Loaded {name}. Paint the part that should move, or use Auto-detect."
                );
            }
            Err(e) => self.status = format!("Could not open that file: {e}"),
        }
    }

    // ----- Jobs ----------------------------------------------------------

    pub fn busy(&self) -> bool {
        self.task.is_some()
    }

    pub fn current_recipe(&self) -> Option<ClipRecipe> {
        let s = self.scene.as_ref()?;
        let (source, frames, fps) = match &s.video {
            Some(v) => {
                let [start, end] = self.trim_range();
                let used = end - start + 1;
                let crossfade = if self.bounce { 0 } else { (self.crossfade_seconds * v.fps).round() as usize };
                let settings = VideoLoop {
                    freeze_painted: self.freeze_painted,
                    still_index: s.still_index,
                    crossfade: crossfade.min(used / 2),
                    start,
                    end,
                    stabilize: self.stabilize,
                };
                let frames = used - settings.crossfade;
                let fps = (v.fps * self.video_speed).round().clamp(2.0, 50.0) as u32;
                (Source::Video { clip: v.clone(), settings }, frames, fps)
            }
            None => (
                Source::Photo(Arc::new(s.full.clone())),
                ((self.loop_seconds * self.fps as f32).round() as usize).max(4),
                self.fps,
            ),
        };
        Some(ClipRecipe {
            source,
            name: s.name.clone(),
            look: self.look,
            bounce: self.bounce,
            crop: s.crop,
            scene_id: s.id,
            mask: s.mask.clone(),
            mask_size: s.work_size(),
            arrows: s.arrows.clone(),
            pins: s.pins.clone(),
            speed: self.speed,
            frames,
            fps,
        })
    }

    pub fn start_preview(&mut self, ctx: &egui::Context) {
        let (Some(scene), Some(recipe)) = (&self.scene, self.current_recipe()) else {
            return;
        };
        if let Source::Video { clip, settings } = &recipe.source {
            let (clip, settings, mask) = (clip.clone(), *settings, recipe.mask.clone());
            let (look, bounce) = (recipe.look, recipe.bounce);
            let progress = Arc::new(Progress::new());
            let (tx, rx) = channel();
            let p = progress.clone();
            let ctx2 = ctx.clone();
            std::thread::spawn(move || {
                let out = crate::video::make_loop(&clip.work, &clip.work, &mask, settings, &p)
                    .map(|f| crate::finish::finish(f, &look, bounce));
                let _ = tx.send(TaskResult::Frames(out));
                ctx2.request_repaint();
            });
            self.task = Some(Task {
                progress,
                rx,
                kind: TaskKind::Preview(recipe),
                label: "Making loop".into(),
            });
            return;
        }
        if recipe.arrows.is_empty() {
            self.status = "Draw at least one motion arrow first.".into();
            return;
        }
        let job = RenderJob {
            photo: scene.work.clone(),
            mask: recipe.mask.clone(),
            arrows: recipe.arrows.clone(),
            pins: recipe.pins.clone(),
            speed: recipe.speed,
            frames: recipe.frames,
        };
        let (look, bounce) = (recipe.look, recipe.bounce);
        let progress = Arc::new(Progress::new());
        let (tx, rx) = channel();
        let p = progress.clone();
        let ctx2 = ctx.clone();
        std::thread::spawn(move || {
            let out = crate::render::render_loop(&job, &p).map(|f| crate::finish::finish(f, &look, bounce));
            let _ = tx.send(TaskResult::Frames(out));
            ctx2.request_repaint();
        });
        self.task = Some(Task {
            progress,
            rx,
            kind: TaskKind::Preview(recipe),
            label: "Animating".into(),
        });
    }

    /// Can the chosen export format be saved right now?
    pub fn can_export(&self) -> bool {
        if self.busy() {
            return false;
        }
        if self.export_format.is_still() {
            self.scene.is_some()
        } else {
            self.active_clip.is_some()
        }
    }

    /// Save the still picture with look, crop, caption and border.
    fn export_still(&mut self) {
        let Some(scene) = &self.scene else { return };
        let format = self.export_format;
        let base = scene.name.rsplit_once('.').map(|(a, _)| a.to_string()).unwrap_or_else(|| scene.name.clone());
        let ext = format.extension().unwrap_or("png");
        let Some(path) = rfd::FileDialog::new()
            .set_title("Save image")
            .set_file_name(format!("{base}_still.{ext}"))
            .add_filter(format.label(), &[ext])
            .save_file()
        else {
            return;
        };
        let photo = if self.export_size == 0 {
            scene.full.clone()
        } else {
            crate::scene::downscale(&scene.full, self.export_size)
        };
        let (look, crop, border, caption) = (self.look, scene.crop, self.border, self.caption.clone());
        let progress = Arc::new(Progress::new());
        let (tx, rx) = channel();
        let path2 = path.clone();
        std::thread::spawn(move || {
            let mut img = photo;
            crate::finish::apply_look(&mut img, &look);
            let img = finish_frame(&img, crop, &caption_overlay(&caption, crop, &img), border);
            let result = crate::export::save_still(format, &img, &path2);
            let _ = tx.send(TaskResult::Saved(result));
        });
        self.task = Some(Task { progress, rx, kind: TaskKind::Export, label: "Saving image".into() });
    }

    pub fn start_export(&mut self) {
        if self.export_format.is_still() {
            self.export_still();
            return;
        }
        let Some(idx) = self.active_clip else { return };
        let format = self.export_format;
        let clip_name = self.clips[idx].recipe.name.clone();
        let base = clip_name
            .rsplit_once('.')
            .map(|(a, _)| a.to_string())
            .unwrap_or_else(|| clip_name.clone());
        let default_name = match format.extension() {
            Some(ext) => format!("{base}_animagraph.{ext}"),
            None => format!("{base}_animagraph_frames"),
        };
        let mut dialog = rfd::FileDialog::new()
            .set_title("Save loop")
            .set_file_name(&default_name);
        if let Some(ext) = format.extension() {
            dialog = dialog.add_filter(format.label(), &[ext]);
        }
        let Some(path) = dialog.save_file() else { return };

        let clip = &self.clips[idx];
        let recipe = clip.recipe.clone();
        let fps = recipe.fps;
        let export_size = self.export_size;
        let fit = move |img: &RgbaImage| {
            if export_size == 0 { img.clone() } else { crate::scene::downscale(img, export_size) }
        };
        let preview_dims = clip.frames[0].dimensions();
        let preview_frames = clip.frames.clone();
        let crop = recipe.crop;
        let border = self.border;
        let caption = self.caption.clone();
        let repeats = self.video_repeats;
        let (look, bounce) = (recipe.look, recipe.bounce);
        let progress = Arc::new(Progress::new());
        let (tx, rx) = channel();
        let p = progress.clone();
        let path2 = path.clone();
        std::thread::spawn(move || {
            let frames = match &recipe.source {
                Source::Photo(full) => {
                    let photo = fit(full);
                    let size = [photo.width(), photo.height()];
                    if (size[0], size[1]) == preview_dims {
                        Some(preview_frames)
                    } else {
                        let mask = crate::scene::resize_mask(&recipe.mask, recipe.mask_size, size);
                        let job = RenderJob {
                            photo,
                            mask,
                            arrows: recipe.arrows.clone(),
                            pins: recipe.pins.clone(),
                            speed: recipe.speed * size[0] as f32 / recipe.mask_size[0] as f32,
                            frames: recipe.frames,
                        };
                        crate::render::render_loop(&job, &p).map(|f| crate::finish::finish(f, &look, bounce))
                    }
                }
                Source::Video { clip, settings } => {
                    let frames: Vec<RgbaImage> = clip.frames.iter().map(&fit).collect();
                    let (w, h) = frames[0].dimensions();
                    if (w, h) == preview_dims {
                        Some(preview_frames)
                    } else {
                        let mask = crate::scene::resize_mask(&recipe.mask, recipe.mask_size, [w, h]);
                        crate::video::make_loop(&frames, &clip.work, &mask, *settings, &p)
                            .map(|f| crate::finish::finish(f, &look, bounce))
                    }
                }
            };
            let frames = frames.map(|f| {
                let overlay = f.first().and_then(|x| caption_overlay(&caption, crop, x));
                if crop.is_full() && !border.enabled && overlay.is_none() {
                    f
                } else {
                    f.iter().map(|x| finish_frame(x, crop, &overlay, border)).collect()
                }
            });
            let result = match frames {
                Some(f) => crate::export::save(format, &f, fps, &path2, repeats),
                None => Err("Canceled".into()),
            };
            let _ = tx.send(TaskResult::Saved(result));
        });
        self.task = Some(Task {
            progress,
            rx,
            kind: TaskKind::Export,
            label: format!("Saving {}", format.label()),
        });
    }

    pub fn cancel_task(&mut self) {
        if let Some(t) = &self.task {
            t.progress.cancel.store(true, std::sync::atomic::Ordering::Relaxed);
        }
    }

    fn poll_task(&mut self, ctx: &egui::Context) {
        let Some(task) = &self.task else { return };
        ctx.request_repaint_after(std::time::Duration::from_millis(50));
        let Ok(result) = task.rx.try_recv() else { return };
        let task = self.task.take().expect("task present");
        match (result, task.kind) {
            (TaskResult::Frames(Some(frames)), TaskKind::Preview(recipe)) => {
                let number = self.clips.iter().map(|c| c.number).max().unwrap_or(0) + 1;
                let textures = frames
                    .iter()
                    .enumerate()
                    .map(|(i, f)| {
                        let c = egui::ColorImage::from_rgba_unmultiplied(
                            [f.width() as usize, f.height() as usize],
                            f.as_raw(),
                        );
                        ctx.load_texture(format!("clip{number}_{i}"), c, TextureOptions::LINEAR)
                    })
                    .collect();
                self.clips.push(Clip { number, frames, textures, recipe });
                self.active_clip = Some(self.clips.len() - 1);
                self.view = View::Preview;
                self.playing = true;
                self.play_pos = 0.0;
                self.status = format!("Loop {number} is ready. Export it, or adjust and animate again.");
            }
            (TaskResult::Frames(None), _) => self.status = "Animation canceled.".into(),
            (TaskResult::Saved(Ok(path)), TaskKind::Export) => {
                self.status = format!("Saved {}", path.display());
                self.last_export = Some(path);
            }
            (TaskResult::Saved(Err(e)), _) => self.status = format!("Saving failed: {e}"),
            (TaskResult::Video(Ok(v), name), _) => self.set_video(ctx, name, v),
            (TaskResult::Video(Err(e), name), _) => {
                self.status = format!("Could not read {name}.");
                self.error = Some(format!("Could not open {name}.\n\n{e}"));
            }
            _ => {}
        }
    }

    /// Remove a loop from Results. Loops live only in memory (nothing is
    /// on disk until exported), so this frees memory; the last deletion can
    /// be restored.
    pub fn delete_clip(&mut self, idx: usize) {
        if idx >= self.clips.len() {
            return;
        }
        let was_active = self.active_clip == Some(idx);
        let clip = self.clips.remove(idx);
        let number = clip.number;
        self.last_deleted = vec![(idx, clip)];
        self.active_clip = match self.active_clip {
            // Select the neighbour so Delete can be pressed again.
            Some(a) if a == idx => (!self.clips.is_empty()).then(|| idx.min(self.clips.len() - 1)),
            Some(a) if a > idx => Some(a - 1),
            other => other,
        };
        if was_active {
            self.play_pos = 0.0;
        }
        if self.active_clip.is_none() {
            self.view = View::Edit;
        }
        self.status = format!("Deleted Loop {number}. Exported files are not touched.");
    }

    pub fn delete_all_clips(&mut self) {
        if self.clips.is_empty() {
            return;
        }
        let n = self.clips.len();
        self.last_deleted = self.clips.drain(..).enumerate().collect();
        self.active_clip = None;
        self.view = View::Edit;
        self.status = format!("Deleted {n} loops. Exported files are not touched.");
    }

    /// Put back the loops removed by the last delete.
    pub fn restore_deleted(&mut self) {
        let items = std::mem::take(&mut self.last_deleted);
        let mut last = None;
        for (idx, clip) in items {
            let at = idx.min(self.clips.len());
            self.clips.insert(at, clip);
            last = Some(at);
        }
        if let Some(at) = last {
            self.active_clip = Some(at);
            self.view = View::Preview;
            self.status = "Restored.".into();
        }
    }

    // ----- Mask overlay --------------------------------------------------

    pub fn refresh_mask_texture(&mut self, ctx: &egui::Context) {
        let Some(scene) = &self.scene else { return };
        if self.mask_tex.is_some() && self.mask_tex_version == scene.mask_version {
            return;
        }
        let [w, h] = scene.work_size();
        let t = theme::MASK_TINT;
        let pixels: Vec<Color32> = scene
            .mask
            .iter()
            .map(|m| {
                let a = (m.clamp(0.0, 1.0) * 120.0) as u8;
                Color32::from_rgba_unmultiplied(t.r(), t.g(), t.b(), a)
            })
            .collect();
        let img = egui::ColorImage::new([w as usize, h as usize], pixels);
        match &mut self.mask_tex {
            Some(tex) => tex.set(img, TextureOptions::LINEAR),
            None => self.mask_tex = Some(ctx.load_texture("mask", img, TextureOptions::LINEAR)),
        }
        self.mask_tex_version = scene.mask_version;
    }

    // ----- Keyboard ------------------------------------------------------

    fn shortcuts(&mut self, ctx: &egui::Context) {
        // Only a text field should swallow the keys. Clicking the canvas, a
        // button or a slider also gives it focus, which must not turn
        // undo and the tool keys off.
        let typing = ctx
            .memory(|m| m.focused())
            .is_some_and(|id| egui::text_edit::TextEditState::load(ctx, id).is_some());
        if typing {
            return;
        }
        if self.tool == Tool::Crop && ctx.input(|i| i.key_pressed(egui::Key::Enter)) {
            self.finish_crop();
        }
        let (rot_left, rot_right) =
            ctx.input(|i| (i.key_pressed(egui::Key::OpenBracket), i.key_pressed(egui::Key::CloseBracket)));
        if (rot_left || rot_right) && self.task.is_none() {
            self.rotate_current(ctx, rot_right);
        }
        let delete = ctx.input(|i| i.key_pressed(egui::Key::Delete) || i.key_pressed(egui::Key::Backspace));
        if delete {
            if let Some(i) = self.active_clip {
                self.delete_clip(i);
            }
        }
        let (undo, redo, keys) = ctx.input(|i| {
            let cmd = i.modifiers.command;
            let z = i.key_pressed(egui::Key::Z);
            (
                cmd && z && !i.modifiers.shift,
                cmd && ((z && i.modifiers.shift) || i.key_pressed(egui::Key::Y)),
                [
                    (egui::Key::B, i.key_pressed(egui::Key::B)),
                    (egui::Key::E, i.key_pressed(egui::Key::E)),
                    (egui::Key::A, i.key_pressed(egui::Key::A) && !cmd),
                    (egui::Key::P, i.key_pressed(egui::Key::P)),
                    (egui::Key::R, i.key_pressed(egui::Key::R)),
                    (egui::Key::C, i.key_pressed(egui::Key::C) && !cmd),
                    (egui::Key::H, i.key_pressed(egui::Key::H)),
                    (egui::Key::V, i.key_pressed(egui::Key::V) && !cmd),
                    (egui::Key::Space, i.key_pressed(egui::Key::Space)),
                    (egui::Key::Tab, i.key_pressed(egui::Key::Tab)),
                ],
            )
        });
        if undo {
            self.undo(ctx);
        }
        if redo {
            self.redo(ctx);
        }
        for (key, pressed) in keys {
            if !pressed {
                continue;
            }
            match key {
                egui::Key::B => self.pick_tool(Tool::Brush),
                egui::Key::E => self.pick_tool(Tool::Eraser),
                egui::Key::A => self.pick_tool(Tool::Arrow),
                egui::Key::P => self.pick_tool(Tool::Pin),
                egui::Key::R => self.pick_tool(Tool::Rect),
                egui::Key::C => self.pick_tool(Tool::Crop),
                egui::Key::H => self.pick_tool(Tool::Hand),
                egui::Key::V => self.pick_tool(Tool::Select),
                egui::Key::Space => self.playing = !self.playing,
                egui::Key::Tab => self.toggle_view(),
                _ => {}
            }
        }
    }

    pub fn pick_tool(&mut self, tool: Tool) {
        let was = self.tool;
        self.tool = tool;
        let edits = !matches!(tool, Tool::Hand | Tool::Select);
        if edits {
            self.view = View::Edit;
            // Painting or cropping happens on the edited picture.
            self.compare = Compare::Edited;
        }
        if let Some(s) = &mut self.scene {
            if tool == Tool::Crop {
                // Show the whole picture while choosing the crop.
                s.crop_applied = false;
            } else if was == Tool::Crop {
                s.crop_applied = !s.crop.is_full();
            }
        }
    }

    /// Confirm the crop: hide the cut-away part and go back to painting.
    pub fn finish_crop(&mut self) {
        let next = if self.scene.as_ref().is_some_and(|s| s.is_video()) { Tool::Rect } else { Tool::Brush };
        self.pick_tool(next);
        if let Some(s) = &self.scene {
            self.status = if s.crop.is_full() {
                "No crop: the whole picture is kept.".into()
            } else {
                let [_, _, w, h] = s.crop.pixels(s.full.width(), s.full.height());
                format!("Cropped to {w} × {h} px. Pick the crop tool again to change it.")
            };
        }
    }

    pub fn panel_visible(&self, key: &str) -> bool {
        !self.prefs.hidden.contains(key)
    }

    pub fn set_panel_visible(&mut self, key: &str, visible: bool) {
        if visible {
            self.prefs.hidden.remove(key);
        } else {
            self.prefs.hidden.insert(key.to_string());
        }
    }

    /// Caption preview, drawn over the cropped part of the picture.
    fn refresh_caption_texture(&mut self, ctx: &egui::Context) {
        let Some(scene) = &self.scene else {
            self.caption_tex = None;
            return;
        };
        let [iw, ih] = scene.work_size();
        let [_, _, w, h] = scene.crop.pixels(iw, ih);
        let key = self.caption.key(w, h);
        if key == self.caption_key {
            return;
        }
        self.caption_key = key;
        // An empty box previews a faint placeholder; exports draw nothing.
        let shown = if self.caption.text.trim().is_empty() {
            crate::caption::Caption {
                text: crate::caption::Caption::PLACEHOLDER.into(),
                opacity: self.caption.opacity * 0.4,
                shadow: false,
                ..self.caption.clone()
            }
        } else {
            self.caption.clone()
        };
        self.caption_tex = shown.render(w, h).map(|img| {
            let c = egui::ColorImage::from_rgba_unmultiplied([w as usize, h as usize], img.as_raw());
            ctx.load_texture("caption", c, TextureOptions::LINEAR)
        });
    }

    /// Put every adjustable setting back to its default (edits are kept).
    pub fn reset_settings(&mut self) {
        use defaults::*;
        self.brush_radius = BRUSH_RADIUS;
        self.brush_hardness = BRUSH_HARDNESS;
        self.speed = SPEED;
        self.loop_seconds = LOOP_SECONDS;
        self.fps = FPS;
        self.auto_angle = AUTO_ANGLE;
        self.arrow_len = ARROW_LEN;
        self.arrow_fixed = false;
        self.video_speed = VIDEO_SPEED;
        self.crossfade_seconds = CROSSFADE;
        self.video_seconds = VIDEO_SECONDS;
        self.video_repeats = REPEATS;
        self.stabilize = false;
        self.bounce = false;
        self.look = Look::default();
        self.border = crate::border::Border::default();
        let font = self.caption.font.clone();
        self.caption = crate::caption::Caption { font, ..crate::caption::Caption::default() };
        self.status = "Settings reset to defaults. Your painted areas, arrows and crops are kept.".into();
    }

    pub fn toggle_view(&mut self) {
        self.view = match self.view {
            View::Edit if self.active_clip.is_some() => View::Preview,
            _ => View::Edit,
        };
    }

    fn handle_dropped_files(&mut self, ctx: &egui::Context) {
        let dropped: Vec<PathBuf> =
            ctx.input(|i| i.raw.dropped_files.iter().map(|f| f.path().to_path_buf()).collect());
        if !dropped.is_empty() {
            self.load_paths(ctx, dropped);
        }
    }

    // ----- Screenshot mode ----------------------------------------------

    fn screenshot_tick(&mut self, ctx: &egui::Context) {
        let Some(path) = self.screenshot.clone() else { return };
        self.frame_count += 1;
        ctx.request_repaint();
        let ready = self.task.is_none() || self.frame_count > 900;
        if self.frame_count >= 120 && ready && !self.screenshot_sent {
            self.screenshot_sent = true;
            let ctx2 = ctx.clone();
            ctx.request_screenshot(move |img| {
                let [w, h] = img.size;
                let mut buf = Vec::with_capacity(w * h * 4);
                for p in &img.pixels {
                    buf.extend_from_slice(&p.to_srgba_unmultiplied());
                }
                if let Some(parent) = path.parent() {
                    let _ = std::fs::create_dir_all(parent);
                }
                if let Some(out) = RgbaImage::from_raw(w as u32, h as u32, buf) {
                    if let Err(e) = out.save(&path) {
                        eprintln!("Could not save screenshot: {e}");
                    }
                }
                ctx2.send_viewport_cmd(egui::ViewportCommand::Close);
            });
        }
    }
}

/// The caption drawn at the size of the cropped picture.
fn caption_overlay(
    caption: &crate::caption::Caption,
    crop: CropRect,
    full: &RgbaImage,
) -> Option<RgbaImage> {
    let [_, _, w, h] = crop.pixels(full.width(), full.height());
    caption.render(w, h)
}

/// Crop, then caption, then border: the last steps before saving a frame.
fn finish_frame(
    img: &RgbaImage,
    crop: CropRect,
    caption: &Option<RgbaImage>,
    border: crate::border::Border,
) -> RgbaImage {
    let mut out = crop.apply(img);
    if let Some(c) = caption {
        crate::caption::blend(&mut out, c);
    }
    border.apply(&out)
}

/// Open a photo and turn it upright using the orientation the camera saved.
fn open_upright(path: &std::path::Path) -> image::ImageResult<image::DynamicImage> {
    use image::ImageDecoder;
    let mut decoder = image::ImageReader::open(path)?.with_guessed_format()?.into_decoder()?;
    let orientation = decoder.orientation().unwrap_or(image::metadata::Orientation::NoTransforms);
    let mut img = image::DynamicImage::from_decoder(decoder)?;
    img.apply_orientation(orientation);
    Ok(img)
}

impl eframe::App for StudioApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.poll_task(&ctx);
        self.handle_dropped_files(&ctx);
        self.shortcuts(&ctx);
        self.refresh_mask_texture(&ctx);
        self.refresh_photo_texture(&ctx);
        self.refresh_caption_texture(&ctx);
        self.screenshot_tick(&ctx);
        // Remember interface choices.
        self.prefs.caption_font = self
            .caption
            .font
            .path
            .as_ref()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default();
        if self.prefs != self.prefs_saved {
            self.prefs.save();
            self.prefs_saved = self.prefs.clone();
        }
        self.pending_open_dialog(&ctx);
        self.error_dialog(&ctx);
        if self.task.is_none() && !self.open_queue.is_empty() {
            let next = self.open_queue.remove(0);
            self.load_paths(&ctx, vec![next]);
        }

        egui::Panel::top("top_bar")
            .exact_size(52.0)
            .frame(
                egui::Frame::default()
                    .fill(theme::BG)
                    .inner_margin(egui::Margin::symmetric(14, 8))
                    .stroke(egui::Stroke::new(1.0, theme::BORDER)),
            )
            .show(ui, |ui| crate::panels::top_bar(self, ui));

        if self.panel_visible("controls") {
            egui::Panel::right("controls")
                .exact_size(340.0)
                .resizable(false)
                .frame(
                    egui::Frame::default()
                        .fill(theme::PANEL)
                        .inner_margin(egui::Margin::same(14))
                        .stroke(egui::Stroke::new(1.0, theme::BORDER)),
                )
                .show(ui, |ui| crate::panels::side_panel(self, ui));
        }

        if self.panel_visible("results") {
            egui::Panel::bottom("results")
                .exact_size(118.0)
                .frame(
                    egui::Frame::default()
                        .fill(theme::PANEL)
                        .inner_margin(egui::Margin::symmetric(14, 10))
                        .stroke(egui::Stroke::new(1.0, theme::BORDER)),
                )
                .show(ui, |ui| crate::panels::results_strip(self, ui));
        }

        if self.panel_visible("files") {
            egui::Panel::left("files")
                .exact_size(256.0)
                .resizable(false)
                .frame(
                    egui::Frame::default()
                        .fill(theme::PANEL)
                        .inner_margin(egui::Margin::same(12))
                        .stroke(egui::Stroke::new(1.0, theme::BORDER)),
                )
                .show(ui, |ui| crate::files_panel::files_panel(self, ui));
        }

        egui::CentralPanel::default()
            .frame(egui::Frame::default().fill(theme::BG))
            .show(ui, |ui| crate::canvas::canvas(self, ui));
    }
}
