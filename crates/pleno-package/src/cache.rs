//! The package cache: the files of each fetched commit, shared by every
//! project on the machine, so a commit is downloaded once.
//!
//! Layout: `<cache>/git/<repo>-<hash of the URL>/<commit>/`. A commit folder
//! is complete or absent: files are written to a scratch folder and renamed
//! into place.

use crate::{PackageError, git, manifest::normalize_git_url};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct Cache {
    pub root: PathBuf,
}

/// The folder plm keeps its data in: `$PLENO_HOME`, or `~/.pleno`.
pub fn pleno_home() -> PathBuf {
    if let Some(home) = std::env::var_os("PLENO_HOME").filter(|v| !v.is_empty()) {
        return PathBuf::from(home);
    }
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".pleno")
}

impl Cache {
    /// `$PLENO_CACHE_DIR`, or `<pleno home>/cache`.
    pub fn default_root() -> PathBuf {
        if let Some(dir) = std::env::var_os("PLENO_CACHE_DIR").filter(|v| !v.is_empty()) {
            return PathBuf::from(dir);
        }
        pleno_home().join("cache")
    }

    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    fn repo_dir(&self, url: &str) -> PathBuf {
        let normalized = normalize_git_url(url);
        let hash = hex::encode(Sha256::digest(normalized.as_bytes()));
        let base: String = crate::manifest::default_name(&normalized)
            .chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                    c
                } else {
                    '_'
                }
            })
            .collect();
        self.root
            .join("git")
            .join(format!("{base}-{}", &hash[..16]))
    }

    /// Where the files of `commit` of `url` are kept.
    pub fn commit_dir(&self, url: &str, commit: &str) -> PathBuf {
        self.repo_dir(url).join(commit)
    }

    /// A folder for scratch repositories (`git` needs one to fetch into).
    pub fn scratch_dir(&self, url: &str) -> PathBuf {
        self.repo_dir(url)
    }

    /// The files of `commit`, fetched now unless they are cached. With
    /// `offline`, a commit that is not cached is an error.
    pub fn ensure(
        &self,
        name: &str,
        url: &str,
        commit: &str,
        hint: Option<&str>,
        offline: bool,
    ) -> Result<PathBuf, PackageError> {
        let dir = self.commit_dir(url, commit);
        if dir.is_dir() {
            return Ok(dir);
        }
        if offline {
            return Err(PackageError::Offline(format!(
                "'{name}' (commit {commit} of {url}) is not in the cache at {}, and --offline forbids \
                 fetching it. Run `plm install` once with network access",
                self.root.display()
            )));
        }
        let staging = dir.with_file_name(format!("{commit}.part-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&staging);
        let result = git::fetch_commit_files(url, commit, hint, &staging);
        if let Err(err) = result {
            let _ = std::fs::remove_dir_all(&staging);
            return Err(err);
        }
        if std::fs::rename(&staging, &dir).is_err() {
            // Another plm finished first; its copy is the same commit.
            let _ = std::fs::remove_dir_all(&staging);
            if !dir.is_dir() {
                return Err(PackageError::Io(std::io::Error::other(format!(
                    "cannot write {} to the cache",
                    dir.display()
                ))));
            }
        }
        Ok(dir)
    }
}

/// The checksum of a package: `sha256:` and the hex SHA-256 of one line per
/// regular file, `<hex sha256 of the file>  <path>\n`, sorted by path (with
/// `/` separators). Folders, symbolic links and file modes do not count, so
/// the result is the same on every OS. This is Go's `h1:` dirhash with SHA-256
/// in hex.
pub fn checksum_dir(dir: &Path) -> Result<String, PackageError> {
    let mut files = Vec::new();
    collect_files(dir, dir, &mut files)?;
    files.sort();
    let mut summary = Sha256::new();
    for (path, full) in files {
        let bytes = std::fs::read(&full)?;
        let file_hash = hex::encode(Sha256::digest(&bytes));
        summary.update(format!("{file_hash}  {path}\n").as_bytes());
    }
    Ok(format!("sha256:{}", hex::encode(summary.finalize())))
}

fn collect_files(
    root: &Path,
    dir: &Path,
    out: &mut Vec<(String, PathBuf)>,
) -> Result<(), PackageError> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let kind = entry.file_type()?;
        let path = entry.path();
        if kind.is_dir() {
            collect_files(root, &path, out)?;
        } else if kind.is_file() {
            let relative = path
                .strip_prefix(root)
                .unwrap_or(&path)
                .components()
                .map(|c| c.as_os_str().to_string_lossy().into_owned())
                .collect::<Vec<_>>()
                .join("/");
            out.push((relative, path));
        }
    }
    Ok(())
}

/// Copies the regular files of `src` into `dst`, leaving out the top-level
/// folders in `skip`.
pub fn copy_dir(src: &Path, dst: &Path, skip: &[&str]) -> Result<(), PackageError> {
    if !src.is_dir() {
        return Err(PackageError::NotFound(src.to_path_buf()));
    }
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let name = entry.file_name();
        if skip.iter().any(|s| name == *s) {
            continue;
        }
        let kind = entry.file_type()?;
        let target = dst.join(&name);
        if kind.is_dir() {
            copy_dir(&entry.path(), &target, &[])?;
        } else if kind.is_file() {
            std::fs::copy(entry.path(), target)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checksum_depends_on_paths_and_contents() {
        let base = std::env::temp_dir().join(format!("pleno-checksum-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let a = base.join("a");
        std::fs::create_dir_all(a.join("src")).unwrap();
        std::fs::write(a.join("pleno.toml"), "x").unwrap();
        std::fs::write(a.join("src/lib.pln"), "y").unwrap();
        let first = checksum_dir(&a).unwrap();
        assert!(first.starts_with("sha256:"));

        let b = base.join("b");
        copy_dir(&a, &b, &[]).unwrap();
        assert_eq!(checksum_dir(&b).unwrap(), first);

        std::fs::write(b.join("src/lib.pln"), "z").unwrap();
        assert_ne!(checksum_dir(&b).unwrap(), first);
        let _ = std::fs::remove_dir_all(&base);
    }
}
