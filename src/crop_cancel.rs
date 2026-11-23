//! Esc while cropping: the crop goes back to what it was when the crop tool
//! was picked, and the crop tool is left.

use crate::app::{StudioApp, Tool};

impl StudioApp {
    /// Remember the crop as it is now, unless this file's crop is already
    /// being changed.
    pub fn begin_crop(&mut self) {
        let Some(s) = &self.scene else { return };
        if self.crop_entry.is_none_or(|(id, _, _)| id != s.id) {
            self.crop_entry = Some((s.id, s.crop, s.aspect));
        }
    }

    /// Undo the crop made since the crop tool was picked.
    pub fn cancel_crop(&mut self) {
        let entry = self.crop_entry.take();
        self.crop_drag = None;
        self.crop_before = None;
        let mut undone = false;
        if let (Some((id, crop, aspect)), Some(s)) = (entry, &mut self.scene) {
            if s.id == id && (s.crop != crop || s.aspect != aspect) {
                s.checkpoint();
                s.aspect = aspect;
                self.set_crop(crop);
                undone = true;
            }
        }
        self.pick_tool(Tool::Hand);
        self.status = if undone { "Crop undone.".into() } else { "Left the crop tool.".into() };
    }
}
