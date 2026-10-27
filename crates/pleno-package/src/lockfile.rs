//! `pleno.lock`: the exact commit and checksum of every installed package.

use crate::PackageError;
use serde::{Deserialize, Serialize};
use std::path::Path;

/// The lockfile format this plm writes. A lockfile without `version`, or with
/// another number, is from an older plm and is resolved again from scratch.
pub const LOCKFILE_VERSION: u32 = 1;

const HEADER: &str =
    "# Written by plm. Do not edit it by hand: run `plm install` or `plm update`.\n";

#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq)]
pub struct Lockfile {
    #[serde(default)]
    pub version: u32,
    #[serde(rename = "package", default)]
    pub packages: Vec<LockedPackage>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct LockedPackage {
    pub name: String,
    /// `git+<url>` or `path+<folder>`.
    pub source: String,
    /// The version of the tag, when a tag was chosen.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tag: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
    /// The full commit SHA (Git packages).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub commit: Option<String>,
    /// `sha256:<hex>` of the package's files (Git packages).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub checksum: Option<String>,
    /// The names of the packages this one depends on.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub dependencies: Vec<String>,
}

impl LockedPackage {
    pub fn git_url(&self) -> Option<&str> {
        self.source.strip_prefix("git+")
    }

    pub fn path(&self) -> Option<&str> {
        self.source.strip_prefix("path+")
    }

    /// `v1.2.0`, `branch main @ 3f2c9e1`, `rev 3f2c9e1` or `path ../greet`.
    pub fn describe(&self) -> String {
        let short = self
            .commit
            .as_deref()
            .map(|c| &c[..c.len().min(7)])
            .unwrap_or("?");
        if let Some(path) = self.path() {
            format!("path {path}")
        } else if let Some(version) = &self.version {
            format!("v{version}")
        } else if let Some(tag) = &self.tag {
            format!("tag {tag} @ {short}")
        } else if let Some(branch) = &self.branch {
            format!("branch {branch} @ {short}")
        } else {
            format!("rev {short}")
        }
    }
}

impl Lockfile {
    pub fn get(&self, name: &str) -> Option<&LockedPackage> {
        self.packages.iter().find(|p| p.name == name)
    }

    /// Reads `pleno.lock`; a missing file is an empty lockfile.
    pub fn load_from_file<P: AsRef<Path>>(path: P) -> Result<Self, PackageError> {
        let path = path.as_ref();
        if !path.exists() {
            return Ok(Self::default());
        }
        let content = std::fs::read_to_string(path)?;
        toml::from_str(&content).map_err(|e| {
            PackageError::Lockfile(format!(
                "{} is not a valid lockfile ({e}). Delete it and run `plm install` to write a new one",
                path.display()
            ))
        })
    }

    pub fn to_text(&self) -> Result<String, PackageError> {
        Ok(format!("{HEADER}\n{}", toml::to_string_pretty(self)?))
    }

    pub fn save_to_file<P: AsRef<Path>>(&self, path: P) -> Result<(), PackageError> {
        std::fs::write(path, self.to_text()?)?;
        Ok(())
    }
}
