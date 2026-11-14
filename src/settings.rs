//! Remembers interface choices between sessions (which panels are shown or
//! folded, the caption font) in a small text file in the app's data folder.

use std::collections::BTreeSet;
use std::path::PathBuf;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Prefs {
    /// Panels switched off in the View menu.
    pub hidden: BTreeSet<String>,
    /// Panels folded to their title.
    pub collapsed: BTreeSet<String>,
    /// Path of the last caption font, empty for the built-in one.
    pub caption_font: String,
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
                _ => {}
            }
        }
        p
    }

    pub fn to_text(&self) -> String {
        let join = |s: &BTreeSet<String>| s.iter().cloned().collect::<Vec<_>>().join(",");
        format!(
            "hidden={}\ncollapsed={}\ncaption_font={}\n",
            join(&self.hidden),
            join(&self.collapsed),
            self.caption_font
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
        assert_eq!(Prefs::parse(&p.to_text()), p);
    }
}
