//! Pleno's package manager.
//!
//! A package is a Git repository with a `pleno.toml` at its top. A version
//! of a package is a Git tag `vX.Y.Z`. There is no central registry: `plm`
//! reads tags with `git ls-remote`, fetches the commit it chose into a cache
//! shared by all projects, copies it into the project's `packages/` folder
//! and records the commit and a checksum of its files in `pleno.lock`.
//! `docs/specs/package-manager.md` describes the protocol.

// The errors carry the details a user needs (URLs, commits, checksums); they
// are rare and come after git or disk work, so their size does not matter.
#![allow(clippy::result_large_err)]

pub mod cache;
mod error;
pub mod git;
pub mod lockfile;
pub mod manifest;
pub mod publish;
pub mod resolve;
pub mod spec;

pub use cache::{Cache, checksum_dir};
pub use error::PackageError;
pub use lockfile::{LOCKFILE_VERSION, LockedPackage, Lockfile};
pub use manifest::{
    DepSource, Dependency, DependencyDetail, Manifest, ManifestEditor, PackageSection, Want,
};
pub use resolve::{ResolvedPackage, ResolvedSource, Resolver, Unlock};
pub use spec::{AddSpec, parse_add_spec};

use serde::{Deserialize, Serialize};
use std::collections::{BTreeSet, HashMap};
use std::path::{Path, PathBuf};

#[derive(Debug, Serialize, Deserialize)]
pub struct Bundle {
    pub name: String,
    pub version: String,
    pub main_module: String,
    pub modules: HashMap<String, pleno_bytecode::Chunk>,
}

/// A change `plm` made to the installed packages.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Change {
    Added {
        name: String,
        to: String,
    },
    Updated {
        name: String,
        from: String,
        to: String,
    },
    Removed {
        name: String,
        from: String,
    },
}

impl std::fmt::Display for Change {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Change::Added { name, to } => write!(f, "+ {name} {to}"),
            Change::Updated { name, from, to } => write!(f, "~ {name} {from} -> {to}"),
            Change::Removed { name, from } => write!(f, "- {name} {from}"),
        }
    }
}

/// What an install, update, add or remove did.
#[derive(Debug, Clone, Default)]
pub struct Report {
    pub changes: Vec<Change>,
    pub lockfile: Lockfile,
}

impl Report {
    pub fn installed(&self) -> usize {
        self.lockfile.packages.len()
    }
}

/// What `plm add` should add.
#[derive(Debug, Clone)]
pub struct AddRequest {
    /// The dependency name; by default the repository or folder name.
    pub name: Option<String>,
    pub source: DepSource,
    /// The pin; `None` for a Git source means "the newest tag, as `^X.Y.Z`".
    pub want: Option<Want>,
    /// The requirement exactly as typed, kept in pleno.toml (`^1.2`).
    pub version_text: Option<String>,
    pub dev: bool,
}

/// A project: the folder with `pleno.toml`, `pleno.lock` and `packages/`.
pub struct PackageManager {
    pub root: PathBuf,
    pub cache: Cache,
    pub offline: bool,
}

impl PackageManager {
    /// A project in `root`, with the cache from `PLENO_CACHE_DIR` or
    /// `~/.pleno/cache`.
    pub fn new<P: AsRef<Path>>(root: P) -> Self {
        Self {
            root: root.as_ref().to_path_buf(),
            cache: Cache::new(Cache::default_root()),
            offline: false,
        }
    }

    pub fn with_cache_dir(mut self, dir: impl Into<PathBuf>) -> Self {
        self.cache = Cache::new(dir);
        self
    }

    pub fn with_offline(mut self, offline: bool) -> Self {
        self.offline = offline;
        self
    }

    pub fn manifest_path(&self) -> PathBuf {
        self.root.join("pleno.toml")
    }

    pub fn lock_path(&self) -> PathBuf {
        self.root.join("pleno.lock")
    }

    pub fn packages_dir(&self) -> PathBuf {
        self.root.join("packages")
    }

    pub fn load_manifest(&self) -> Result<Manifest, PackageError> {
        Manifest::load_from_file(self.manifest_path())
    }

    /// Installs what `pleno.toml` asks for, keeping every locked commit that
    /// still fits. With a complete lockfile and cache it needs no network.
    pub fn install(&self, features: &[String]) -> Result<Report, PackageError> {
        let manifest = self.load_manifest()?;
        self.sync(&manifest, Unlock::None, features)
    }

    /// Chooses again, ignoring `pleno.lock`, for `names` (all when empty).
    pub fn update(&self, names: &[String]) -> Result<Report, PackageError> {
        let manifest = self.load_manifest()?;
        let unlock = if names.is_empty() {
            Unlock::All
        } else {
            let lock = Lockfile::load_from_file(self.lock_path())?;
            for name in names {
                let known = lock.get(name).is_some()
                    || manifest.dependencies.contains_key(name)
                    || manifest.dev_dependencies.contains_key(name);
                if !known {
                    return Err(PackageError::InvalidManifest(format!(
                        "'{name}' is not a dependency of this project (see `plm tree`)"
                    )));
                }
            }
            Unlock::Names(names.iter().cloned().collect())
        };
        self.sync(&manifest, unlock, &[])
    }

    /// Adds a dependency to `pleno.toml` and installs it. `pleno.toml` is
    /// only changed when the install succeeds.
    pub fn add(&self, request: &AddRequest) -> Result<(String, Report), PackageError> {
        let text = std::fs::read_to_string(self.manifest_path())
            .map_err(|_| PackageError::NotFound(self.manifest_path()))?;
        let name = match &request.name {
            Some(name) => name.clone(),
            None => match &request.source {
                DepSource::Git(url) => manifest::default_name(url),
                DepSource::Path(path) => manifest::default_name(path),
            },
        };
        manifest::check_name(&name)?;

        let mut detail = DependencyDetail::default();
        match &request.source {
            DepSource::Git(url) => {
                detail.git = Some(url.clone());
                match &request.want {
                    Some(Want::Range(req)) => {
                        detail.version = Some(
                            request
                                .version_text
                                .clone()
                                .unwrap_or_else(|| req.to_string()),
                        )
                    }
                    Some(Want::Tag(tag)) => detail.tag = Some(tag.clone()),
                    Some(Want::Rev(rev)) => detail.rev = Some(rev.clone()),
                    Some(Want::Branch(branch)) => detail.branch = Some(branch.clone()),
                    None => detail.version = Some(self.newest_release(&name, url)?),
                }
            }
            DepSource::Path(path) => detail.path = Some(path.replace('\\', "/")),
        }

        let mut editor = ManifestEditor::parse(&text)?;
        editor.set_dependency(&name, &detail, request.dev);
        let new_text = editor.to_text();
        let manifest = Manifest::parse(&new_text)?;
        let unlock = Unlock::Names(BTreeSet::from([name.clone()]));
        let report = self.sync(&manifest, unlock, &[])?;
        std::fs::write(self.manifest_path(), new_text)?;
        Ok((name, report))
    }

    /// `^X.Y.Z` for the newest `vX.Y.Z` tag of `url` (pre-releases left out).
    fn newest_release(&self, name: &str, url: &str) -> Result<String, PackageError> {
        if self.offline {
            return Err(PackageError::Offline(format!(
                "adding '{name}' needs the network to read its tags; run without --offline, or give \
                 --rev, --tag or --branch and have it in the cache"
            )));
        }
        let refs = git::ls_remote(url)?;
        let newest = refs
            .tags
            .iter()
            .filter_map(|(tag, _)| {
                semver::Version::parse(tag.strip_prefix('v').unwrap_or(tag)).ok()
            })
            .filter(|v| v.pre.is_empty())
            .max();
        match newest {
            Some(version) => Ok(format!("^{version}")),
            None => Err(PackageError::Git(format!(
                "{url} has no release tags (vX.Y.Z), so there is no version of '{name}' to add. Ask the \
                 author to push a tag such as v0.1.0, or depend on a commit or a branch: \
                 `plm add {url} --rev <sha>` or `--branch main`"
            ))),
        }
    }

    /// Removes a dependency from `pleno.toml` and from `packages/`.
    pub fn remove(&self, name: &str) -> Result<Report, PackageError> {
        let text = std::fs::read_to_string(self.manifest_path())
            .map_err(|_| PackageError::NotFound(self.manifest_path()))?;
        let mut editor = ManifestEditor::parse(&text)?;
        if !editor.remove_dependency(name) {
            return Err(PackageError::InvalidManifest(format!(
                "'{name}' is not in [dependencies] or [dev-dependencies] of pleno.toml"
            )));
        }
        let new_text = editor.to_text();
        let manifest = Manifest::parse(&new_text)?;
        let report = self.sync(&manifest, Unlock::None, &[])?;
        std::fs::write(self.manifest_path(), new_text)?;
        Ok(report)
    }

    /// Resolves, installs into `packages/` and writes `pleno.lock`.
    pub fn sync(
        &self,
        manifest: &Manifest,
        unlock: Unlock,
        features: &[String],
    ) -> Result<Report, PackageError> {
        let old = Lockfile::load_from_file(self.lock_path())?;
        let resolver = Resolver::new(&self.root, &self.cache, &old, unlock, self.offline);
        let packages = resolver.resolve(manifest, features)?;
        self.materialize(&packages, &old)?;

        let lockfile = Lockfile {
            version: LOCKFILE_VERSION,
            packages: packages.iter().map(ResolvedPackage::lock_entry).collect(),
        };
        if lockfile != old || !self.lock_path().exists() {
            lockfile.save_to_file(self.lock_path())?;
        }
        Ok(Report {
            changes: diff(&old, &lockfile),
            lockfile,
        })
    }

    /// Makes `packages/` hold exactly the resolved packages.
    fn materialize(
        &self,
        packages: &[ResolvedPackage],
        old: &Lockfile,
    ) -> Result<(), PackageError> {
        let dir = self.packages_dir();
        if !packages.is_empty() {
            std::fs::create_dir_all(&dir)?;
        }
        for package in packages {
            let dest = dir.join(&package.name);
            match &package.source {
                ResolvedSource::Git { .. } => {
                    if dest.is_dir()
                        && package.checksum.as_deref() == checksum_dir(&dest).ok().as_deref()
                    {
                        continue;
                    }
                    remove_path(&dest)?;
                    cache::copy_dir(&package.dir, &dest, &[])?;
                }
                ResolvedSource::Path { .. } => {
                    remove_path(&dest)?;
                    cache::copy_dir(&package.dir, &dest, &[".git", "packages", "target"])?;
                }
            }
        }
        // Packages plm installed before that are no longer needed.
        for old_package in &old.packages {
            if packages.iter().all(|p| p.name != old_package.name)
                && manifest::check_name(&old_package.name).is_ok()
            {
                remove_path(&dir.join(&old_package.name))?;
            }
        }
        Ok(())
    }

    /// The dependency tree from `pleno.toml` and `pleno.lock`, without
    /// network access.
    pub fn tree(&self) -> Result<String, PackageError> {
        let manifest = self.load_manifest()?;
        let lock = Lockfile::load_from_file(self.lock_path())?;
        let mut out = format!("{} v{}\n", manifest.package.name, manifest.package.version);
        let roots: Vec<(&String, bool)> = manifest
            .dependencies
            .keys()
            .map(|n| (n, false))
            .chain(manifest.dev_dependencies.keys().map(|n| (n, true)))
            .collect();
        let mut seen = BTreeSet::new();
        for (i, (name, dev)) in roots.iter().enumerate() {
            let last = i + 1 == roots.len();
            render_tree(
                &mut out,
                name,
                "",
                last,
                *dev,
                &lock,
                &mut seen,
                &mut Vec::new(),
            );
        }
        Ok(out)
    }

    pub fn run_script(&self, manifest: &Manifest, script_name: &str) -> Result<(), PackageError> {
        if let Some(script) = manifest.scripts.get(script_name) {
            println!("> {}", script);
            let status = if cfg!(windows) {
                std::process::Command::new("cmd")
                    .arg("/C")
                    .arg(script)
                    .status()?
            } else {
                std::process::Command::new("sh")
                    .arg("-c")
                    .arg(script)
                    .status()?
            };

            if !status.success() {
                return Err(PackageError::Io(std::io::Error::other(format!(
                    "Script '{}' exited with status {}",
                    script_name, status
                ))));
            }
            Ok(())
        } else {
            Err(PackageError::Io(std::io::Error::other(format!(
                "Script '{}' not found",
                script_name
            ))))
        }
    }

    pub fn build_project(
        &self,
        manifest: &Manifest,
        entry_path: Option<&str>,
    ) -> Result<(), PackageError> {
        use pleno_compiler::Compiler;
        use pleno_lexer::lex;
        use pleno_parser::Parser;

        let main_path = entry_path
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(&manifest.package.main));

        if !main_path.exists() {
            return Err(PackageError::NotFound(main_path));
        }

        let source = std::fs::read_to_string(&main_path)?;
        let tokens = lex(&source).map_err(|e| PackageError::LexError(format!("{:?}", e)))?;
        let mut parser = Parser::new(tokens, &source);
        let mut program = parser
            .parse_program()
            .map_err(|e| PackageError::ParseError(format!("{:?}", e)))?;

        let mut compiler = Compiler::new(source);
        let chunk = compiler
            .compile(&mut program)
            .map_err(|e| PackageError::CompileError(format!("{:?}", e)))?;

        let mut modules = HashMap::new();
        modules.insert(manifest.package.name.clone(), chunk);

        let bundle = Bundle {
            name: manifest.package.name.clone(),
            version: manifest.package.version.clone(),
            main_module: manifest.package.name.clone(),
            modules,
        };

        let bundle_path = PathBuf::from(format!("{}.plb", manifest.package.name));
        let encoded: Vec<u8> = bincode::serialize(&bundle)
            .map_err(|e| std::io::Error::other(format!("Serialization failed: {}", e)))?;

        std::fs::write(bundle_path, encoded)?;

        Ok(())
    }
}

fn remove_path(path: &Path) -> Result<(), PackageError> {
    match std::fs::symlink_metadata(path) {
        Ok(meta) if meta.is_dir() => std::fs::remove_dir_all(path)?,
        Ok(_) => std::fs::remove_file(path)?,
        Err(_) => {}
    }
    Ok(())
}

fn diff(old: &Lockfile, new: &Lockfile) -> Vec<Change> {
    let mut changes = Vec::new();
    let usable_old = old.version == LOCKFILE_VERSION;
    for package in &new.packages {
        let before = if usable_old {
            old.get(&package.name)
        } else {
            None
        };
        match before {
            None => changes.push(Change::Added {
                name: package.name.clone(),
                to: package.describe(),
            }),
            Some(before)
                if before.commit != package.commit
                    || before.source != package.source
                    || before.describe() != package.describe() =>
            {
                changes.push(Change::Updated {
                    name: package.name.clone(),
                    from: before.describe(),
                    to: package.describe(),
                })
            }
            Some(_) => {}
        }
    }
    if usable_old {
        for package in &old.packages {
            if new.get(&package.name).is_none() {
                changes.push(Change::Removed {
                    name: package.name.clone(),
                    from: package.describe(),
                });
            }
        }
    }
    changes
}

#[allow(clippy::too_many_arguments)]
fn render_tree(
    out: &mut String,
    name: &str,
    prefix: &str,
    last: bool,
    dev: bool,
    lock: &Lockfile,
    seen: &mut BTreeSet<String>,
    stack: &mut Vec<String>,
) {
    let branch = if last { "└── " } else { "├── " };
    let dev_note = if dev { " [dev]" } else { "" };
    let Some(entry) = lock.get(name) else {
        out.push_str(&format!(
            "{prefix}{branch}{name}{dev_note} (not installed: run `plm install`)\n"
        ));
        return;
    };
    let source = entry
        .git_url()
        .map(|u| format!(" ({u})"))
        .unwrap_or_default();
    let repeated = !entry.dependencies.is_empty() && seen.contains(name);
    let cycle = stack.iter().any(|s| s == name);
    out.push_str(&format!(
        "{prefix}{branch}{name} {}{source}{dev_note}{}\n",
        entry.describe(),
        if cycle {
            " (cycle)"
        } else if repeated {
            " (*)"
        } else {
            ""
        }
    ));
    if repeated || cycle {
        return;
    }
    seen.insert(name.to_string());
    stack.push(name.to_string());
    let child_prefix = format!("{prefix}{}", if last { "    " } else { "│   " });
    for (i, child) in entry.dependencies.iter().enumerate() {
        let child_last = i + 1 == entry.dependencies.len();
        render_tree(
            out,
            child,
            &child_prefix,
            child_last,
            false,
            lock,
            seen,
            stack,
        );
    }
    stack.pop();
}
