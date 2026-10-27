//! Dependency resolution.
//!
//! Every package gets one version in a project (`packages/` is flat, so
//! `import ... from "name"` means the same package everywhere). For each
//! package plm takes the requirements of everything that depends on it and
//! picks the **highest tag that satisfies all of them**. Pins (`tag`, `rev`,
//! `branch`, `path`) in your own `pleno.toml` win over what other packages
//! ask for; that is how you settle a conflict. plm does not backtrack: it never
//! picks an older version of one package to make room for another; when no
//! version fits, it says who asked for what.
//!
//! With a lockfile, a locked commit is kept as long as it still satisfies the
//! requirements, so `plm install` changes nothing and needs no network; only
//! `plm update` (or a changed requirement) chooses again.

use crate::PackageError;
use crate::cache::{Cache, checksum_dir};
use crate::git::{self, RemoteRefs};
use crate::lockfile::{LOCKFILE_VERSION, LockedPackage, Lockfile};
use crate::manifest::{DepSource, Dependency, Manifest, Want, check_name, normalize_git_url};
use semver::{Version, VersionReq};
use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet, HashMap, VecDeque};
use std::path::{Component, Path, PathBuf};

/// Which locked packages may be chosen again.
#[derive(Debug, Clone)]
pub enum Unlock {
    /// None: keep every locked commit that still fits (`plm install`).
    None,
    /// All of them (`plm update`).
    All,
    /// Only these (`plm update a b`, `plm add a`).
    Names(BTreeSet<String>),
}

impl Unlock {
    fn allows_lock(&self, name: &str) -> bool {
        match self {
            Unlock::None => true,
            Unlock::All => false,
            Unlock::Names(names) => !names.contains(name),
        }
    }
}

/// A package chosen by the resolver, with its files ready on disk.
#[derive(Debug, Clone)]
pub struct ResolvedPackage {
    pub name: String,
    pub source: ResolvedSource,
    pub version: Option<Version>,
    pub tag: Option<String>,
    pub branch: Option<String>,
    pub commit: Option<String>,
    pub checksum: Option<String>,
    /// Where its files are: a cache folder, or the path dependency itself.
    pub dir: PathBuf,
    pub manifest: Option<Manifest>,
    pub dependencies: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolvedSource {
    Git {
        url: String,
    },
    /// `written` is relative to the project root, `dir` absolute.
    Path {
        written: String,
        dir: PathBuf,
    },
}

impl ResolvedPackage {
    pub fn lock_entry(&self) -> LockedPackage {
        LockedPackage {
            name: self.name.clone(),
            source: match &self.source {
                ResolvedSource::Git { url } => format!("git+{url}"),
                ResolvedSource::Path { written, .. } => format!("path+{written}"),
            },
            version: self.version.as_ref().map(Version::to_string),
            tag: self.tag.clone(),
            branch: self.branch.clone(),
            commit: self.commit.clone(),
            checksum: self.checksum.clone(),
            dependencies: self.dependencies.clone(),
        }
    }

    fn label(&self) -> String {
        format!("{} {}", self.name, self.lock_entry().describe())
    }
}

#[derive(Debug, Clone)]
struct Requirement {
    /// Who asks: `pleno.toml`, or `greet v1.2.0`.
    by: String,
    from_root: bool,
    source: ReqSource,
    want: Option<Want>,
}

#[derive(Debug, Clone)]
enum ReqSource {
    Git { url: String, key: String },
    Path { written: String, dir: PathBuf },
}

impl ReqSource {
    fn same_as(&self, other: &ReqSource) -> bool {
        match (self, other) {
            (ReqSource::Git { key: a, .. }, ReqSource::Git { key: b, .. }) => a == b,
            (ReqSource::Path { dir: a, .. }, ReqSource::Path { dir: b, .. }) => a == b,
            _ => false,
        }
    }

    fn describe(&self) -> String {
        match self {
            ReqSource::Git { url, .. } => url.clone(),
            ReqSource::Path { written, .. } => format!("path {written}"),
        }
    }
}

/// What a package must be, once every requirement on it is combined.
#[derive(Debug, Clone)]
enum Decision {
    Git {
        url: String,
        key: String,
        want: GitWant,
    },
    Path {
        written: String,
        dir: PathBuf,
    },
}

#[derive(Debug, Clone)]
enum GitWant {
    Ranges(Vec<(VersionReq, String)>),
    Tag(String),
    Rev(String),
    Branch(String),
}

impl GitWant {
    fn describe(&self) -> String {
        match self {
            GitWant::Ranges(ranges) => ranges
                .iter()
                .map(|(req, by)| format!("{req} (from {by})"))
                .collect::<Vec<_>>()
                .join(" and "),
            GitWant::Tag(tag) => format!("tag {tag}"),
            GitWant::Rev(rev) => format!("rev {rev}"),
            GitWant::Branch(branch) => format!("branch {branch}"),
        }
    }

    /// Whether a package with these fields is what this asks for.
    fn accepts(
        &self,
        version: Option<&Version>,
        tag: Option<&str>,
        branch: Option<&str>,
        commit: Option<&str>,
    ) -> bool {
        match self {
            GitWant::Ranges(ranges) => {
                tag.is_some()
                    && version.is_some_and(|v| ranges.iter().all(|(req, _)| req.matches(v)))
            }
            GitWant::Tag(t) => tag == Some(t.as_str()),
            GitWant::Rev(rev) => commit.is_some_and(|c| c.starts_with(rev.as_str())),
            GitWant::Branch(b) => branch == Some(b.as_str()),
        }
    }
}

fn tag_version(tag: &str) -> Option<Version> {
    Version::parse(tag.strip_prefix('v').unwrap_or(tag)).ok()
}

/// Joins `relative` to `base` and removes `.` and `..` without touching the
/// file system.
fn lexical_join(base: &Path, relative: &str) -> PathBuf {
    let mut out = PathBuf::new();
    for component in base.join(relative).components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                if !out.pop() {
                    out.push("..");
                }
            }
            other => out.push(other.as_os_str()),
        }
    }
    out
}

fn written_join(base: &str, relative: &str) -> String {
    if base.is_empty() || Path::new(relative).is_absolute() {
        return relative.replace('\\', "/");
    }
    lexical_join(Path::new(base), relative)
        .to_string_lossy()
        .replace('\\', "/")
}

pub struct Resolver<'a> {
    pub root_dir: &'a Path,
    pub cache: &'a Cache,
    pub lock: &'a Lockfile,
    pub unlock: Unlock,
    pub offline: bool,
    refs: RefCell<HashMap<String, RemoteRefs>>,
}

const MAX_ROUNDS: usize = 64;

impl<'a> Resolver<'a> {
    pub fn new(
        root_dir: &'a Path,
        cache: &'a Cache,
        lock: &'a Lockfile,
        unlock: Unlock,
        offline: bool,
    ) -> Self {
        Self {
            root_dir,
            cache,
            lock,
            unlock,
            offline,
            refs: RefCell::new(HashMap::new()),
        }
    }

    fn locked(&self, name: &str) -> Option<&LockedPackage> {
        if self.lock.version != LOCKFILE_VERSION || !self.unlock.allows_lock(name) {
            return None;
        }
        self.lock.get(name)
    }

    /// Resolves `root` and its dependencies (with dev-dependencies, which are
    /// only installed for the project itself) and returns them sorted by name.
    pub fn resolve(
        &self,
        root: &Manifest,
        features: &[String],
    ) -> Result<Vec<ResolvedPackage>, PackageError> {
        root.validate()?;
        let mut chosen: BTreeMap<String, ResolvedPackage> = BTreeMap::new();
        for _ in 0..MAX_ROUNDS {
            let (requirements, edges) = self.collect(root, features, &chosen)?;
            let mut next = BTreeMap::new();
            let mut changed = requirements.len() != chosen.len();
            for (name, reqs) in &requirements {
                let decision = decide(name, reqs)?;
                if let Some(current) = chosen.get(name)
                    && accepts(&decision, current)
                {
                    next.insert(name.clone(), current.clone());
                    continue;
                }
                changed = true;
                next.insert(name.clone(), self.pick(name, &decision)?);
            }
            chosen = next;
            if !changed {
                let mut out: Vec<ResolvedPackage> = chosen.into_values().collect();
                for package in &mut out {
                    package.dependencies = edges.get(&package.name).cloned().unwrap_or_default();
                }
                return Ok(out);
            }
        }
        Err(PackageError::Conflict(
            "dependency resolution did not settle: the packages' requirements keep changing each other. \
             Pin the packages involved with an exact version in your pleno.toml"
                .to_string(),
        ))
    }

    /// Walks the graph from `root` through the packages chosen so far and
    /// gathers every requirement on each package, and who depends on whom.
    #[allow(clippy::type_complexity)]
    fn collect(
        &self,
        root: &Manifest,
        features: &[String],
        chosen: &BTreeMap<String, ResolvedPackage>,
    ) -> Result<
        (
            BTreeMap<String, Vec<Requirement>>,
            BTreeMap<String, Vec<String>>,
        ),
        PackageError,
    > {
        // Features requested of each package by anything in the graph.
        let mut requested: HashMap<String, BTreeSet<String>> = HashMap::new();
        let all_manifests =
            std::iter::once(root).chain(chosen.values().filter_map(|p| p.manifest.as_ref()));
        for manifest in all_manifests {
            for (name, dep) in manifest
                .dependencies
                .iter()
                .chain(&manifest.dev_dependencies)
            {
                requested
                    .entry(name.clone())
                    .or_default()
                    .extend(dep.requested_features().iter().cloned());
            }
        }

        let mut requirements: BTreeMap<String, Vec<Requirement>> = BTreeMap::new();
        let mut edges: BTreeMap<String, Vec<String>> = BTreeMap::new();
        let mut queue = VecDeque::new();

        let root_features: BTreeSet<String> = features.iter().cloned().collect();
        let root_deps = root
            .dependencies
            .iter()
            .chain(&root.dev_dependencies)
            .filter(|(name, dep)| enabled(root, name, dep, &root_features));
        for (name, dep) in root_deps {
            let (source, want) = dep.spec(name)?;
            let source = match source {
                DepSource::Git(url) => ReqSource::Git {
                    key: normalize_git_url(&url),
                    url,
                },
                DepSource::Path(written) => ReqSource::Path {
                    dir: lexical_join(self.root_dir, &written),
                    written: written.replace('\\', "/"),
                },
            };
            requirements
                .entry(name.clone())
                .or_default()
                .push(Requirement {
                    by: "pleno.toml".to_string(),
                    from_root: true,
                    source,
                    want,
                });
            queue.push_back(name.clone());
        }

        let mut visited = BTreeSet::new();
        while let Some(name) = queue.pop_front() {
            if !visited.insert(name.clone()) {
                continue;
            }
            let Some(package) = chosen.get(&name) else {
                continue;
            };
            let Some(manifest) = &package.manifest else {
                edges.insert(name.clone(), Vec::new());
                continue;
            };
            let active: BTreeSet<String> = requested.get(&name).cloned().unwrap_or_default();
            let mut deps = Vec::new();
            for (dep_name, dep) in &manifest.dependencies {
                if !enabled(manifest, dep_name, dep, &active) {
                    continue;
                }
                if *dep_name == root.package.name && dep_name != &name {
                    // A dependency that depends on the project itself.
                    continue;
                }
                check_name(dep_name)?;
                let (source, want) = dep.spec(dep_name).map_err(|e| {
                    PackageError::InvalidManifest(format!("in {}: {e}", package.label()))
                })?;
                let source = match (source, &package.source) {
                    (DepSource::Git(url), _) => ReqSource::Git {
                        key: normalize_git_url(&url),
                        url,
                    },
                    (DepSource::Path(written), ResolvedSource::Path { written: base, dir }) => {
                        ReqSource::Path {
                            dir: lexical_join(dir, &written),
                            written: written_join(base, &written),
                        }
                    }
                    (DepSource::Path(written), ResolvedSource::Git { url }) => {
                        return Err(PackageError::InvalidManifest(format!(
                            "{} (from {url}) depends on '{dep_name}' by path ({written}). A package fetched \
                             from Git can only depend on other Git repositories; ask its author to use \
                             {{ git = ... }}",
                            package.label()
                        )));
                    }
                };
                requirements
                    .entry(dep_name.clone())
                    .or_default()
                    .push(Requirement {
                        by: package.label(),
                        from_root: false,
                        source,
                        want,
                    });
                deps.push(dep_name.clone());
                queue.push_back(dep_name.clone());
            }
            edges.insert(name.clone(), deps);
        }
        Ok((requirements, edges))
    }

    fn remote_refs(&self, url: &str) -> Result<RemoteRefs, PackageError> {
        let key = normalize_git_url(url);
        if let Some(refs) = self.refs.borrow().get(&key) {
            return Ok(refs.clone());
        }
        let refs = git::ls_remote(url)?;
        self.refs.borrow_mut().insert(key, refs.clone());
        Ok(refs)
    }

    fn pick(&self, name: &str, decision: &Decision) -> Result<ResolvedPackage, PackageError> {
        match decision {
            Decision::Path { written, dir } => {
                if !dir.is_dir() {
                    return Err(PackageError::InvalidManifest(format!(
                        "dependency '{name}' is the folder {written}, which does not exist ({})",
                        dir.display()
                    )));
                }
                let manifest_path = dir.join("pleno.toml");
                let manifest = if manifest_path.is_file() {
                    Some(Manifest::load_from_file(&manifest_path)?)
                } else {
                    None
                };
                Ok(ResolvedPackage {
                    name: name.to_string(),
                    source: ResolvedSource::Path {
                        written: written.clone(),
                        dir: dir.clone(),
                    },
                    version: None,
                    tag: None,
                    branch: None,
                    commit: None,
                    checksum: None,
                    dir: dir.clone(),
                    manifest,
                    dependencies: Vec::new(),
                })
            }
            Decision::Git { url, key, want } => self.pick_git(name, url, key, want),
        }
    }

    fn pick_git(
        &self,
        name: &str,
        url: &str,
        key: &str,
        want: &GitWant,
    ) -> Result<ResolvedPackage, PackageError> {
        let locked = self
            .locked(name)
            .filter(|l| l.git_url().is_some_and(|u| normalize_git_url(u) == key));
        // 1. The locked commit, if it still fits.
        if let Some(lock) = locked {
            let version = lock.version.as_deref().and_then(|v| Version::parse(v).ok());
            if let Some(commit) = &lock.commit
                && want.accepts(
                    version.as_ref(),
                    lock.tag.as_deref(),
                    lock.branch.as_deref(),
                    Some(commit),
                )
            {
                let hint = lock
                    .tag
                    .as_ref()
                    .map(|t| format!("refs/tags/{t}"))
                    .or_else(|| lock.branch.as_ref().map(|b| format!("refs/heads/{b}")));
                let dir = self
                    .cache
                    .ensure(name, url, commit, hint.as_deref(), self.offline)?;
                let checksum = checksum_dir(&dir)?;
                if let Some(expected) = &lock.checksum
                    && *expected != checksum
                {
                    return Err(PackageError::ChecksumMismatch {
                        name: name.to_string(),
                        commit: commit.clone(),
                        expected: expected.clone(),
                        actual: checksum,
                        cache: dir,
                    });
                }
                return self.finish(
                    name,
                    url,
                    version,
                    lock.tag.clone(),
                    lock.branch.clone(),
                    commit.clone(),
                    checksum,
                    dir,
                );
            }
        }

        // 2. Choose from the repository's tags and branches.
        if self.offline {
            let why = match self.lock.get(name) {
                None => "it is not in pleno.lock".to_string(),
                Some(lock) => format!(
                    "pleno.lock has {}, but {} is wanted",
                    lock.describe(),
                    want.describe()
                ),
            };
            return Err(PackageError::Offline(format!(
                "'{name}' needs the network: {why}. Run the command without --offline"
            )));
        }
        let (commit, version, tag, branch) = match want {
            GitWant::Ranges(ranges) => {
                let refs = self.remote_refs(url)?;
                let mut versions: Vec<(Version, &str, &str)> = refs
                    .tags
                    .iter()
                    .filter_map(|(tag, commit)| {
                        tag_version(tag).map(|v| (v, tag.as_str(), commit.as_str()))
                    })
                    .collect();
                // Highest version first; for the same version, `v1.0.0` over `1.0.0`.
                versions.sort_by(|a, b| {
                    b.0.cmp(&a.0)
                        .then(b.1.starts_with('v').cmp(&a.1.starts_with('v')))
                });
                let best = versions
                    .iter()
                    .find(|(v, _, _)| ranges.iter().all(|(req, _)| req.matches(v)));
                let Some((version, tag, commit)) = best else {
                    let mut available: Vec<Version> =
                        versions.iter().map(|(v, _, _)| v.clone()).collect();
                    available.sort();
                    available.dedup();
                    return Err(PackageError::NoMatchingVersion {
                        name: name.to_string(),
                        url: url.to_string(),
                        requirement: want.describe(),
                        available: available.iter().map(|v| format!("v{v}")).collect(),
                    });
                };
                (
                    commit.to_string(),
                    Some(version.clone()),
                    Some(tag.to_string()),
                    None,
                )
            }
            GitWant::Tag(tag) => {
                let refs = self.remote_refs(url)?;
                let Some(commit) = refs.tag(tag) else {
                    let mut tags: Vec<&str> = refs.tags.iter().map(|(t, _)| t.as_str()).collect();
                    tags.sort();
                    return Err(PackageError::Git(format!(
                        "{url} has no tag {tag} (needed by '{name}'). Its tags: {}",
                        if tags.is_empty() {
                            "none".to_string()
                        } else {
                            tags.join(", ")
                        }
                    )));
                };
                (
                    commit.to_string(),
                    tag_version(tag),
                    Some(tag.clone()),
                    None,
                )
            }
            GitWant::Branch(branch) => {
                let refs = self.remote_refs(url)?;
                let Some(commit) = refs.head(branch) else {
                    return Err(PackageError::Git(format!(
                        "{url} has no branch {branch} (needed by '{name}')"
                    )));
                };
                (commit.to_string(), None, None, Some(branch.clone()))
            }
            GitWant::Rev(rev) => {
                let commit = if rev.len() == 40 {
                    rev.clone()
                } else {
                    git::expand_rev(url, rev, &self.cache.scratch_dir(url))?
                };
                (commit, None, None, None)
            }
        };

        // A tag that moved since the lockfile was written is refused.
        if let (Some(tag), Some(lock)) = (&tag, self.lock.get(name))
            && lock.git_url().is_some_and(|u| normalize_git_url(u) == key)
            && lock.tag.as_ref() == Some(tag)
            && let Some(locked_commit) = &lock.commit
            && *locked_commit != commit
        {
            return Err(PackageError::TagMoved {
                name: name.to_string(),
                url: url.to_string(),
                tag: tag.clone(),
                locked: locked_commit.clone(),
                now: commit,
            });
        }

        let hint = tag
            .as_ref()
            .map(|t| format!("refs/tags/{t}"))
            .or_else(|| branch.as_ref().map(|b| format!("refs/heads/{b}")));
        let dir = self
            .cache
            .ensure(name, url, &commit, hint.as_deref(), false)?;
        let checksum = checksum_dir(&dir)?;
        // The same commit as in the lockfile must have the same files.
        if let Some(lock) = self.lock.get(name)
            && lock.commit.as_deref() == Some(commit.as_str())
            && let Some(expected) = &lock.checksum
            && *expected != checksum
        {
            return Err(PackageError::ChecksumMismatch {
                name: name.to_string(),
                commit,
                expected: expected.clone(),
                actual: checksum,
                cache: dir,
            });
        }
        self.finish(name, url, version, tag, branch, commit, checksum, dir)
    }

    #[allow(clippy::too_many_arguments)]
    fn finish(
        &self,
        name: &str,
        url: &str,
        version: Option<Version>,
        tag: Option<String>,
        branch: Option<String>,
        commit: String,
        checksum: String,
        dir: PathBuf,
    ) -> Result<ResolvedPackage, PackageError> {
        let manifest_path = dir.join("pleno.toml");
        let manifest = if manifest_path.is_file() {
            Some(Manifest::load_from_file(&manifest_path).map_err(|e| {
                PackageError::InvalidManifest(format!(
                    "the pleno.toml of '{name}' at commit {commit} of {url} is invalid: {e}"
                ))
            })?)
        } else {
            None
        };
        Ok(ResolvedPackage {
            name: name.to_string(),
            source: ResolvedSource::Git {
                url: url.to_string(),
            },
            version,
            tag,
            branch,
            commit: Some(commit),
            checksum: Some(checksum),
            dir,
            manifest,
            dependencies: Vec::new(),
        })
    }
}

/// Whether a dependency is used: not optional, or optional and turned on by
/// an active feature (or the `default` feature).
fn enabled(manifest: &Manifest, name: &str, dep: &Dependency, active: &BTreeSet<String>) -> bool {
    if !dep.is_optional() {
        return true;
    }
    manifest.features.iter().any(|(feature, deps)| {
        (feature == "default" || active.contains(feature)) && deps.iter().any(|d| d == name)
    })
}

fn accepts(decision: &Decision, package: &ResolvedPackage) -> bool {
    match (decision, &package.source) {
        (Decision::Path { dir, .. }, ResolvedSource::Path { dir: current, .. }) => dir == current,
        (Decision::Git { key, want, .. }, ResolvedSource::Git { url }) => {
            normalize_git_url(url) == *key
                && want.accepts(
                    package.version.as_ref(),
                    package.tag.as_deref(),
                    package.branch.as_deref(),
                    package.commit.as_deref(),
                )
        }
        _ => false,
    }
}

/// Combines the requirements on one package into what it must be.
fn decide(name: &str, reqs: &[Requirement]) -> Result<Decision, PackageError> {
    let list = |reqs: &[Requirement]| {
        reqs.iter()
            .map(|r| {
                let want = r
                    .want
                    .as_ref()
                    .map(|w| format!(", {}", w.describe()))
                    .unwrap_or_default();
                format!("  - {} wants {}{want}", r.by, r.source.describe())
            })
            .collect::<Vec<_>>()
            .join("\n")
    };

    // Your own pleno.toml decides when it pins the package.
    let root = reqs.iter().find(|r| r.from_root);
    let root_pins = root.is_some_and(|r| {
        matches!(r.source, ReqSource::Path { .. }) || !matches!(r.want, Some(Want::Range(_)) | None)
    });
    let considered: Vec<Requirement> = if root_pins {
        vec![root.unwrap().clone()]
    } else {
        reqs.to_vec()
    };

    let first = &considered[0];
    if considered.iter().any(|r| !r.source.same_as(&first.source)) {
        return Err(PackageError::Conflict(format!(
            "'{name}' is required from different places:\n{}\nA project can have only one package named \
             '{name}'. Add '{name}' to your pleno.toml with the source you want; your own pleno.toml wins.",
            list(reqs)
        )));
    }

    match &first.source {
        ReqSource::Path { written, dir } => Ok(Decision::Path {
            written: written.clone(),
            dir: dir.clone(),
        }),
        ReqSource::Git { url, key } => {
            let mut pin: Option<(&Want, &str)> = None;
            let mut ranges = Vec::new();
            for req in &considered {
                match &req.want {
                    None => ranges.push((VersionReq::STAR, req.by.clone())),
                    Some(Want::Range(range)) => ranges.push((range.clone(), req.by.clone())),
                    Some(other) => match pin {
                        None => pin = Some((other, &req.by)),
                        Some((existing, by)) if existing != other => {
                            return Err(PackageError::Conflict(format!(
                                "'{name}' is pinned two ways ({} by {by}, {} by {}):
{}
Pin '{name}' in your                                  pleno.toml to choose; your own pleno.toml wins.",
                                existing.describe(),
                                other.describe(),
                                req.by,
                                list(reqs)
                            )));
                        }
                        Some(_) => {}
                    },
                }
            }
            let want = match pin {
                None => GitWant::Ranges(ranges),
                Some((Want::Tag(tag), _)) => {
                    if let Some(version) = tag_version(tag) {
                        if let Some((req, by)) =
                            ranges.iter().find(|(req, _)| !req.matches(&version))
                        {
                            return Err(PackageError::Conflict(format!(
                                "'{name}' is pinned to tag {tag}, but {by} needs {req}:\n{}\nChange one of \
                                 them, or pin '{name}' in your pleno.toml; your own pleno.toml wins.",
                                list(reqs)
                            )));
                        }
                    } else if !ranges.is_empty() {
                        return Err(PackageError::Conflict(format!(
                            "'{name}' is pinned to tag {tag}, which is not a version, and also required by \
                             version:\n{}\nPin '{name}' in your pleno.toml to choose.",
                            list(reqs)
                        )));
                    }
                    GitWant::Tag(tag.clone())
                }
                Some((other, by)) => {
                    if !ranges.is_empty() {
                        return Err(PackageError::Conflict(format!(
                            "'{name}' is pinned to {} by {by}, and required by version as well:\n{}\nPin '{name}' \
                             in your pleno.toml to choose; your own pleno.toml wins.",
                            other.describe(),
                            list(reqs)
                        )));
                    }
                    match other {
                        Want::Rev(rev) => GitWant::Rev(rev.clone()),
                        Want::Branch(branch) => GitWant::Branch(branch.clone()),
                        Want::Tag(_) | Want::Range(_) => unreachable!(),
                    }
                }
            };
            Ok(Decision::Git {
                url: url.clone(),
                key: key.clone(),
                want,
            })
        }
    }
}
