//! Saving several photos (the selected ones, or all that are open) into one
//! folder, each with its own edits. One background thread saves them one after
//! the other, so it goes on while the window is hidden and not being drawn.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc::channel;

use crate::app::{StudioApp, Task, TaskKind, TaskResult};
use crate::export::Format;
use crate::render::Progress;
use crate::still_export::{StillJob, unique_target};

/// A save that is running, as the window sees it.
pub struct Batch {
    folder: PathBuf,
    total: usize,
    state: Arc<State>,
}

#[derive(Default)]
struct State {
    done: AtomicUsize,
    stop: AtomicBool,
}

/// How a save ended.
#[derive(Default)]
pub struct Outcome {
    saved: Vec<PathBuf>,
    /// Names of photos that could not be saved, with the reason.
    failed: Vec<String>,
}

impl Batch {
    /// "Saving photo 2 of 5".
    pub fn label(&self) -> String {
        let n = (self.state.done.load(Ordering::Relaxed) + 1).min(self.total);
        format!("Saving photo {n} of {}", self.total)
    }
}

/// Save every job (its target is a name without an extension), until stopped.
fn save_all(jobs: Vec<StillJob>, state: &State) -> Outcome {
    let mut out = Outcome::default();
    for mut job in jobs {
        if state.stop.load(Ordering::Relaxed) {
            break;
        }
        let base = job.target.file_name().map_or_else(String::new, |n| n.to_string_lossy().into_owned());
        let folder = job.target.parent().unwrap_or(Path::new("")).to_path_buf();
        let taken: HashSet<PathBuf> = out.saved.iter().cloned().collect();
        job.target = unique_target(&folder, &base, job.format.extension().unwrap_or("png"), &taken);
        match job.run() {
            Ok(p) => out.saved.push(p),
            Err(e) => out.failed.push(format!("{base}: {e}")),
        }
        state.done.fetch_add(1, Ordering::Relaxed);
    }
    out
}

/// "1 photo", "3 photos".
pub fn photos(n: usize) -> String {
    format!("{n} photo{}", if n == 1 { "" } else { "s" })
}

/// `ids` when there is something to save besides the open file.
fn several(ids: Vec<u64>, open: Option<u64>) -> Option<Vec<u64>> {
    (ids.iter().any(|id| Some(*id) != open)).then_some(ids)
}

impl StudioApp {
    /// Scene ids of the open photos, in Files panel order (no videos).
    pub fn photo_ids(&self) -> Vec<u64> {
        self.photo_ids_where(|_| true)
    }

    /// The photos Export saves together: the open one and those selected in
    /// the Files panel. `None` when nothing else is selected, so Export saves
    /// the open file alone.
    pub fn selected_photo_ids(&self) -> Option<Vec<u64>> {
        if !self.selected_files.iter().any(|i| Some(*i) != self.active_file) {
            return None;
        }
        let ids = self.photo_ids_where(|i| Some(i) == self.active_file || self.selected_files.contains(&i));
        several(ids, self.scene.as_ref().map(|s| s.id))
    }

    fn photo_ids_where(&self, keep: impl Fn(usize) -> bool) -> Vec<u64> {
        (0..self.files.len())
            .filter(|&i| keep(i))
            .filter_map(|i| if Some(i) == self.active_file { self.scene.as_ref() } else { self.files[i].scene.as_ref() })
            .filter(|s| !s.is_video())
            .map(|s| s.id)
            .collect()
    }

    /// Ask for a folder, then save these photos into it.
    pub fn export_photos(&mut self, ids: Vec<u64>) {
        let title = format!("Save {} into a folder", photos(ids.len()));
        if let Some(folder) = rfd::FileDialog::new().set_title(title).pick_folder() {
            self.start_batch(&folder, ids);
        }
    }

    pub fn start_batch(&mut self, folder: &Path, ids: Vec<u64>) {
        if self.busy() || !self.export_format.is_still() {
            return;
        }
        // The edits are the ones at this moment.
        let jobs: Vec<StillJob> = ids.into_iter().filter_map(|id| self.still_job(id, folder, self.export_format)).collect();
        if jobs.is_empty() {
            return;
        }
        let state = Arc::new(State::default());
        let batch = Batch { folder: folder.to_path_buf(), total: jobs.len(), state: state.clone() };
        self.status = format!("Saving {}…", photos(jobs.len()));
        let label = batch.label();
        self.batch = Some(batch);
        let (tx, rx) = channel();
        std::thread::spawn(move || {
            let _ = tx.send(TaskResult::Batch(save_all(jobs, &state)));
        });
        self.task = Some(Task { progress: Arc::new(Progress::new()), rx, kind: TaskKind::Batch, label });
    }

    fn still_job(&self, id: u64, folder: &Path, format: Format) -> Option<StillJob> {
        let (scene, edits) = self.scene_and_edits(id)?;
        let base = scene.name.rsplit_once('.').map_or(scene.name.as_str(), |(a, _)| a);
        Some(StillJob {
            full: scene.full.clone(),
            crop: scene.crop,
            edits,
            size: self.export_size,
            format,
            target: folder.join(base),
        })
    }

    /// Stop after the photo being saved now.
    pub fn stop_batch(&mut self) {
        if let Some(b) = &self.batch {
            b.state.stop.store(true, Ordering::Relaxed);
        }
    }

    /// Every photo is saved, or the save was stopped.
    pub fn batch_finished(&mut self, mut out: Outcome) {
        let Some(b) = self.batch.take() else { return };
        self.status = format!("Saved {} of {} into {}", out.saved.len(), photos(b.total), b.folder.display());
        if !out.failed.is_empty() {
            self.error = Some(format!("Some photos could not be saved:\n\n{}", out.failed.join("\n")));
        }
        if let Some(p) = out.saved.pop() {
            self.last_export = Some(p);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn export_is_for_several_only_with_another_photo() {
        assert_eq!(several(vec![4, 7], Some(4)), Some(vec![4, 7]));
        // The open file is a video and one photo is selected.
        assert_eq!(several(vec![7], Some(4)), Some(vec![7]));
        // Only videos are selected besides the open photo.
        assert_eq!(several(vec![4], Some(4)), None);
        assert_eq!(several(vec![], Some(4)), None);
    }

    fn job(folder: &Path, name: &str) -> StillJob {
        StillJob {
            full: image::RgbaImage::from_pixel(40, 30, image::Rgba([90, 90, 90, 255])).into(),
            crop: crate::crop::CropRect::FULL,
            edits: Default::default(),
            size: crate::export_size::ExportSize::ORIGINAL,
            format: Format::Png,
            target: folder.join(name),
        }
    }

    #[test]
    fn photos_with_the_same_name_are_all_kept() {
        let dir = std::env::temp_dir().join(format!("animagraph_batch_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let state = State::default();
        let out = save_all(vec![job(&dir, "a"), job(&dir, "a"), job(&dir, "b")], &state);
        assert_eq!(out.saved, [dir.join("a.png"), dir.join("a 2.png"), dir.join("b.png")]);
        assert!(out.failed.is_empty() && out.saved.iter().all(|p| p.exists()));
        assert_eq!(state.done.load(Ordering::Relaxed), 3);
        // Stopped before the first one: nothing is saved.
        state.stop.store(true, Ordering::Relaxed);
        assert!(save_all(vec![job(&dir, "c")], &state).saved.is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
