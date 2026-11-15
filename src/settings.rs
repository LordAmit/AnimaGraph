//! Remembers interface choices between sessions (which panels are shown or
//! folded, the caption font, the still photo's view quality, Apple Log conversion) in a small text file in the app's data folder.

use std::collections::BTreeSet;
use std::path::PathBuf;

#[derive(Clone, Debug, PartialEq)]
pub struct Prefs {
    /// Panels switched off in the View menu.
    pub hidden: BTreeSet<String>,
    /// Panels folded to their title.
    pub collapsed: BTreeSet<String>,
    /// Path of the last caption font, empty for the built-in one.
    pub caption_font: String,
    /// Long side of the still photo shown while editing, in pixels;
    /// 0 shows the full photo.
    pub still_view: u32,
    /// Convert Apple Log videos to normal colours when they are opened.
    pub convert_log: bool,
}

/// Choices for the still photo's view quality: (long side, label).
pub const STILL_VIEW_CHOICES: [(u32, &str); 4] = [
    (crate::scene::WORK_SIZE, "Fast · 720 px"),
    (1440, "Good · 1440 px"),
    (2560, "Sharp · 2560 px"),
    (0, "Full photo · slowest"),
];

impl Default for Prefs {
    fn default() -> Self {
        Self {
            hidden: BTreeSet::new(),
            collapsed: BTreeSet::new(),
            caption_font: String::new(),
            still_view: crate::scene::WORK_SIZE,
            convert_log: true,
        }
    }
}

fn file() -> PathBuf {
    crate::fonts::data_dir().join("settings.txt")
}

fn parse_set(v: &str) -> BTreeSet<String> {
    v.split(',').map(str::trim).filter(|s| !s.is_empty()).map(String::from).collect()
}

impl Prefs {
    pub fn load() -> Self {
        let Ok(text) = std::fs::read_to_string(file()) else { return Self::default() };
        Self::parse(&text)
    }

    pub fn parse(text: &str) -> Self {
        let mut p = Self::default();
        for line in text.lines() {
            let Some((k, v)) = line.split_once('=') else { continue };
            match k.trim() {
                "hidden" => p.hidden = parse_set(v),
                "collapsed" => p.collapsed = parse_set(v),
                "caption_font" => p.caption_font = v.trim().to_string(),
                "convert_log" => p.convert_log = v.trim() != "0",
                "still_view" => {
                    if let Ok(n) = v.trim().parse::<u32>() {
                        if STILL_VIEW_CHOICES.iter().any(|(c, _)| *c == n) {
                            p.still_view = n;
                        }
                    }
                }
                _ => {}
            }
        }
        p
    }

    pub fn to_text(&self) -> String {
        let join = |s: &BTreeSet<String>| s.iter().cloned().collect::<Vec<_>>().join(",");
        format!(
            "hidden={}\ncollapsed={}\ncaption_font={}\nstill_view={}\nconvert_log={}\n",
            join(&self.hidden),
            join(&self.collapsed),
            self.caption_font,
            self.still_view,
            u8::from(self.convert_log)
        )
    }

    pub fn save(&self) {
        let path = file();
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = std::fs::write(path, self.to_text());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip() {
        let mut p = Prefs::default();
        p.hidden.insert("look".into());
        p.collapsed.insert("border".into());
        p.collapsed.insert("motion".into());
        p.caption_font = "/x/y.ttf".into();
        p.still_view = 2560;
        p.convert_log = false;
        assert_eq!(Prefs::parse(&p.to_text()), p);
    }
}
