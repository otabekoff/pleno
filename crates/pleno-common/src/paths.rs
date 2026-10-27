//! Path normalization shared by the sandbox's path rules and the HTTP
//! server's static files, so both decide "is this inside that folder" the
//! same way.

use std::path::{Component, Path, PathBuf};

/// `path` made absolute against the working directory, with `.` and `..`
/// removed and symbolic links resolved in the part of it that exists.
pub fn normalized_path(path: impl AsRef<Path>) -> Option<PathBuf> {
    let path = path.as_ref();
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir().ok()?.join(path)
    };
    let mut lexical = PathBuf::new();
    for component in absolute.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                lexical.pop();
            }
            other => lexical.push(other.as_os_str()),
        }
    }
    // Resolve links in the longest part that exists; a file about to be
    // written does not exist yet.
    let mut existing = lexical.clone();
    let mut missing = Vec::new();
    while !existing.exists() {
        match existing.file_name() {
            Some(name) => {
                missing.push(name.to_os_string());
                existing.pop();
            }
            None => break,
        }
    }
    let mut resolved = std::fs::canonicalize(&existing)
        .map(strip_verbatim_prefix)
        .unwrap_or(existing);
    for name in missing.iter().rev() {
        resolved.push(name);
    }
    Some(resolved)
}

/// `C:\dir` for Windows' `\\?\C:\dir`, so a canonical path compares equal to
/// one that was not canonicalized.
pub fn strip_verbatim_prefix(path: PathBuf) -> PathBuf {
    match path.to_str().and_then(|text| text.strip_prefix(r"\\?\")) {
        Some(rest) if !rest.starts_with("UNC") => PathBuf::from(rest),
        _ => path,
    }
}

/// `path` starts with `base`, component by component; on Windows without
/// regard to case, as its file systems compare names.
pub fn path_starts_with(path: &Path, base: &Path) -> bool {
    if cfg!(windows) {
        let lower = |p: &Path| {
            p.components()
                .map(|c| c.as_os_str().to_string_lossy().to_lowercase())
                .collect::<Vec<_>>()
        };
        let (path, base) = (lower(path), lower(base));
        path.len() >= base.len() && path[..base.len()] == base[..]
    } else {
        path.starts_with(base)
    }
}

/// Whether `path` is inside one of the `allowed` folders (or is one of the
/// allowed files). Both sides are compared as normalized absolute paths, one
/// component at a time, so `data/../secret` is not inside `data` and `data2`
/// is not inside `data`.
pub fn path_allowed(path: &str, allowed: &[String]) -> bool {
    let Some(path) = normalized_path(path) else {
        return false;
    };
    allowed
        .iter()
        .filter_map(normalized_path)
        .any(|base| path_starts_with(&path, &base))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parent_components_cannot_leave_a_folder() {
        let base = normalized_path("data").unwrap();
        assert!(path_starts_with(
            &normalized_path("data/file.txt").unwrap(),
            &base
        ));
        assert!(!path_starts_with(
            &normalized_path("data/../secret").unwrap(),
            &base
        ));
        assert!(!path_starts_with(&normalized_path("data2").unwrap(), &base));
    }
}
