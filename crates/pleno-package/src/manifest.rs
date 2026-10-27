//! `pleno.toml`: the package manifest, and what each dependency asks for.

use crate::PackageError;
use semver::VersionReq;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Manifest {
    pub package: PackageSection,
    #[serde(default)]
    pub scripts: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub features: BTreeMap<String, Vec<String>>,
    #[serde(default)]
    pub dependencies: BTreeMap<String, Dependency>,
    #[serde(
        rename = "dev-dependencies",
        default,
        skip_serializing_if = "BTreeMap::is_empty"
    )]
    pub dev_dependencies: BTreeMap<String, Dependency>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PackageSection {
    pub name: String,
    pub version: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default = "default_main")]
    pub main: String,
}

fn default_main() -> String {
    "src/main.pln".to_string()
}

/// One entry of `[dependencies]`. Only the table form is valid; a bare string
/// such as `"^1.2"` parses so that plm can explain what to write instead.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(untagged)]
pub enum Dependency {
    Version(String),
    Detailed(DependencyDetail),
}

#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq)]
pub struct DependencyDetail {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub git: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tag: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rev: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub optional: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub features: Option<Vec<String>>,
}

/// Where a dependency comes from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DepSource {
    /// A Git URL, as written.
    Git(String),
    /// A folder, as written (relative to the manifest that names it).
    Path(String),
}

/// Which commit of a Git dependency is wanted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Want {
    /// The highest `vX.Y.Z` tag that matches.
    Range(VersionReq),
    /// Exactly this tag.
    Tag(String),
    /// Exactly this commit (a full or abbreviated SHA).
    Rev(String),
    /// The newest commit of this branch, when the lockfile is written.
    Branch(String),
}

impl Want {
    pub fn describe(&self) -> String {
        match self {
            Want::Range(req) => format!("version {req}"),
            Want::Tag(tag) => format!("tag {tag}"),
            Want::Rev(rev) => format!("rev {rev}"),
            Want::Branch(branch) => format!("branch {branch}"),
        }
    }
}

/// Parses a version requirement as written in `pleno.toml`: Cargo's syntax,
/// where `1.2` means `^1.2`. A leading `v` (`v1.2`) is accepted.
pub fn parse_requirement(text: &str) -> Result<VersionReq, PackageError> {
    let trimmed = text.trim();
    let without_v = trimmed
        .strip_prefix('v')
        .filter(|rest| rest.starts_with(|c: char| c.is_ascii_digit()))
        .unwrap_or(trimmed);
    VersionReq::parse(without_v).map_err(|_| PackageError::InvalidVersionReq(text.to_string()))
}

impl Dependency {
    pub fn is_optional(&self) -> bool {
        matches!(self, Dependency::Detailed(d) if d.optional == Some(true))
    }

    pub fn requested_features(&self) -> &[String] {
        match self {
            Dependency::Detailed(d) => d.features.as_deref().unwrap_or(&[]),
            Dependency::Version(_) => &[],
        }
    }

    /// The source of this dependency and, for Git, which commit is wanted.
    pub fn spec(&self, name: &str) -> Result<(DepSource, Option<Want>), PackageError> {
        let detail = match self {
            Dependency::Version(text) => {
                return Err(PackageError::InvalidManifest(format!(
                    "dependency '{name}' = \"{text}\" does not say where the package is. Pleno has no central \
                     registry: packages are Git repositories. Write it as \
                     {name} = {{ git = \"https://github.com/<owner>/{name}\", version = \"{text}\" }}"
                )));
            }
            Dependency::Detailed(detail) => detail,
        };
        let pins = [
            detail.version.as_ref().map(|_| "version"),
            detail.tag.as_ref().map(|_| "tag"),
            detail.rev.as_ref().map(|_| "rev"),
            detail.branch.as_ref().map(|_| "branch"),
        ];
        let given: Vec<&str> = pins.iter().flatten().copied().collect();
        match (&detail.git, &detail.path) {
            (Some(_), Some(_)) => Err(PackageError::InvalidManifest(format!(
                "dependency '{name}' has both 'git' and 'path'; give one"
            ))),
            (None, None) => Err(PackageError::InvalidManifest(format!(
                "dependency '{name}' needs 'git' (a repository URL) or 'path' (a folder)"
            ))),
            (None, Some(path)) => {
                if let Some(key) = given.first() {
                    return Err(PackageError::InvalidManifest(format!(
                        "dependency '{name}' is a path, so '{key}' does not apply; remove it"
                    )));
                }
                Ok((DepSource::Path(path.clone()), None))
            }
            (Some(url), None) => {
                if url.trim().is_empty() {
                    return Err(PackageError::InvalidManifest(format!(
                        "dependency '{name}' has an empty 'git' URL"
                    )));
                }
                if given.len() > 1 {
                    return Err(PackageError::InvalidManifest(format!(
                        "dependency '{name}' gives {}; give only one of version, tag, rev and branch",
                        given.join(" and ")
                    )));
                }
                let want = if let Some(version) = &detail.version {
                    Want::Range(parse_requirement(version).map_err(|_| {
                        PackageError::InvalidManifest(format!(
                            "dependency '{name}' has an invalid version requirement '{version}' \
                             (examples: \"^1.2\", \"~1.2.3\", \">=1.0, <2.0\", \"=1.4.0\")"
                        ))
                    })?)
                } else if let Some(tag) = &detail.tag {
                    Want::Tag(tag.clone())
                } else if let Some(rev) = &detail.rev {
                    let rev = rev.trim().to_ascii_lowercase();
                    if rev.len() < 7 || !rev.chars().all(|c| c.is_ascii_hexdigit()) {
                        return Err(PackageError::InvalidManifest(format!(
                            "dependency '{name}' has rev '{rev}': a rev is a commit SHA of at least 7 hex digits"
                        )));
                    }
                    Want::Rev(rev)
                } else if let Some(branch) = &detail.branch {
                    Want::Branch(branch.clone())
                } else {
                    // No pin: the newest release.
                    Want::Range(VersionReq::STAR)
                };
                Ok((DepSource::Git(url.trim().to_string()), Some(want)))
            }
        }
    }
}

impl Manifest {
    pub fn new(name: &str, version: &str) -> Self {
        Self {
            package: PackageSection {
                name: name.to_string(),
                version: version.to_string(),
                description: None,
                main: default_main(),
            },
            scripts: BTreeMap::new(),
            features: BTreeMap::new(),
            dependencies: BTreeMap::new(),
            dev_dependencies: BTreeMap::new(),
        }
    }

    pub fn parse(text: &str) -> Result<Self, PackageError> {
        let manifest: Manifest = toml::from_str(text)?;
        if manifest.package.name.is_empty() {
            return Err(PackageError::MissingName);
        }
        Ok(manifest)
    }

    pub fn load_from_file<P: AsRef<Path>>(path: P) -> Result<Self, PackageError> {
        let path = path.as_ref();
        if !path.exists() {
            return Err(PackageError::NotFound(path.to_path_buf()));
        }
        let content = std::fs::read_to_string(path)?;
        Self::parse(&content).map_err(|err| match err {
            PackageError::TomlParse(e) => {
                PackageError::InvalidManifest(format!("{}: {e}", path.display()))
            }
            other => other,
        })
    }

    pub fn save_to_file<P: AsRef<Path>>(&self, path: P) -> Result<(), PackageError> {
        let content = toml::to_string_pretty(self)?;
        std::fs::write(path, content)?;
        Ok(())
    }

    /// Checks every dependency entry, so a mistake is reported before any
    /// network access.
    pub fn validate(&self) -> Result<(), PackageError> {
        for (name, dep) in self.dependencies.iter().chain(&self.dev_dependencies) {
            check_name(name)?;
            dep.spec(name)?;
        }
        Ok(())
    }
}

/// A dependency name is also the folder under `packages/` and the name in
/// `import ... from "name"`, so it is kept to letters, digits, `-`, `_` and `.`.
pub fn check_name(name: &str) -> Result<(), PackageError> {
    let ok = !name.is_empty()
        && !name.starts_with('.')
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'));
    if ok {
        Ok(())
    } else {
        Err(PackageError::InvalidManifest(format!(
            "'{name}' is not a valid dependency name: use letters, digits, '-', '_' and '.'"
        )))
    }
}

/// The form of a Git URL used to compare two URLs and to name cache folders:
/// no trailing `/` or `.git`, and the scheme and host in lower case, so
/// `https://GitHub.com/Owner/Repo.git` and `https://github.com/Owner/Repo` are
/// the same repository.
pub fn normalize_git_url(url: &str) -> String {
    let mut url = url.trim().trim_end_matches('/').to_string();
    if let Some(stripped) = url.strip_suffix(".git") {
        url = stripped.to_string();
    }
    if let Some(pos) = url.find("://") {
        let (scheme, rest) = url.split_at(pos + 3);
        let host_end = rest.find('/').unwrap_or(rest.len());
        let (host, path) = rest.split_at(host_end);
        url = format!(
            "{}{}{}",
            scheme.to_ascii_lowercase(),
            host.to_ascii_lowercase(),
            path
        );
    }
    url
}

/// The last part of a Git URL or path without `.git`: the default name of a
/// dependency (`https://github.com/ada/greet.git` → `greet`).
pub fn default_name(url_or_path: &str) -> String {
    let trimmed = url_or_path
        .trim()
        .trim_end_matches(['/', '\\'])
        .trim_end_matches(".git");
    trimmed
        .rsplit(['/', '\\', ':'])
        .next()
        .unwrap_or(trimmed)
        .to_string()
}

/// Edits `pleno.toml` in place, keeping its comments and layout.
pub struct ManifestEditor {
    doc: toml_edit::DocumentMut,
}

impl ManifestEditor {
    pub fn parse(text: &str) -> Result<Self, PackageError> {
        let doc = text
            .parse::<toml_edit::DocumentMut>()
            .map_err(|e| PackageError::InvalidManifest(format!("pleno.toml: {e}")))?;
        Ok(Self { doc })
    }

    /// Adds or replaces a dependency, in `[dependencies]` or, with `dev`, in
    /// `[dev-dependencies]`, and removes it from the other table.
    pub fn set_dependency(&mut self, name: &str, detail: &DependencyDetail, dev: bool) {
        let (table, other) = if dev {
            ("dev-dependencies", "dependencies")
        } else {
            ("dependencies", "dev-dependencies")
        };
        if let Some(other) = self.doc.get_mut(other).and_then(|t| t.as_table_like_mut()) {
            other.remove(name);
        }
        let mut inline = toml_edit::InlineTable::new();
        let fields = [
            ("git", &detail.git),
            ("path", &detail.path),
            ("version", &detail.version),
            ("tag", &detail.tag),
            ("rev", &detail.rev),
            ("branch", &detail.branch),
        ];
        for (key, value) in fields {
            if let Some(value) = value {
                inline.insert(key, value.as_str().into());
            }
        }
        if !self.doc.contains_key(table) {
            self.doc[table] = toml_edit::table();
        }
        let deps = &mut self.doc[table];
        deps[name] = toml_edit::value(inline);
    }

    /// Removes a dependency from either table; false if it was in neither.
    pub fn remove_dependency(&mut self, name: &str) -> bool {
        let mut removed = false;
        for table in ["dependencies", "dev-dependencies"] {
            if let Some(t) = self.doc.get_mut(table).and_then(|t| t.as_table_like_mut()) {
                removed |= t.remove(name).is_some();
            }
        }
        removed
    }

    pub fn to_text(&self) -> String {
        self.doc.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dep(text: &str) -> Dependency {
        let table: toml::Table = toml::from_str(&format!("d = {text}")).unwrap();
        table["d"].clone().try_into().unwrap()
    }

    #[test]
    fn dependency_specs() {
        let (source, want) = dep(r#"{ git = "https://x/a", version = "^1.2" }"#)
            .spec("a")
            .unwrap();
        assert_eq!(source, DepSource::Git("https://x/a".into()));
        assert_eq!(want, Some(Want::Range(VersionReq::parse("^1.2").unwrap())));
        let (_, want) = dep(r#"{ git = "https://x/a" }"#).spec("a").unwrap();
        assert_eq!(want, Some(Want::Range(VersionReq::STAR)));
        let (_, want) = dep(r#"{ git = "https://x/a", tag = "v1.0.0" }"#)
            .spec("a")
            .unwrap();
        assert_eq!(want, Some(Want::Tag("v1.0.0".into())));
        let (source, want) = dep(r#"{ path = "../a" }"#).spec("a").unwrap();
        assert_eq!(source, DepSource::Path("../a".into()));
        assert_eq!(want, None);
    }

    #[test]
    fn invalid_dependency_specs_explain_themselves() {
        let err = dep(r#""^1.2""#).spec("a").unwrap_err().to_string();
        assert!(err.contains("no central registry"), "{err}");
        let err = dep(r#"{ git = "https://x/a", tag = "v1", branch = "main" }"#)
            .spec("a")
            .unwrap_err()
            .to_string();
        assert!(err.contains("tag and branch"), "{err}");
        let err = dep(r#"{ git = "https://x/a", rev = "xyz" }"#)
            .spec("a")
            .unwrap_err()
            .to_string();
        assert!(err.contains("commit SHA"), "{err}");
        let err = dep(r#"{ path = "../a", version = "1" }"#)
            .spec("a")
            .unwrap_err()
            .to_string();
        assert!(err.contains("is a path"), "{err}");
    }

    #[test]
    fn urls_normalize() {
        assert_eq!(
            normalize_git_url("https://GitHub.com/Ada/Greet.git/"),
            "https://github.com/Ada/Greet"
        );
        assert_eq!(default_name("https://github.com/ada/greet.git"), "greet");
        assert_eq!(default_name("git@github.com:ada/greet.git"), "greet");
        assert_eq!(default_name("../libs/greet/"), "greet");
    }

    #[test]
    fn editor_keeps_comments() {
        let text = "# my app\n[package]\nname = \"app\"\nversion = \"0.1.0\"\n\n[dependencies]\n# keep me\nold = { path = \"../old\" }\n";
        let mut editor = ManifestEditor::parse(text).unwrap();
        editor.set_dependency(
            "greet",
            &DependencyDetail {
                git: Some("https://x/greet".into()),
                version: Some("^1.0.0".into()),
                ..Default::default()
            },
            false,
        );
        assert!(editor.remove_dependency("old"));
        assert!(!editor.remove_dependency("missing"));
        let out = editor.to_text();
        assert!(out.contains("# my app"), "{out}");
        assert!(
            out.contains(r#"greet = { git = "https://x/greet", version = "^1.0.0" }"#),
            "{out}"
        );
        let manifest = Manifest::parse(&out).unwrap();
        assert!(manifest.dependencies.contains_key("greet"));
        assert!(!manifest.dependencies.contains_key("old"));
    }
}
