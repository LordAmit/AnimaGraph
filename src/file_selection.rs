//! Picking files in the Files panel: click, Cmd/Ctrl-click, Shift-click,
//! Cmd/Ctrl+A and Shift+Up/Down. The file shown on the canvas is always the
//! one picked last; unpicking it shows the one picked before it.

use std::collections::BTreeSet;

use eframe::egui;

use crate::app::StudioApp;

/// The picked files (the shown one included) and the order they were picked in.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Picks {
    pub shown: Option<usize>,
    pub picked: BTreeSet<usize>,
    /// Oldest first; the last one is shown.
    order: Vec<usize>,
    /// Where Shift-click and Shift+arrow ranges start.
    pub anchor: Option<usize>,
}

pub enum Pick {
    /// Plain click: only this file.
    Click(usize),
    /// Cmd/Ctrl-click: add or take away one file.
    Toggle(usize),
    /// Shift-click: every file from the anchor to this one.
    Range(usize),
    /// Shift+Up (-1) or Shift+Down (+1): grow or shrink the range from the anchor.
    Step(i32),
    All,
    /// Only the shown file.
    Clear,
}

impl Picks {
    fn show(&mut self, i: usize) {
        self.order.retain(|&k| k != i);
        self.order.push(i);
        self.picked.insert(i);
        self.shown = Some(i);
    }

    fn unpick(&mut self, i: usize) {
        self.picked.remove(&i);
        self.order.retain(|&k| k != i);
    }

    /// Change the picks; `files` is how many files there are.
    pub fn apply(&mut self, pick: Pick, files: usize) {
        if files == 0 {
            return;
        }
        match pick {
            Pick::Click(i) => {
                *self = Picks::default();
                self.show(i);
                self.anchor = Some(i);
            }
            Pick::Toggle(i) => {
                if !self.picked.contains(&i) {
                    self.show(i);
                    self.anchor = Some(i);
                } else if self.picked.len() > 1 {
                    self.unpick(i);
                    if self.shown == Some(i) {
                        // Show the file picked before it.
                        self.shown = self.order.last().copied();
                    }
                    self.anchor = self.shown;
                }
            }
            Pick::Range(i) => {
                let from = self.anchor.or(self.shown).unwrap_or(i).min(files - 1);
                let range: Vec<usize> = if from <= i { (from..=i).collect() } else { (i..=from).rev().collect() };
                for k in range {
                    self.show(k);
                }
                self.anchor = Some(from);
            }
            Pick::Step(dir) => {
                let Some(at) = self.shown else { return };
                let next = at as i64 + dir as i64;
                if next < 0 || next >= files as i64 {
                    return;
                }
                let next = next as usize;
                let anchor = *self.anchor.get_or_insert(at);
                // Going back towards the anchor gives up the file left behind.
                let towards = (next as i64 - anchor as i64).abs() < (at as i64 - anchor as i64).abs();
                if towards {
                    self.unpick(at);
                }
                self.show(next);
            }
            Pick::All => {
                let shown = self.shown;
                for k in 0..files {
                    if !self.picked.contains(&k) {
                        self.show(k);
                    }
                }
                // The file on the canvas stays.
                if let Some(s) = shown {
                    self.show(s);
                }
            }
            Pick::Clear => {
                let shown = self.shown;
                *self = Picks::default();
                if let Some(s) = shown {
                    self.show(s);
                    self.anchor = Some(s);
                }
            }
        }
    }
}

impl StudioApp {
    /// The picks as the rest of the app keeps them.
    fn picks(&self) -> Picks {
        let mut p = self.file_picks.clone();
        p.shown = self.active_file;
        p.picked = self.selected_files.clone();
        p.picked.extend(self.active_file);
        p.order.retain(|k| p.picked.contains(k));
        // Files picked in another way (an Apply target list from earlier, say).
        for k in p.picked.clone() {
            if !p.order.contains(&k) {
                p.order.insert(0, k);
            }
        }
        if let Some(s) = p.shown {
            p.order.retain(|&k| k != s);
            p.order.push(s);
        }
        p
    }

    /// Pick files and show the one picked last.
    pub fn pick_files(&mut self, ctx: &egui::Context, pick: Pick) {
        let mut p = self.picks();
        p.apply(pick, self.files.len());
        if let Some(s) = p.shown.filter(|&s| Some(s) != self.active_file) {
            self.select_file(ctx, s);
            self.files_scroll = true;
        }
        // One file picked is no selection: Apply then goes to every file.
        self.selected_files = if p.picked.len() > 1 { p.picked.iter().copied().filter(|&k| Some(k) != self.active_file).collect() } else { BTreeSet::new() };
        self.select_anchor = p.anchor;
        self.file_picks = p;
    }

    /// Cmd/Ctrl+A and Shift+Up/Down, while the Files panel is shown and no
    /// text is being typed.
    pub fn file_keys(&mut self, ctx: &egui::Context) {
        if self.files.is_empty() || self.layout_open {
            return;
        }
        let (all, up, down) = ctx.input(|i| {
            let shift_only = i.modifiers.shift && !i.modifiers.command && !i.modifiers.alt;
            (
                i.modifiers.command && i.key_pressed(egui::Key::A),
                shift_only && i.key_pressed(egui::Key::ArrowUp),
                shift_only && i.key_pressed(egui::Key::ArrowDown),
            )
        });
        if all {
            self.pick_files(ctx, Pick::All);
        }
        if up {
            self.pick_files(ctx, Pick::Step(-1));
        }
        if down {
            self.pick_files(ctx, Pick::Step(1));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn picks(p: &Picks) -> (Option<usize>, Vec<usize>) {
        (p.shown, p.picked.iter().copied().collect())
    }

    fn start(at: usize) -> Picks {
        let mut p = Picks::default();
        p.apply(Pick::Click(at), 6);
        p
    }

    #[test]
    fn the_last_picked_file_is_shown_and_unpicking_it_shows_the_one_before() {
        let mut p = start(0);
        p.apply(Pick::Toggle(3), 6);
        p.apply(Pick::Toggle(5), 6);
        assert_eq!(picks(&p), (Some(5), vec![0, 3, 5]));
        p.apply(Pick::Toggle(5), 6);
        assert_eq!(picks(&p), (Some(3), vec![0, 3]));
        // Unpicking one that is not shown leaves the shown one.
        p.apply(Pick::Toggle(0), 6);
        assert_eq!(picks(&p), (Some(3), vec![3]));
        // The last file stays picked.
        p.apply(Pick::Toggle(3), 6);
        assert_eq!(picks(&p), (Some(3), vec![3]));
    }

    #[test]
    fn a_plain_click_picks_only_that_file() {
        let mut p = start(1);
        p.apply(Pick::Toggle(4), 6);
        p.apply(Pick::Click(1), 6);
        assert_eq!(picks(&p), (Some(1), vec![1]));
    }

    #[test]
    fn shift_arrows_grow_and_shrink_from_where_they_started() {
        let mut p = start(2);
        p.apply(Pick::Step(1), 6);
        p.apply(Pick::Step(1), 6);
        assert_eq!(picks(&p), (Some(4), vec![2, 3, 4]));
        p.apply(Pick::Step(-1), 6);
        assert_eq!(picks(&p), (Some(3), vec![2, 3]));
        p.apply(Pick::Step(-1), 6);
        p.apply(Pick::Step(-1), 6);
        assert_eq!(picks(&p), (Some(1), vec![1, 2]));
        // Not past the first file.
        p.apply(Pick::Step(-1), 6);
        p.apply(Pick::Step(-1), 6);
        assert_eq!(picks(&p), (Some(0), vec![0, 1, 2]));
    }

    #[test]
    fn shift_click_picks_a_range_and_shows_its_end() {
        let mut p = start(1);
        p.apply(Pick::Range(4), 6);
        assert_eq!(picks(&p), (Some(4), vec![1, 2, 3, 4]));
        p.apply(Pick::Step(1), 6);
        assert_eq!(picks(&p), (Some(5), vec![1, 2, 3, 4, 5]));
    }

    #[test]
    fn select_all_keeps_the_shown_file_and_clear_keeps_only_it() {
        let mut p = start(2);
        p.apply(Pick::All, 6);
        assert_eq!(picks(&p), (Some(2), vec![0, 1, 2, 3, 4, 5]));
        p.apply(Pick::Toggle(2), 6);
        assert_eq!(p.shown.map(|s| s != 2), Some(true));
        p.apply(Pick::Clear, 6);
        assert_eq!(p.picked.len(), 1);
    }
}
