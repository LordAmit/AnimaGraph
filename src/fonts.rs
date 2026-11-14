//! Fonts for captions: the built-in font, fonts installed on the computer,
//! and fonts the person adds. Added fonts are copied into the app's own
//! data folder so they are still there next time:
//!   macOS:   ~/Library/Application Support/Animagraph/fonts
//!   Windows: %APPDATA%\Animagraph\fonts
//!   Linux:   ~/.local/share/animagraph/fonts

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum FontSource {
    BuiltIn,
    Added,
    Installed,
}

#[derive(Clone, PartialEq, Eq, Debug, Hash)]
pub struct FontEntry {
    pub name: String,
    pub source: FontSource,
    /// `None` for the built-in font.
    pub path: Option<PathBuf>,
}

impl FontEntry {
    pub fn builtin() -> Self {
        Self { name: "Default".into(), source: FontSource::BuiltIn, path: None }
    }
}

/// The app's data folder (created when needed).
pub fn data_dir() -> PathBuf {
    let home = std::env::var_os("HOME").map(PathBuf::from).unwrap_or_else(std::env::temp_dir);
    #[cfg(target_os = "macos")]
    let dir = home.join("Library/Application Support/Animagraph");
    #[cfg(target_os = "windows")]
    let dir = std::env::var_os("APPDATA").map(PathBuf::from).unwrap_or(home).join("Animagraph");
    #[cfg(all(unix, not(target_os = "macos")))]
    let dir = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| home.join(".local/share"))
        .join("animagraph");
    dir
}

pub fn fonts_dir() -> PathBuf {
    data_dir().join("fonts")
}

fn is_font_file(p: &Path) -> bool {
    matches!(
        p.extension().and_then(|e| e.to_str()).map(|e| e.to_ascii_lowercase()).as_deref(),
        Some("ttf" | "otf" | "ttc")
    )
}

fn display_name(p: &Path) -> String {
    p.file_stem().map(|s| nice_name(&s.to_string_lossy())).unwrap_or_default()
}

/// A readable font name from a file name, for example
/// "ShantellSans-Italic-VariableFont_BNCE,INFM,SPAC,wght" becomes
/// "Shantell Sans Italic (variable)".
pub fn nice_name(stem: &str) -> String {
    let mut s = stem.to_string();
    // Axis lists of variable fonts: "[wght]" or "_BNCE,INFM,wght".
    if let Some(i) = s.find('[') {
        s.truncate(i);
    }
    let variable = s.contains("VariableFont") || stem.contains('[');
    if let Some(i) = s.find("VariableFont") {
        s.truncate(i);
    }
    // Words: split on separators and between lower and upper case letters.
    let mut words: Vec<String> = Vec::new();
    for part in s.split(['-', '_', ' ', ',']).filter(|p| !p.is_empty()) {
        let mut cur = String::new();
        let chars: Vec<char> = part.chars().collect();
        for (i, &c) in chars.iter().enumerate() {
            let prev_lower = i > 0 && chars[i - 1].is_lowercase();
            if c.is_uppercase() && prev_lower && !cur.is_empty() {
                words.push(std::mem::take(&mut cur));
            }
            cur.push(c);
        }
        if !cur.is_empty() {
            words.push(cur);
        }
    }
    let mut name = words.join(" ");
    if name.is_empty() {
        name = stem.to_string();
    }
    if variable {
        name.push_str(" (variable)");
    }
    name
}

fn list_dir(dir: &Path, source: FontSource, depth: u32, out: &mut Vec<FontEntry>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for e in entries.flatten() {
        let p = e.path();
        if p.is_dir() {
            if depth > 0 {
                list_dir(&p, source, depth - 1, out);
            }
        } else if is_font_file(&p) {
            out.push(FontEntry { name: display_name(&p), source, path: Some(p) });
        }
    }
}

/// Fonts the person added, in name order.
pub fn added_fonts() -> Vec<FontEntry> {
    let mut out = Vec::new();
    list_dir(&fonts_dir(), FontSource::Added, 0, &mut out);
    out.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    out
}

/// Fonts installed on this computer, in name order.
pub fn installed_fonts() -> Vec<FontEntry> {
    let mut dirs: Vec<PathBuf> = Vec::new();
    let home = std::env::var_os("HOME").map(PathBuf::from);
    #[cfg(target_os = "macos")]
    {
        dirs.push("/System/Library/Fonts".into());
        dirs.push("/Library/Fonts".into());
        if let Some(h) = &home {
            dirs.push(h.join("Library/Fonts"));
        }
    }
    #[cfg(target_os = "windows")]
    {
        dirs.push("C:\\Windows\\Fonts".into());
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        dirs.push("/usr/share/fonts".into());
        if let Some(h) = &home {
            dirs.push(h.join(".local/share/fonts"));
        }
    }
    let _ = &home;
    let mut out = Vec::new();
    for d in dirs {
        list_dir(&d, FontSource::Installed, 2, &mut out);
    }
    out.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    out.dedup_by(|a, b| a.name == b.name);
    out
}

/// Copy a font file into the app's fonts folder. Returns the stored entry.
pub fn add_font(src: &Path) -> Result<FontEntry, String> {
    if !is_font_file(src) {
        return Err("Choose a .ttf, .otf or .ttc font file.".into());
    }
    let bytes = std::fs::read(src).map_err(|e| e.to_string())?;
    ab_glyph::FontRef::try_from_slice(&bytes).map_err(|_| "That file is not a font this app can read.".to_string())?;
    let dir = fonts_dir();
    std::fs::create_dir_all(&dir).map_err(|e| format!("Could not create {}: {e}", dir.display()))?;
    let file_name = src.file_name().ok_or("No file name")?;
    let dest = dir.join(file_name);
    std::fs::write(&dest, &bytes).map_err(|e| e.to_string())?;
    Ok(FontEntry { name: display_name(&dest), source: FontSource::Added, path: Some(dest) })
}

/// Remove a font the person added (only files inside the app's fonts folder).
pub fn remove_added_font(entry: &FontEntry) -> Result<(), String> {
    let Some(p) = &entry.path else { return Ok(()) };
    if entry.source != FontSource::Added || !p.starts_with(fonts_dir()) {
        return Err("Only fonts you added can be removed here.".into());
    }
    std::fs::remove_file(p).map_err(|e| e.to_string())
}

static CACHE: OnceLock<Mutex<HashMap<Option<PathBuf>, Arc<Vec<u8>>>>> = OnceLock::new();

/// The font file's bytes (read once, then kept).
pub fn font_bytes(entry: &FontEntry) -> Option<Arc<Vec<u8>>> {
    let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    if let Some(b) = cache.lock().ok()?.get(&entry.path) {
        return Some(b.clone());
    }
    let bytes = match &entry.path {
        Some(p) => std::fs::read(p).ok()?,
        None => builtin_bytes(),
    };
    let b = Arc::new(bytes);
    cache.lock().ok()?.insert(entry.path.clone(), b.clone());
    Some(b)
}

fn builtin_bytes() -> Vec<u8> {
    let defs = eframe::egui::FontDefinitions::default();
    defs.font_data
        .get("Ubuntu-Light")
        .or_else(|| defs.font_data.values().next())
        .map(|f| {
            let blob: &dyn AsRef<[u8]> = &*f.font;
            blob.as_ref().to_vec()
        })
        .unwrap_or_default()
}

/// Fonts larger than this are listed without a preview (reading them
/// would make the list slow).
const PREVIEW_MAX_BYTES: u64 = 30 * 1024 * 1024;

/// `text` drawn in the font as white letters on a transparent strip,
/// `px` pixels tall. `None` when the font can't be read or lacks letters
/// for the text (symbol and emoji fonts).
pub fn render_sample(entry: &FontEntry, text: &str, px: f32) -> Option<image::RgbaImage> {
    use ab_glyph::{Font, FontRef, PxScale, ScaleFont, point};
    let bytes = match &entry.path {
        Some(p) => {
            if std::fs::metadata(p).ok()?.len() > PREVIEW_MAX_BYTES {
                return None;
            }
            std::fs::read(p).ok()?
        }
        None => builtin_bytes(),
    };
    let font = FontRef::try_from_slice_and_index(&bytes, 0).ok()?;
    if text.chars().filter(|c| !c.is_whitespace()).any(|c| font.glyph_id(c).0 == 0) {
        return None;
    }
    let scale = PxScale::from(px);
    let scaled = font.as_scaled(scale);
    let mut width = 0.0f32;
    let mut prev = None;
    for c in text.chars() {
        let id = font.glyph_id(c);
        if let Some(p) = prev {
            width += scaled.kern(p, id);
        }
        width += scaled.h_advance(id);
        prev = Some(id);
    }
    let (w, h) = ((width.ceil() as u32 + 2).min(1200), (scaled.height().ceil() as u32 + 2).max(4));
    let mut cover = vec![0.0f32; (w * h) as usize];
    let mut x = 1.0;
    let mut prev = None;
    for c in text.chars() {
        let id = font.glyph_id(c);
        if let Some(p) = prev {
            x += scaled.kern(p, id);
        }
        let g = id.with_scale_and_position(scale, point(x, 1.0 + scaled.ascent()));
        x += scaled.h_advance(id);
        prev = Some(id);
        if let Some(o) = font.outline_glyph(g) {
            let b = o.px_bounds();
            o.draw(|gx, gy, c| {
                let xx = b.min.x as i32 + gx as i32;
                let yy = b.min.y as i32 + gy as i32;
                if xx >= 0 && yy >= 0 && (xx as u32) < w && (yy as u32) < h {
                    let i = (yy as u32 * w + xx as u32) as usize;
                    cover[i] = (cover[i] + c).min(1.0);
                }
            });
        }
    }
    if cover.iter().all(|c| *c == 0.0) {
        return None;
    }
    Some(image::RgbaImage::from_fn(w, h, |x, y| {
        image::Rgba([255, 255, 255, (cover[(y * w + x) as usize] * 255.0) as u8])
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn font_names_read_cleanly() {
        assert_eq!(nice_name("ShantellSans-Italic-VariableFont_BNCE,INFM,SPAC,wght"), "Shantell Sans Italic (variable)");
        assert_eq!(nice_name("Inter[opsz,wght]"), "Inter (variable)");
        assert_eq!(nice_name("Helvetica"), "Helvetica");
        assert_eq!(nice_name("Arial Bold"), "Arial Bold");
        assert_eq!(nice_name("SFNS"), "SFNS");
    }

    #[test]
    fn sample_renders_in_the_font() {
        let img = render_sample(&FontEntry::builtin(), "Default", 40.0).unwrap();
        assert!(img.width() > 60 && img.height() >= 40);
        assert!(img.pixels().any(|p| p[3] > 200));
    }

    #[test]
    fn builtin_font_is_readable() {
        let b = font_bytes(&FontEntry::builtin()).unwrap();
        assert!(ab_glyph::FontRef::try_from_slice(&b).is_ok());
    }
}
