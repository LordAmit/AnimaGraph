//! Which folders an assistant may use through MCP. Reading and saving are
//! only allowed inside these folders; everything else is refused.

use std::path::{Path, PathBuf};

/// Folders used when none are set: Pictures, Desktop and Downloads.
pub fn default_folders() -> Vec<PathBuf> {
    let Some(home) = std::env::var_os("HOME").map(PathBuf::from) else { return Vec::new() };
    ["Pictures", "Desktop", "Downloads"].iter().map(|d| home.join(d)).filter(|p| p.is_dir()).collect()
}

/// The folders in force: the ones set in the app, or the defaults.
pub fn allowed_folders(prefs: &crate::settings::Prefs) -> Vec<PathBuf> {
    if prefs.mcp_folders.is_empty() {
        default_folders()
    } else {
        prefs.mcp_folders.iter().map(PathBuf::from).collect()
    }
}

fn inside(path: &Path, folders: &[PathBuf]) -> bool {
    folders.iter().filter_map(|f| f.canonicalize().ok()).any(|f| path.starts_with(&f))
}

fn refused(path: &Path, folders: &[PathBuf]) -> String {
    let list: Vec<String> = folders.iter().map(|f| f.display().to_string()).collect();
    format!(
        "{} is outside the folders Animagraph allows ({}). Add the folder in Animagraph: View > Assistant access (MCP).",
        path.display(),
        if list.is_empty() { "none set".into() } else { list.join(", ") }
    )
}

/// An existing file the assistant may read.
pub fn check_read(path: &Path, folders: &[PathBuf]) -> Result<PathBuf, String> {
    let real = path.canonicalize().map_err(|e| format!("Cannot open {}: {e}", path.display()))?;
    if inside(&real, folders) { Ok(real) } else { Err(refused(path, folders)) }
}

/// A file the assistant may write: its folder must exist and be allowed.
pub fn check_write(path: &Path, folders: &[PathBuf]) -> Result<PathBuf, String> {
    let parent = path.parent().filter(|p| !p.as_os_str().is_empty()).unwrap_or(Path::new("."));
    let real_parent = parent.canonicalize().map_err(|e| format!("Cannot save into {}: {e}", parent.display()))?;
    let name = path.file_name().ok_or_else(|| format!("{} has no file name", path.display()))?;
    let target = real_parent.join(name);
    if inside(&target, folders) { Ok(target) } else { Err(refused(path, folders)) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_inside_allowed_folders() {
        let base = std::env::temp_dir().join(format!("animagraph_mcp_access_{}", std::process::id()));
        let ok = base.join("ok");
        let other = base.join("other");
        std::fs::create_dir_all(&ok).unwrap();
        std::fs::create_dir_all(&other).unwrap();
        std::fs::write(ok.join("a.jpg"), b"x").unwrap();
        std::fs::write(other.join("b.jpg"), b"x").unwrap();
        let folders = vec![ok.clone()];
        assert!(check_read(&ok.join("a.jpg"), &folders).is_ok());
        assert!(check_read(&other.join("b.jpg"), &folders).is_err());
        // "..": resolved before checking.
        assert!(check_read(&ok.join("../other/b.jpg"), &folders).is_err());
        assert!(check_write(&ok.join("new.png"), &folders).is_ok());
        assert!(check_write(&other.join("new.png"), &folders).is_err());
        let _ = std::fs::remove_dir_all(&base);
    }
}
