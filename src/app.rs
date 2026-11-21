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
    /// A developed RAW photo.
    Photo(Result<RgbaImage, String>, String),
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
    /// Files picked with Cmd/Ctrl-click or Shift-click in the Files panel,
    /// for "Apply … to selected files". The current file is never in it.
    pub selected_files: std::collections::BTreeSet<usize>,
    /// Where the last Cmd/Ctrl-click was, for Shift-click ranges.
    pub select_anchor: Option<usize>,
    /// Interface choices kept between sessions.
    pub prefs: crate::settings::Prefs,
    prefs_saved: crate::settings::Prefs,
    /// Caption on the output, its preview texture and the fonts to choose from.
    pub caption: crate::caption::Caption,
    pub caption_tex: Option<TextureHandle>,
    /// The caption preview covers the whole output (it sits in the border).
    pub caption_on_border: bool,
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
    /// Film simulation: last chosen style, its strength, and preview tiles.
    pub film_choice: Option<usize>,
    /// Imported Lightroom presets and the editor's current choices.
    pub lr_presets: Vec<crate::presets::Preset>,
    /// A look shown while the pointer rests on a preset (with its name);
    /// `hover_look` is set during a frame and shown from the next one.
    pub hover_look: Option<(Look, String)>,
    pub look_preview: Option<(Look, String)>,
    pub lr_choice: Option<usize>,
    pub curve_channel: usize,
    pub hsl_band: usize,
    /// The color grading range whose sliders are shown (0 shadows,
    /// 1 midtones, 2 highlights, 3 global).
    pub grade_range: usize,
    pub film_strength: f32,
    /// Preview tiles for Styles, kept until the picture changes.
    pub style_thumbs: std::collections::HashMap<String, TextureHandle>,
    pub style_thumbs_scene: String,
    pub style_thumb_src: Option<RgbaImage>,
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
    /// The look at the last undo step, with the file it belongs to.
    look_committed: Option<(u64, Look)>,
    /// Straighten angle when the current change started (for undo).
    angle_from: Option<f32>,
    /// The straighten slider is being dragged: the canvas shows a grid.
    pub straightening: bool,
    /// The layout of several files (one per session), whether the canvas
    /// shows it, each file's finished picture for its preview, the preview
    /// texture and what it was drawn from, the selected box, and how it is
    /// saved.
    pub layout: Option<crate::layout::Layout>,
    pub layout_open: bool,
    pub layout_photos: std::collections::HashMap<u64, RgbaImage>,
    pub layout_tex: Option<TextureHandle>,
    layout_drawn: Option<crate::layout::Layout>,
    pub layout_cell: Option<usize>,
    pub layout_drag: Option<usize>,
    /// The dragged box as it was when the drag started (put back while the
    /// photo is dragged over another box).
    pub layout_drag_saved: Option<crate::layout::Cell>,
    pub layout_size: u32,
    pub layout_format: Format,
    /// An imported preset waiting for the person to confirm its deletion.
    pub preset_to_delete: Option<crate::presets::Preset>,
    /// Vectorscope trace, what it was drawn from, and its zoom (1 or 2).
    pub scope_tex: Option<TextureHandle>,
    pub scope_key: u64,
    pub scope_zoom: f32,
    /// The still at the chosen view quality, before the look.
    still_base: Option<RgbaImage>,
    /// The view quality `still_base` was made at.
    still_base_size: Option<u32>,
    /// The shown still was drawn quickly and is redrawn sharp when idle.
    photo_fast: bool,

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
            selected_files: std::collections::BTreeSet::new(),
            select_anchor: None,
            active_file: None,
            prefs: crate::settings::Prefs::default(),
            prefs_saved: crate::settings::Prefs::default(),
            caption: crate::caption::Caption::default(),
            caption_tex: None,
            caption_on_border: false,
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
            film_choice: None,
            lr_presets: crate::presets::list(),
            hover_look: None,
            look_preview: None,
            lr_choice: None,
            curve_channel: 0,
            hsl_band: 0,
            grade_range: 0,
            film_strength: 1.0,
            style_thumbs: std::collections::HashMap::new(),
            style_thumbs_scene: String::new(),
            style_thumb_src: None,
            split: 0.5,
            raw_tex: None,
            border: crate::border::Border::default(),
            arrow_fixed: false,
            arrow_len: defaults::ARROW_LEN,
            crop_before: None,
            photo_look: None,
            look_committed: None,
            angle_from: None,
            straightening: false,
            layout: None,
            layout_open: false,
            layout_photos: std::collections::HashMap::new(),
            layout_tex: None,
            layout_drawn: None,
            layout_cell: None,
            layout_drag: None,
            layout_drag_saved: None,
            layout_size: 3000,
            layout_format: if Format::default_export().is_still() { Format::default_export() } else { Format::Png },
            preset_to_delete: None,
            scope_tex: None,
            scope_key: 0,
            scope_zoom: 1.0,
            still_base: None,
            still_base_size: None,
            photo_fast: false,
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
            export_format: Format::default_export(),
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
    pub fn set_scene(&mut self, ctx: &egui::Context, mut scene: Scene) {
        self.stash_active();
        // A new file starts without edits, but keeps the chosen caption font.
        scene.edits.caption.font = self.caption.font.clone();
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
        self.save_edits_to_scene();
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
        self.selected_files.remove(&index);
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
        self.selected_files.clear();
        self.select_anchor = None;
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
        self.selected_files.clear();
        self.select_anchor = None;
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
    /// Keep the current look, border and caption with the current file.
    fn save_edits_to_scene(&mut self) {
        let edits = crate::scene::Edits { look: self.look, border: self.border, caption: self.caption.clone() };
        if let Some(s) = &mut self.scene {
            s.edits = edits;
        }
    }

    /// Files that "Apply … to" buttons copy to: the selected files, or
    /// every other open file when none is selected. Never the current file.
    pub fn apply_targets(&self) -> Vec<usize> {
        let others = (0..self.files.len()).filter(|&i| Some(i) != self.active_file && self.files[i].scene.is_some());
        if self.selected_files.is_empty() {
            others.collect()
        } else {
            others.filter(|i| self.selected_files.contains(i)).collect()
        }
    }

    /// "all files" or "3 selected files", for button labels.
    pub fn apply_target_words(&self) -> String {
        if self.selected_files.is_empty() {
            "all files".into()
        } else {
            let n = self.apply_targets().len();
            format!("{n} selected file{}", if n == 1 { "" } else { "s" })
        }
    }

    /// Copy the current file's colour edits (Edit drawer: light, colour,
    /// curves, presets, film) to the target files.
    pub fn apply_edits_to_targets(&mut self) {
        let look = self.look;
        let targets = self.apply_targets();
        for &i in &targets {
            if let Some(s) = &mut self.files[i].scene {
                s.edits.look = look;
            }
        }
        self.save_edits_to_scene();
        let n = targets.len();
        self.status = format!("Applied this file's colour edits to {n} other file{}.", if n == 1 { "" } else { "s" });
    }

    /// Copy the current file's framing (crop, straighten, border, caption)
    /// to the target files. Quarter turns are not copied (each file keeps
    /// its own way up). A crop with a fixed shape keeps that shape on
    /// pictures of another size.
    pub fn apply_framing_to_targets(&mut self) {
        self.save_edits_to_scene();
        let Some(src) = &self.scene else { return };
        let (crop, aspect, crop_applied, angle) = (src.crop, src.aspect, src.crop_applied, src.angle);
        let src_size = (src.full.width(), src.full.height());
        let (border, caption) = (self.border, self.caption.clone());
        let targets = self.apply_targets();
        for &i in &targets {
            let entry = &mut self.files[i];
            let Some(s) = &mut entry.scene else { continue };
            s.edits.border = border;
            s.edits.caption = caption.clone();
            let (w, h) = (s.full.width(), s.full.height());
            s.crop = if (w, h) == src_size { crop } else { crop.with_aspect(aspect, w, h) };
            s.aspect = aspect;
            s.crop_applied = crop_applied && !s.crop.is_full();
            if (s.angle - angle).abs() > 1e-4 {
                s.set_angle(angle, false);
                let thumb = crate::scene::downscale(&s.work, 160);
                let c = egui::ColorImage::from_rgba_unmultiplied([thumb.width() as usize, thumb.height() as usize], thumb.as_raw());
                entry.thumb.set(c, TextureOptions::LINEAR);
            }
        }
        let n = targets.len();
        self.status = format!(
            "Applied this file's crop, straightening, border and caption to {n} other file{}.",
            if n == 1 { "" } else { "s" }
        );
    }

    fn show_scene(&mut self, _ctx: &egui::Context, scene: Scene) {
        self.look = scene.edits.look;
        self.border = scene.edits.border;
        self.caption = scene.edits.caption.clone();
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
            .add_filter("Photos, RAW and videos", &open_extensions())
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
            let convert_log = self.prefs.convert_log;
            self.start_decode(ctx, name, move || {
                crate::video::decode_video(&first, seconds, fps).map(|v| v.with_log_choice(convert_log))
            });
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
                self.open_queue.extend(paths);
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
        let log = match (video.apple_log, video.log_converted) {
            (true, true) => " Apple Log, converted to normal colours (switch off in the Video card).",
            (true, false) => " Apple Log, shown as shot (flat).",
            _ => "",
        };
        self.set_scene(ctx, Scene::from_video(name.clone(), video));
        self.tool = Tool::Rect;
        self.status = format!(
            "Loaded {name} ({n} frames, {secs:.1}s).{log} Drag a box or paint the area to freeze, then press Make loop."
        );
    }

    /// Switch the Apple Log conversion on or off, for the open clip and for
    /// clips opened later. Painted areas, trim and frozen frame are kept.
    pub fn set_log_conversion(&mut self, ctx: &egui::Context, convert: bool) {
        self.prefs.convert_log = convert;
        let Some(scene) = &mut self.scene else { return };
        let Some(video) = &scene.video else { return };
        if !video.apple_log || video.log_converted == convert {
            return;
        }
        let Some(switched) = video.log_switched(convert) else { return };
        scene.video = Some(Arc::new(switched));
        scene.switch_base_log(convert);
        let still = scene.still_index;
        self.set_still(ctx, still);
        self.status = if convert {
            "Apple Log converted to normal colours.".into()
        } else {
            "Apple Log shown as shot (flat, not converted).".into()
        };
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

    /// Frame of the current video shown by the live preview now.
    pub fn live_index(&self) -> Option<usize> {
        self.scene.as_ref()?.video.as_ref()?;
        let [start, end] = self.trim_range();
        let span = (end - start + 1) as f32;
        let cycle = if self.bounce { (2.0 * span - 2.0).max(1.0) } else { span };
        let phase = self.live_pos.rem_euclid(cycle);
        let rel = if phase < span { phase } else { cycle - phase };
        Some((start + rel as usize).min(end))
    }

    /// Move the live preview to a frame of the video (inside the trim range)
    /// and pause there.
    pub fn seek_live(&mut self, frame: usize) {
        let [start, end] = self.trim_range();
        self.live = true;
        self.playing = false;
        self.live_pos = (frame.clamp(start, end) - start) as f32;
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
    /// The look shown on the canvas: a hovered preset, or the file's own look.
    pub fn shown_look(&self) -> Look {
        self.look_preview.as_ref().map(|p| p.0).unwrap_or(self.look)
    }

    pub fn refresh_photo_texture(&mut self, ctx: &egui::Context) {
        let shown = self.shown_look();
        let Some(scene) = &self.scene else { return };
        // The still is shown at the quality picked in View. Above the editing
        // size it is drawn quickly at the editing size while a slider is
        // dragged or a style is hovered, then again sharp once that stops.
        let size = self.prefs.still_view;
        let sharp = size != crate::scene::WORK_SIZE && !scene.full_outdated;
        let fast = sharp && (self.look_preview.is_some() || ctx.input(|i| i.pointer.any_down()));
        let rebuild_base = self.photo_look.is_none() || self.raw_tex.is_none() || self.still_base_size != Some(size);
        if self.photo_tex.is_some() && !rebuild_base && self.photo_look == Some(shown) && !(self.photo_fast && !fast) {
            return;
        }
        if rebuild_base {
            let base = if sharp {
                crate::scene::downscale(&scene.full, if size == 0 { u32::MAX } else { size })
            } else {
                scene.work.clone()
            };
            let c = egui::ColorImage::from_rgba_unmultiplied([base.width() as usize, base.height() as usize], base.as_raw());
            match &mut self.raw_tex {
                Some(t) => t.set(c, TextureOptions::LINEAR),
                None => self.raw_tex = Some(ctx.load_texture("original", c, TextureOptions::LINEAR)),
            }
            self.still_base = Some(base);
            self.still_base_size = Some(size);
        }
        let Some(scene) = &self.scene else { return };
        let mut img = match &self.still_base {
            Some(b) if !fast => b.clone(),
            _ => scene.work.clone(),
        };
        crate::finish::apply_look(&mut img, &shown);
        let color = egui::ColorImage::from_rgba_unmultiplied([img.width() as usize, img.height() as usize], img.as_raw());
        match &mut self.photo_tex {
            Some(t) => t.set(color, TextureOptions::LINEAR),
            None => self.photo_tex = Some(ctx.load_texture("photo", color, TextureOptions::LINEAR)),
        }
        self.palette = crate::palette::dominant_colors(&img, 3);
        self.shown_still = Some(img);
        self.photo_look = Some(shown);
        self.photo_fast = fast;
    }

    /// Pixel size of the still as shown now (for the View menu).
    pub fn still_view_pixels(&self) -> Option<[u32; 2]> {
        self.still_base.as_ref().map(|b| [b.width(), b.height()])
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
        let idx = self.live_index().unwrap_or(start);
        let Some(scene) = &self.scene else { return false };
        let Some(video) = &scene.video else { return false };
        let mut frame = crate::video::live_frame(&video.work[idx], &video.work[scene.still_index], &scene.mask, self.freeze_painted);
        crate::finish::apply_look(&mut frame, &self.shown_look());
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
    /// Straighten the current file to `degrees`. With `quick` (while the
    /// slider is dragged) only the editing copy is redrawn; `finish_angle`
    /// completes it and records one undo step.
    pub fn set_angle(&mut self, ctx: &egui::Context, degrees: f32, quick: bool) {
        let Some(scene) = &mut self.scene else { return };
        if self.angle_from.is_none() {
            self.angle_from = Some(scene.angle);
        }
        scene.set_angle(degrees, quick);
        self.straightening = quick;
        self.photo_look = None;
        self.live_tex = None;
        self.refresh_photo_texture(ctx);
        if !quick {
            self.finish_angle(ctx);
        }
    }

    pub fn finish_angle(&mut self, ctx: &egui::Context) {
        self.straightening = false;
        let Some(scene) = &mut self.scene else { return };
        let was_outdated = scene.full_outdated;
        scene.finish_angle();
        if let Some(from) = self.angle_from.take() {
            scene.push_angle_step(from);
        }
        let angle = scene.angle;
        if was_outdated {
            self.photo_look = None;
            self.live_tex = None;
            self.refresh_photo_texture(ctx);
        }
        self.refresh_thumbnail();
        self.status = if angle == 0.0 { "Straighten off.".into() } else { format!("Straightened {angle:+.1}°.") };
    }

    /// The picture the canvas shows now, for the vectorscope: the playing
    /// loop's frame in Preview, otherwise the edited still.
    pub fn scope_source(&self) -> Option<&RgbaImage> {
        if self.view == View::Preview && self.look_preview.is_none() {
            if let Some(clip) = self.active_clip.and_then(|i| self.clips.get(i)) {
                let n = clip.frames.len();
                if n > 0 {
                    return clip.frames.get((self.play_pos as usize).min(n - 1));
                }
            }
        }
        self.shown_still.as_ref()
    }

    /// Redraw the vectorscope when the shown picture or the zoom changed.
    pub fn refresh_scope(&mut self, ctx: &egui::Context) {
        use std::hash::{Hash, Hasher};
        let Some(img) = self.scope_source() else {
            self.scope_tex = None;
            self.scope_key = 0;
            return;
        };
        let mut h = std::collections::hash_map::DefaultHasher::new();
        (img.as_raw().as_ptr() as usize, img.width(), img.height(), self.scope_zoom.to_bits()).hash(&mut h);
        // A few pixels too, in case a new picture reuses the same memory.
        let raw = img.as_raw();
        for k in 0..16 {
            raw.get(k * raw.len() / 16).hash(&mut h);
        }
        let key = h.finish();
        if key == self.scope_key && self.scope_tex.is_some() {
            return;
        }
        let trace = crate::scope::trace(img, self.scope_zoom);
        self.scope_key = key;
        match &mut self.scope_tex {
            Some(t) => t.set(trace, TextureOptions::NEAREST),
            None => self.scope_tex = Some(ctx.load_texture("vectorscope", trace, TextureOptions::NEAREST)),
        }
    }

    // ----- Layout -------------------------------------------------------

    /// A file's scene and its edits (the current file's edits live in the app).
    fn scene_and_edits(&self, id: u64) -> Option<(&Scene, crate::scene::Edits)> {
        if let Some(s) = self.scene.as_ref().filter(|s| s.id == id) {
            return Some((s, crate::scene::Edits { look: self.look, border: self.border, caption: self.caption.clone() }));
        }
        self.files.iter().filter_map(|f| f.scene.as_ref()).find(|s| s.id == id).map(|s| (s, s.edits.clone()))
    }

    /// Scene ids for a new layout: the current file and the selected ones,
    /// or every file when none is selected, in Files panel order.
    pub fn layout_candidates(&self) -> Vec<u64> {
        let id_at = |i: usize| {
            if Some(i) == self.active_file { self.scene.as_ref().map(|s| s.id) } else { self.files[i].scene.as_ref().map(|s| s.id) }
        };
        (0..self.files.len())
            .filter(|&i| self.selected_files.is_empty() || self.selected_files.contains(&i) || Some(i) == self.active_file)
            .filter_map(id_at)
            .collect()
    }

    /// Start a layout from the current and selected files (or all files).
    pub fn new_layout(&mut self) {
        let ids = self.layout_candidates();
        if ids.len() < 2 {
            self.status = "Open at least two files to make a layout.".into();
            return;
        }
        self.layout = Some(crate::layout::Layout::new(&ids));
        self.layout_cell = None;
        self.open_layout();
    }

    /// Show the layout, with every file's latest edits.
    pub fn open_layout(&mut self) {
        if self.layout.is_none() {
            return;
        }
        self.save_edits_to_scene();
        self.layout_photos.clear();
        self.layout_drawn = None;
        self.layout_open = true;
        self.refresh_layout_photos();
        self.status = "Layout: drag a photo to move it, onto another box to swap, or drag a file from the left into a box.".into();
    }

    pub fn close_layout(&mut self) {
        self.layout_open = false;
        self.layout_drag = None;
        self.status = match &self.scene {
            Some(s) => format!("Editing {}. The layout is kept: press Layout to go back to it.", s.name),
            None => "The layout is kept: press Layout to go back to it.".into(),
        };
    }

    /// Size of the finished pictures used for the layout preview.
    const LAYOUT_PREVIEW: u32 = 1400;

    /// Make the finished picture (colour edits, crop, caption, border) of
    /// each file in the layout that does not have one yet.
    pub fn refresh_layout_photos(&mut self) {
        let Some(layout) = &self.layout else { return };
        let ids: Vec<u64> = layout.cells.iter().filter_map(|c| c.source).chain(layout.files.iter().copied()).collect();
        for id in ids {
            if self.layout_photos.contains_key(&id) {
                continue;
            }
            if let Some((scene, edits)) = self.scene_and_edits(id) {
                let img = framed_still(&scene.full, scene.crop, &edits, Self::LAYOUT_PREVIEW);
                self.layout_photos.insert(id, img);
            }
        }
    }

    /// Swap the photos of two boxes; both fill their new boxes, centered.
    pub fn swap_layout_cells(&mut self, a: usize, b: usize) {
        let Some(l) = &mut self.layout else { return };
        if a == b || a >= l.cells.len() || b >= l.cells.len() {
            return;
        }
        let (sa, sb) = (l.cells[a].source, l.cells[b].source);
        l.cells[a] = crate::layout::Cell::new(sb);
        l.cells[b] = crate::layout::Cell::new(sa);
        self.layout_cell = Some(b);
    }

    /// Put a file in a box of the layout.
    pub fn place_in_layout(&mut self, cell: usize, file_index: usize) {
        let id = if Some(file_index) == self.active_file {
            self.scene.as_ref().map(|s| s.id)
        } else {
            self.files.get(file_index).and_then(|f| f.scene.as_ref()).map(|s| s.id)
        };
        let Some(id) = id else { return };
        if let Some(l) = &mut self.layout {
            if let Some(c) = l.cells.get_mut(cell) {
                *c = crate::layout::Cell::new(Some(id));
            }
            if !l.files.contains(&id) {
                l.files.push(id);
            }
        }
        self.refresh_layout_photos();
    }

    /// Redraw the layout preview when the layout changed.
    pub fn refresh_layout_texture(&mut self, ctx: &egui::Context) {
        let Some(layout) = &self.layout else { return };
        if self.layout_drawn.as_ref() == Some(layout) && self.layout_tex.is_some() {
            return;
        }
        let (w, h) = layout.size(1600);
        let photos = &self.layout_photos;
        let img = crate::layout::render(layout, w, h, &|id| photos.get(&id));
        let c = egui::ColorImage::from_rgba_unmultiplied([w as usize, h as usize], img.as_raw());
        match &mut self.layout_tex {
            Some(t) => t.set(c, TextureOptions::LINEAR),
            None => self.layout_tex = Some(ctx.load_texture("layout", c, TextureOptions::LINEAR)),
        }
        self.layout_drawn = Some(layout.clone());
    }

    /// Save the layout at full quality (in the background).
    pub fn export_layout(&mut self) {
        let Some(layout) = self.layout.clone() else { return };
        if self.task.is_some() {
            return;
        }
        self.save_edits_to_scene();
        let format = self.layout_format;
        let ext = format.extension().unwrap_or("png");
        let one = [ext];
        let Some(path) = rfd::FileDialog::new()
            .set_title("Save layout")
            .set_file_name(format!("layout.{ext}"))
            .add_filter(format.label(), if format == Format::Heif { &["heic", "heif"][..] } else { &one[..] })
            .save_file()
        else {
            return;
        };
        let long = self.layout_size;
        // Each photo at no more than the layout's size, with its edits.
        let sources: Vec<(u64, RgbaImage, CropRect, crate::scene::Edits)> = layout
            .cells
            .iter()
            .filter_map(|c| c.source)
            .filter_map(|id| {
                self.scene_and_edits(id).map(|(s, e)| (id, crate::scene::downscale(&s.full, long), s.crop, e))
            })
            .collect();
        let progress = Arc::new(Progress::new());
        let (tx, rx) = channel();
        let path2 = path.clone();
        std::thread::spawn(move || {
            let photos: std::collections::HashMap<u64, RgbaImage> = sources
                .into_iter()
                .map(|(id, img, crop, edits)| (id, framed_still(&img, crop, &edits, long)))
                .collect();
            let (w, h) = layout.size(long);
            let img = crate::layout::render(&layout, w, h, &|id| photos.get(&id));
            let _ = tx.send(TaskResult::Saved(crate::export::save_still(format, &img, &path2)));
        });
        self.task = Some(Task { progress, rx, kind: TaskKind::Export, label: "Saving layout".into() });
    }

    /// Redraw the file list thumbnail of the current file.
    fn refresh_thumbnail(&mut self) {
        let Some(scene) = &self.scene else { return };
        let thumb_img = crate::scene::downscale(&scene.work, 160);
        let color = egui::ColorImage::from_rgba_unmultiplied(
            [thumb_img.width() as usize, thumb_img.height() as usize],
            thumb_img.as_raw(),
        );
        if let Some(entry) = self.active_file.and_then(|i| self.files.get_mut(i)) {
            entry.thumb.set(color, TextureOptions::LINEAR);
        }
    }

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
        // Colour settings come back with the step.
        if let Some(s) = &self.scene {
            self.look = s.edits.look;
            self.look_committed = Some((s.id, self.look));
        }
        // Loops made from this file follow its crop.
        if let Some(c) = self.scene.as_ref().map(|s| s.crop) {
            self.set_crop(c);
        }
        self.crop_drag = None;
    }

    /// Make each finished change to the colour settings (a slider let go, a
    /// curve point dropped, a preset or film picked, Reset) one undo step.
    fn track_look_undo(&mut self, ctx: &egui::Context) {
        let busy = ctx.input(|i| i.pointer.any_down());
        let look = self.look;
        let Some(scene) = &mut self.scene else {
            self.look_committed = None;
            return;
        };
        match self.look_committed {
            Some((id, old)) if id == scene.id => {
                if old != look && !busy {
                    scene.edits.look = old;
                    scene.checkpoint();
                    self.look_committed = Some((id, look));
                }
            }
            // Another file is open: start from its look.
            _ => self.look_committed = Some((scene.id, look)),
        }
        scene.edits.look = look;
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
        if crate::raw::needs_system_decoder(path) {
            // RAW (and HEIC) files take a moment to develop: do it in the background.
            if self.task.is_some() {
                self.open_queue.push(path.to_path_buf());
                return;
            }
            let name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| "Photo".into());
            let p = path.to_path_buf();
            let progress = Arc::new(Progress::new());
            let (tx, rx) = channel();
            let ctx2 = ctx.clone();
            let n2 = name.clone();
            std::thread::spawn(move || {
                let _ = tx.send(TaskResult::Photo(crate::raw::decode(&p), n2));
                ctx2.request_repaint();
            });
            self.task = Some(Task { progress, rx, kind: TaskKind::Decode, label: format!("Developing {name}") });
            return;
        }
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
        let one = [ext];
        let Some(path) = rfd::FileDialog::new()
            .set_title("Save image")
            .set_file_name(format!("{base}_still.{ext}"))
            .add_filter(format.label(), if format == Format::Heif { &["heic", "heif"][..] } else { &one[..] })
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
            let img = finish_frame(&img, crop, &caption_overlay(&caption, crop, &img, &border), border);
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
                let overlay = f.first().and_then(|x| caption_overlay(&caption, crop, x, &border));
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
            (TaskResult::Photo(Ok(img), name), _) => {
                self.set_scene(ctx, Scene::new(name.clone(), img));
                self.status = format!("Loaded {name}. Paint the part that should move, or use Auto-detect.");
            }
            (TaskResult::Photo(Err(e), name), _) => {
                self.status = format!("Could not read {name}.");
                self.error = Some(format!("Could not open {name}.\n\n{e}"));
            }
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
        let key = format!("{}{:?}", self.caption.key(w, h), self.border);
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
        let rendered = shown.render_with_border(w, h, &self.border);
        self.caption_on_border = rendered.as_ref().is_some_and(|r| r.1);
        self.caption_tex = rendered.map(|(img, _)| {
            let c = egui::ColorImage::from_rgba_unmultiplied([img.width() as usize, img.height() as usize], img.as_raw());
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
        self.film_choice = None;
        self.film_strength = 1.0;
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

/// A file's finished still: colour edits, then crop, caption and border,
/// from a picture made no larger than `long` on its longest side.
fn framed_still(full: &RgbaImage, crop: CropRect, edits: &crate::scene::Edits, long: u32) -> RgbaImage {
    let mut img = crate::scene::downscale(full, long);
    crate::finish::apply_look(&mut img, &edits.look);
    let overlay = caption_overlay(&edits.caption, crop, &img, &edits.border);
    finish_frame(&img, crop, &overlay, edits.border)
}

/// The caption for the cropped picture: picture-sized, or output-sized
/// (flag true) when it goes in the border.
fn caption_overlay(
    caption: &crate::caption::Caption,
    crop: CropRect,
    full: &RgbaImage,
    border: &crate::border::Border,
) -> Option<(RgbaImage, bool)> {
    let [_, _, w, h] = crop.pixels(full.width(), full.height());
    caption.render_with_border(w, h, border)
}

/// Crop, caption and border: the last steps before saving a frame. A
/// caption on the picture goes on before the border, one in the border after.
fn finish_frame(
    img: &RgbaImage,
    crop: CropRect,
    caption: &Option<(RgbaImage, bool)>,
    border: crate::border::Border,
) -> RgbaImage {
    let mut out = crop.apply(img);
    if let Some((c, false)) = caption {
        crate::caption::blend(&mut out, c);
    }
    let mut out = border.apply(&out);
    if let Some((c, true)) = caption {
        crate::caption::blend(&mut out, c);
    }
    out
}

/// Every file type the Open dialog offers.
fn open_extensions() -> Vec<&'static str> {
    let mut v = vec![
        "jpg", "jpeg", "png", "webp", "bmp", "tif", "tiff", "gif", "mp4", "mov", "m4v", "webm", "avi", "mkv",
    ];
    v.extend(crate::raw::SYSTEM_FORMATS);
    v.extend(crate::raw::EXTENSIONS);
    v
}

/// Open a photo and turn it upright using the orientation the camera saved.
/// Open a photo upright and in sRGB: the orientation the camera saved is
/// applied, and colours are converted from the photo's own profile (Display
/// P3 on iPhones) to sRGB, the colours Animagraph edits and shows.
pub(crate) fn open_upright(path: &std::path::Path) -> image::ImageResult<image::DynamicImage> {
    use image::ImageDecoder;
    let mut decoder = image::ImageReader::open(path)?.with_guessed_format()?.into_decoder()?;
    let orientation = decoder.orientation().unwrap_or(image::metadata::Orientation::NoTransforms);
    let icc = decoder.icc_profile().ok().flatten();
    let mut img = image::DynamicImage::from_decoder(decoder)?;
    img.apply_orientation(orientation);
    if let Some(icc) = icc {
        let mut rgba = img.to_rgba8();
        if crate::colour::to_srgb(&mut rgba, &icc) {
            img = image::DynamicImage::ImageRgba8(rgba);
        }
    }
    Ok(img)
}

impl eframe::App for StudioApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.poll_task(&ctx);
        self.handle_dropped_files(&ctx);
        self.track_look_undo(&ctx);
        // A straighten drag that ended without the slider noticing.
        if self.straightening && !ctx.input(|i| i.pointer.any_down()) {
            self.finish_angle(&ctx);
        }
        self.shortcuts(&ctx);
        self.refresh_mask_texture(&ctx);
        // Preview of the preset under the pointer (set during the last frame).
        let preview = self.hover_look.take();
        if preview.as_ref().map(|p| &p.0) != self.look_preview.as_ref().map(|p| &p.0) {
            ctx.request_repaint();
        }
        self.look_preview = preview;
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
            .show(ui, |ui| {
                if self.layout_open {
                    crate::layout_view::canvas(self, ui);
                } else {
                    crate::canvas::canvas(self, ui);
                }
            });
        // Dialogs drawn last, so they sit above the canvas's floating bars.
        crate::preset_dialog::confirm_delete(self, &ctx);
    }
}
