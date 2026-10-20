//! Native file dialogs on Windows; elsewhere the app falls back to an in-app
//! path prompt (WSL/Linux dev builds often lack GTK or an XDG portal).

use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PathPurpose {
    Open,
    SaveAs,
}

/// `Some(result)` when a native dialog ran (`result` is `None` if cancelled);
/// `None` when no native dialog is available and the caller should prompt.
#[cfg(windows)]
pub fn native(purpose: PathPurpose, suggested_name: &str) -> Option<Option<PathBuf>> {
    let dlg = rfd::FileDialog::new().add_filter("NetVis project", &[netvis_core::io::FILE_EXTENSION]);
    Some(match purpose {
        PathPurpose::Open => dlg.pick_file(),
        PathPurpose::SaveAs => dlg.set_file_name(suggested_name).save_file(),
    })
}

#[cfg(not(windows))]
pub fn native(_purpose: PathPurpose, _suggested_name: &str) -> Option<Option<PathBuf>> {
    None
}

/// Ensure the path carries the `.netvis` extension.
pub fn with_extension(mut path: PathBuf) -> PathBuf {
    let ext = netvis_core::io::FILE_EXTENSION;
    if path.extension().is_none_or(|e| !e.eq_ignore_ascii_case(ext)) {
        let mut s = path.into_os_string();
        s.push(".");
        s.push(ext);
        path = s.into();
    }
    path
}

pub fn default_dir() -> PathBuf {
    directories::UserDirs::new()
        .and_then(|u| u.document_dir().map(|d| d.to_path_buf()))
        .or_else(|| directories::UserDirs::new().map(|u| u.home_dir().to_path_buf()))
        .unwrap_or_else(|| PathBuf::from("."))
}

/// File-system-safe version of a project name.
pub fn file_stem_for(name: &str) -> String {
    let s: String = name
        .chars()
        .map(|c| if c.is_alphanumeric() || matches!(c, '-' | '_' | ' ') { c } else { '_' })
        .collect();
    let s = s.trim();
    if s.is_empty() { "network".to_owned() } else { s.to_owned() }
}
