//! Static files for `router.static(prefix, folder)`.
//!
//! A request path is decoded, split into names and checked before it touches
//! the file system: `..`, `.`, backslashes, drive letters and NUL are
//! refused, names starting with `.` are not served, and the file found is
//! normalized the way the sandbox normalizes paths (symbolic links resolved)
//! and must still be inside the folder.

use super::cookies::percent_decode;
use indexmap::IndexMap;
use pleno_common::paths::{normalized_path, path_starts_with};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// What a static layer does with a request.
pub(crate) enum Outcome {
    /// Send this status, headers and body (no body for a 304).
    Serve {
        status: i64,
        headers: Vec<(String, String)>,
        body: Vec<u8>,
    },
    /// Not a file of this folder: the next middleware or route gets the request.
    Skip,
    /// A path that tries to leave the folder, or that the sandbox does not allow.
    Forbidden,
    /// A folder requested without its trailing `/`: redirect to `location`.
    Redirect(String),
}

/// The `Content-Type` for a file name, by its extension.
pub(crate) fn content_type(path: &Path) -> &'static str {
    let extension = path
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase)
        .unwrap_or_default();
    match extension.as_str() {
        "html" | "htm" => "text/html; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "js" | "mjs" => "text/javascript; charset=utf-8",
        "json" | "map" => "application/json",
        "txt" | "text" | "log" => "text/plain; charset=utf-8",
        "md" => "text/markdown; charset=utf-8",
        "csv" => "text/csv; charset=utf-8",
        "xml" => "application/xml",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "avif" => "image/avif",
        "ico" => "image/x-icon",
        "woff" => "font/woff",
        "woff2" => "font/woff2",
        "ttf" => "font/ttf",
        "otf" => "font/otf",
        "pdf" => "application/pdf",
        "wasm" => "application/wasm",
        "zip" => "application/zip",
        "mp3" => "audio/mpeg",
        "wav" => "audio/wav",
        "ogg" => "audio/ogg",
        "mp4" => "video/mp4",
        "webm" => "video/webm",
        "pln" => "text/plain; charset=utf-8",
        _ => "application/octet-stream",
    }
}

/// Names Windows maps to devices whatever the folder or extension.
fn is_windows_device(name: &str) -> bool {
    let stem = name.split('.').next().unwrap_or("").to_ascii_uppercase();
    matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || ((stem.starts_with("COM") || stem.starts_with("LPT"))
            && stem.len() == 4
            && stem.as_bytes()[3].is_ascii_digit())
}

/// The file names of a request path below the prefix, or why there are none.
fn safe_names(rest: &str) -> Result<Vec<String>, Outcome> {
    let decoded = percent_decode(rest, false).ok_or(Outcome::Skip)?;
    let mut names = Vec::new();
    for name in decoded.split('/') {
        if name.is_empty() {
            continue;
        }
        if name == "."
            || name == ".."
            || name.contains(['\\', ':', '\0'])
            || name.chars().any(char::is_control)
        {
            return Err(Outcome::Forbidden);
        }
        // Dotfiles (`.env`, `.git`) are never served.
        if name.starts_with('.') || (cfg!(windows) && is_windows_device(name)) {
            return Err(Outcome::Skip);
        }
        names.push(name.to_string());
    }
    Ok(names)
}

/// An HTTP date (RFC 9110 IMF-fixdate).
pub(crate) fn http_date(time: SystemTime) -> String {
    let time: chrono::DateTime<chrono::Utc> = time.into();
    time.format("%a, %d %b %Y %H:%M:%S GMT").to_string()
}

fn parse_http_date(text: &str) -> Option<u64> {
    let time = chrono::DateTime::parse_from_rfc2822(text.trim()).ok()?;
    u64::try_from(time.timestamp()).ok()
}

/// Serves `rest` (the request path after the prefix) from `root`, which is
/// already normalized. `is_read_allowed` is the sandbox's read check.
pub(crate) fn serve(
    root: &Path,
    raw_path: &str,
    rest: &str,
    headers: &IndexMap<String, String>,
    is_read_allowed: &dyn Fn(&str) -> bool,
) -> Outcome {
    let names = match safe_names(rest) {
        Ok(names) => names,
        Err(outcome) => return outcome,
    };
    let mut candidate = root.to_path_buf();
    for name in &names {
        candidate.push(name);
    }
    // Links are resolved here, so a link that points out of the folder is
    // caught by the comparison with the root.
    let Some(mut file) = normalized_path(&candidate) else {
        return Outcome::Skip;
    };
    if !path_starts_with(&file, root) || !is_read_allowed(&file.to_string_lossy()) {
        return Outcome::Forbidden;
    }
    let Ok(mut metadata) = std::fs::metadata(&file) else {
        return Outcome::Skip;
    };
    if metadata.is_dir() {
        if !raw_path.ends_with('/') {
            // `/docs` to `/docs/`, so links in its index.html resolve inside
            // it. Repeated slashes are collapsed: `//evil.example` must not
            // become a link to another site.
            let mut location = String::new();
            for c in raw_path.chars() {
                if !(c == '/' && location.ends_with('/')) {
                    location.push(c);
                }
            }
            location.push('/');
            return Outcome::Redirect(location);
        }
        file = match normalized_path(file.join("index.html")) {
            Some(index) if path_starts_with(&index, root) => index,
            _ => return Outcome::Forbidden,
        };
        metadata = match std::fs::metadata(&file) {
            Ok(m) if m.is_file() => m,
            _ => return Outcome::Skip,
        };
    }
    if !metadata.is_file() {
        return Outcome::Skip;
    }
    if !is_read_allowed(&file.to_string_lossy()) {
        return Outcome::Forbidden;
    }

    let modified = metadata.modified().ok();
    let modified_secs = modified
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_secs());
    let modified_nanos = modified
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let etag = format!("W/\"{:x}-{:x}\"", metadata.len(), modified_nanos);
    let mut response_headers = vec![
        ("ETag".to_string(), etag.clone()),
        ("Cache-Control".to_string(), "public, max-age=0".to_string()),
    ];
    if let Some(modified) = modified {
        response_headers.push(("Last-Modified".to_string(), http_date(modified)));
    }

    // If-None-Match wins over If-Modified-Since (RFC 9110 13.2.2).
    let not_modified = match headers.get("if-none-match") {
        Some(tags) => tags.split(',').any(|tag| {
            let tag = tag.trim();
            tag == "*" || tag.trim_start_matches("W/") == etag.trim_start_matches("W/")
        }),
        None => match (headers.get("if-modified-since"), modified_secs) {
            (Some(since), Some(modified)) => {
                parse_http_date(since).is_some_and(|since| modified <= since)
            }
            _ => false,
        },
    };
    if not_modified {
        return Outcome::Serve {
            status: 304,
            headers: response_headers,
            body: Vec::new(),
        };
    }

    let body = match std::fs::read(&file) {
        Ok(body) => body,
        Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => return Outcome::Forbidden,
        Err(_) => return Outcome::Skip,
    };
    response_headers.push(("Content-Type".to_string(), content_type(&file).to_string()));
    response_headers.push(("X-Content-Type-Options".to_string(), "nosniff".to_string()));
    Outcome::Serve {
        status: 200,
        headers: response_headers,
        body,
    }
}

/// The folder for `router.static`: it must exist and be a folder.
pub(crate) fn resolve_root(folder: &str) -> Result<PathBuf, String> {
    let root = normalized_path(folder).ok_or_else(|| format!("cannot resolve '{folder}'"))?;
    if !root.is_dir() {
        return Err(format!(
            "router.static(): '{folder}' is not a folder (resolved to {})",
            root.display()
        ));
    }
    Ok(root)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn refuses_names_that_leave_the_folder() {
        for path in [
            "/../secret",
            "/a/../../secret",
            "/%2e%2e/secret",
            "/%2E%2E%2fsecret",
            "/..%5csecret",
            "/C:/Windows/win.ini",
            "/a%00b",
        ] {
            assert!(
                matches!(safe_names(path), Err(Outcome::Forbidden)),
                "{path}"
            );
        }
        assert!(matches!(safe_names("/.env"), Err(Outcome::Skip)));
        assert_eq!(
            safe_names("//a//b%20c").ok(),
            Some(vec!["a".into(), "b c".into()])
        );
    }

    #[test]
    fn knows_common_content_types() {
        assert_eq!(
            content_type(Path::new("a.HTML")),
            "text/html; charset=utf-8"
        );
        assert_eq!(content_type(Path::new("a.png")), "image/png");
        assert_eq!(content_type(Path::new("a")), "application/octet-stream");
    }
}
