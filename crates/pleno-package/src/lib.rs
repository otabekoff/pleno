use semver::{Version, VersionReq};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum PackageError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("TOML Parse error: {0}")]
    TomlParse(#[from] toml::de::Error),
    #[error("TOML Serialize error: {0}")]
    TomlSerialize(#[from] toml::ser::Error),
    #[error("Package manifest not found at {0}")]
    NotFound(PathBuf),
    #[error("Missing 'name' field in manifest")]
    MissingName,
    #[error("Missing 'version' field in manifest")]
    MissingVersion,
    #[error("Invalid version requirement: {0}")]
    InvalidVersionReq(String),
    #[error("No compatible version found for {0} (requires {1})")]
    NoCompatibleVersion(String, String),
    #[error("Compile error: {0}")]
    CompileError(String),
    #[error("Parse error: {0}")]
    ParseError(String),
    #[error("Lex error: {0}")]
    LexError(String),
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Bundle {
    pub name: String,
    pub version: String,
    pub main_module: String,
    pub modules: HashMap<String, pleno_bytecode::Chunk>,
}

pub trait Registry {
    fn query(&self, name: &str, req: &VersionReq) -> Result<Vec<Version>, PackageError>;
    fn fetch_source(
        &self,
        name: &str,
        version: &Version,
    ) -> Result<(String, Option<String>), PackageError>;
    fn publish(&self, package: PublishPackage, token: &str) -> Result<(), PackageError>;
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PublishPackage {
    pub name: String,
    pub version: String,
    pub git_url: String,
    pub commit: String,
    pub sha256: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RegistryMetadata {
    pub name: String,
    pub versions: HashMap<String, VersionMetadata>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct VersionMetadata {
    pub url: String,
    pub sha256: Option<String>,
}

pub struct RemoteRegistry {
    pub base_url: String,
}

impl RemoteRegistry {
    pub fn new(base_url: String) -> Self {
        Self { base_url }
    }
}

impl Registry for RemoteRegistry {
    fn query(&self, name: &str, _req: &VersionReq) -> Result<Vec<Version>, PackageError> {
        let url = format!("{}/packages/{}", self.base_url, name);
        let resp =
            reqwest::blocking::get(url).map_err(|e| PackageError::Io(std::io::Error::other(e)))?;

        if !resp.status().is_success() {
            return Ok(vec![]);
        }

        let metadata: RegistryMetadata = resp
            .json()
            .map_err(|e| PackageError::Io(std::io::Error::other(e)))?;

        let mut versions = vec![];
        for v_str in metadata.versions.keys() {
            if let Ok(v) = Version::parse(v_str) {
                versions.push(v);
            }
        }
        Ok(versions)
    }

    fn fetch_source(
        &self,
        name: &str,
        version: &Version,
    ) -> Result<(String, Option<String>), PackageError> {
        let url = format!("{}/packages/{}", self.base_url, name);
        let resp =
            reqwest::blocking::get(url).map_err(|e| PackageError::Io(std::io::Error::other(e)))?;

        let metadata: RegistryMetadata = resp
            .json()
            .map_err(|e| PackageError::Io(std::io::Error::other(e)))?;

        let v_str = version.to_string();
        let v_meta = metadata
            .versions
            .get(&v_str)
            .ok_or_else(|| PackageError::NoCompatibleVersion(name.to_string(), v_str))?;

        Ok((v_meta.url.clone(), v_meta.sha256.clone()))
    }

    fn publish(&self, package: PublishPackage, token: &str) -> Result<(), PackageError> {
        let url = format!("{}/packages/{}", self.base_url, package.name);
        let client = reqwest::blocking::Client::new();
        let resp = client
            .post(url)
            .header("Authorization", format!("Bearer {}", token))
            .json(&package)
            .send()
            .map_err(|e| PackageError::Io(std::io::Error::other(e)))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let err_msg = resp.text().unwrap_or_default();
            return Err(PackageError::Io(std::io::Error::other(format!(
                "Publish failed: {} - {}",
                status, err_msg
            ))));
        }

        Ok(())
    }
}

pub struct Resolver<'a> {
    registry: &'a dyn Registry,
}

/// Result type for resolved dependencies: maps package name to (Version, source, optional SHA)
pub type ResolvedDeps = HashMap<String, (Version, String, Option<String>)>;

impl<'a> Resolver<'a> {
    pub fn new(registry: &'a dyn Registry) -> Self {
        Self { registry }
    }

    pub fn resolve(
        &self,
        manifest: &Manifest,
        lockfile: &Lockfile,
        features: &[String],
    ) -> Result<ResolvedDeps, PackageError> {
        let mut resolved = HashMap::new();
        let mut stack = Vec::new();
        let mut active_features = std::collections::HashSet::new();
        for f in features {
            active_features.insert(f.clone());
        }
        // Always include default features if defined
        if let Some(defaults) = manifest.features.get("default") {
            for f in defaults {
                active_features.insert(f.clone());
            }
        }

        self.resolve_recursive(
            manifest,
            &mut resolved,
            lockfile,
            &mut stack,
            &active_features,
        )?;
        Ok(resolved)
    }

    fn resolve_recursive(
        &self,
        manifest: &Manifest,
        resolved: &mut HashMap<String, (Version, String, Option<String>)>,
        lockfile: &Lockfile,
        stack: &mut Vec<String>,
        active_features: &std::collections::HashSet<String>,
    ) -> Result<(), PackageError> {
        if stack.contains(&manifest.package.name) {
            return Ok(());
        }
        stack.push(manifest.package.name.clone());

        for (name, dep) in &manifest.dependencies {
            if resolved.contains_key(name) {
                continue;
            }

            match dep {
                Dependency::Version(v_str) => {
                    let req = VersionReq::parse(v_str)
                        .map_err(|_| PackageError::InvalidVersionReq(v_str.clone()))?;

                    // Prefer lockfile if compatible
                    if let Some(lock_pkg) = lockfile.packages.iter().find(|p| p.name == *name) {
                        let lock_version =
                            Version::parse(&lock_pkg.version).unwrap_or(Version::new(0, 0, 0));
                        if req.matches(&lock_version) {
                            resolved.insert(
                                name.clone(),
                                (
                                    lock_version,
                                    lock_pkg.source.clone(),
                                    lock_pkg.checksum.clone(),
                                ),
                            );
                            // Recursively resolve lock package dependencies
                            // (Simplified for now: we'd need to fetch manifest of lock_version)
                            continue;
                        }
                    }

                    let versions = self.registry.query(name, &req)?;
                    let best_version = versions
                        .into_iter()
                        .filter(|v| req.matches(v))
                        .max()
                        .ok_or_else(|| {
                            PackageError::NoCompatibleVersion(name.clone(), v_str.clone())
                        })?;
                    let (source, sha256) = self.registry.fetch_source(name, &best_version)?;
                    resolved.insert(name.clone(), (best_version, source, sha256));
                }
                Dependency::Detailed(detail) => {
                    // Check if optional and enabled
                    if detail.optional.unwrap_or(false) {
                        let mut enabled = false;
                        for (f_name, f_deps) in &manifest.features {
                            if active_features.contains(f_name) && f_deps.contains(name) {
                                enabled = true;
                                break;
                            }
                        }
                        if !enabled {
                            continue;
                        }
                    }

                    let v_str = detail.version.as_deref().unwrap_or("*");
                    let req = VersionReq::parse(v_str)
                        .map_err(|_| PackageError::InvalidVersionReq(v_str.to_string()))?;

                    // Prefer lockfile
                    if let Some(lock_pkg) = lockfile.packages.iter().find(|p| p.name == *name) {
                        let lock_version =
                            Version::parse(&lock_pkg.version).unwrap_or(Version::new(0, 0, 0));
                        if req.matches(&lock_version) {
                            let sha = if lock_pkg.source.starts_with("git+") {
                                lock_pkg.commit.clone()
                            } else {
                                lock_pkg.checksum.clone()
                            };
                            resolved
                                .insert(name.clone(), (lock_version, lock_pkg.source.clone(), sha));
                            continue;
                        }
                    }

                    // Logic similar to Version variant but with detail source
                    let source = if let Some(git) = &detail.git {
                        format!("git+{}", git)
                    } else if let Some(path) = &detail.path {
                        path.clone()
                    } else {
                        // Fallback to registry
                        let versions = self.registry.query(name, &req)?;
                        let best = versions
                            .into_iter()
                            .filter(|v| req.matches(v))
                            .max()
                            .ok_or_else(|| {
                                PackageError::NoCompatibleVersion(name.clone(), v_str.to_string())
                            })?;
                        let (s, _) = self.registry.fetch_source(name, &best)?;
                        s
                    };
                    resolved.insert(name.clone(), (Version::new(0, 0, 0), source, None));
                }
            }
        }

        stack.pop();
        Ok(())
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Manifest {
    pub package: PackageSection,
    #[serde(default)]
    pub scripts: HashMap<String, String>,
    #[serde(default)]
    pub features: HashMap<String, Vec<String>>,
    #[serde(default)]
    pub dependencies: HashMap<String, Dependency>,
    #[serde(rename = "dev-dependencies", default)]
    pub dev_dependencies: HashMap<String, Dependency>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(untagged)]
pub enum Dependency {
    Version(String),
    Detailed(DependencyDetail),
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DependencyDetail {
    pub version: Option<String>,
    pub git: Option<String>,
    pub path: Option<String>,
    pub branch: Option<String>,
    pub tag: Option<String>,
    pub rev: Option<String>,
    pub optional: Option<bool>,
    pub features: Option<Vec<String>>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(untagged)]
pub enum DependencySource {
    Git {
        git: String,
        branch: Option<String>,
        tag: Option<String>,
        rev: Option<String>,
    },
    Path {
        path: String,
    },
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PackageSection {
    pub name: String,
    pub version: String,
    pub description: Option<String>,
    #[serde(default = "default_main")]
    pub main: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct Lockfile {
    #[serde(rename = "package")]
    pub packages: Vec<LockPackage>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct LockPackage {
    pub name: String,
    pub version: String,
    pub source: String,
    pub commit: Option<String>,
    pub checksum: Option<String>,
}

impl Lockfile {
    pub fn load_from_file<P: AsRef<Path>>(path: P) -> Result<Self, PackageError> {
        let path = path.as_ref();
        if !path.exists() {
            return Ok(Self::default());
        }
        let content = std::fs::read_to_string(path)?;
        let lock: Lockfile = toml::from_str(&content)?;
        Ok(lock)
    }

    pub fn save_to_file<P: AsRef<Path>>(&self, path: P) -> Result<(), PackageError> {
        let content = toml::to_string_pretty(self)?;
        std::fs::write(path, content)?;
        Ok(())
    }

    pub fn get_dependency_source(&self, name: &str) -> Option<&String> {
        self.packages
            .iter()
            .find(|p| p.name == name)
            .map(|p| &p.source)
    }
}

fn default_main() -> String {
    "src/main.pln".to_string()
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
            scripts: HashMap::new(),
            features: HashMap::new(),
            dependencies: HashMap::new(),
            dev_dependencies: HashMap::new(),
        }
    }

    pub fn load_from_file<P: AsRef<Path>>(path: P) -> Result<Self, PackageError> {
        let path = path.as_ref();
        if !path.exists() {
            return Err(PackageError::NotFound(path.to_path_buf()));
        }
        let content = std::fs::read_to_string(path)?;
        let manifest: Manifest = toml::from_str(&content)?;

        if manifest.package.name.is_empty() {
            return Err(PackageError::MissingName);
        }

        Ok(manifest)
    }

    pub fn save_to_file<P: AsRef<Path>>(&self, path: P) -> Result<(), PackageError> {
        let content = toml::to_string_pretty(self)?;
        std::fs::write(path, content)?;
        Ok(())
    }
}

pub struct PackageManager {
    pub packages_dir: PathBuf,
    pub cache_dir: PathBuf,
    pub registry_url: Option<String>,
    pub offline: bool,
}

impl PackageManager {
    pub fn new<P: AsRef<Path>>(packages_dir: P) -> Self {
        let cache_base = dirs::cache_dir()
            .map(|d| d.join("pleno").join("cache"))
            .unwrap_or_else(|| {
                // Fallback for systems without a standard cache dir
                dirs::home_dir()
                    .map(|h| h.join(".pleno").join("cache"))
                    .unwrap_or_else(|| std::env::current_dir().unwrap().join(".pleno_cache"))
            });

        Self {
            packages_dir: packages_dir.as_ref().to_path_buf(),
            cache_dir: cache_base,
            registry_url: None,
            offline: false,
        }
    }

    pub fn with_registry(mut self, url: String) -> Self {
        self.registry_url = Some(url);
        self
    }

    pub fn with_offline(mut self, offline: bool) -> Self {
        self.offline = offline;
        self
    }

    pub fn install_all(
        &self,
        manifest: &Manifest,
        features: &[String],
    ) -> Result<(), PackageError> {
        let lockfile_path = Path::new("pleno.lock");
        let lockfile = Lockfile::load_from_file(lockfile_path)?;

        let composite = CompositeRegistry {
            local: self,
            remote: self
                .registry_url
                .as_ref()
                .map(|url| RemoteRegistry::new(url.clone())),
        };

        let resolver = Resolver::new(&composite);
        let resolved = resolver.resolve(manifest, &lockfile, features)?;

        let mut packages = Vec::new();
        for (name, (version, source, sha256)) in resolved {
            println!("Installing {} v{} from {}...", name, version, source);
            let resolved_sha = self.fetch_package(&name, &source, sha256.as_deref())?;

            packages.push(LockPackage {
                name,
                version: version.to_string(),
                source: source.clone(),
                commit: if source.starts_with("git+") {
                    resolved_sha.clone()
                } else {
                    None
                },
                checksum: if source.starts_with("http") {
                    resolved_sha
                } else {
                    sha256
                },
            });
        }

        let new_lock = Lockfile { packages };
        new_lock.save_to_file(lockfile_path)?;

        Ok(())
    }

    pub fn publish(&self, manifest: &Manifest) -> Result<(), PackageError> {
        let token = std::env::var("PLENO_TOKEN")
            .map_err(|_| PackageError::Io(std::io::Error::other("PLENO_TOKEN not set")))?;

        // Extract Git metadata
        let git_url = self.get_git_remote_url()?;
        let commit = self.get_git_current_commit()?;

        let publish_data = PublishPackage {
            name: manifest.package.name.clone(),
            version: manifest.package.version.clone(),
            git_url,
            commit,
            sha256: None, // Optional: could hash the tree here
        };

        if let Some(registry_url) = &self.registry_url {
            let remote = RemoteRegistry::new(registry_url.clone());
            remote.publish(publish_data, &token)?;
        } else {
            return Err(PackageError::Io(std::io::Error::other(
                "No registry configured for publishing",
            )));
        }

        Ok(())
    }

    fn get_git_remote_url(&self) -> Result<String, PackageError> {
        let output = std::process::Command::new("git")
            .args(["remote", "get-url", "origin"])
            .output()?;
        if !output.status.success() {
            return Err(PackageError::Io(std::io::Error::other(
                "git remote get-url failed",
            )));
        }
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }

    fn get_git_current_commit(&self) -> Result<String, PackageError> {
        let output = std::process::Command::new("git")
            .args(["rev-parse", "HEAD"])
            .output()?;
        if !output.status.success() {
            return Err(PackageError::Io(std::io::Error::other(
                "git rev-parse HEAD failed",
            )));
        }
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }
}

pub struct CompositeRegistry<'a> {
    pub local: &'a PackageManager,
    pub remote: Option<RemoteRegistry>,
}

impl<'a> Registry for CompositeRegistry<'a> {
    fn query(&self, name: &str, req: &VersionReq) -> Result<Vec<Version>, PackageError> {
        let mut versions = self.local.query(name, req)?;
        if let Some(remote) = &self.remote {
            versions.extend(remote.query(name, req)?);
        }
        versions.sort();
        versions.dedup();
        Ok(versions)
    }

    fn fetch_source(
        &self,
        name: &str,
        version: &Version,
    ) -> Result<(String, Option<String>), PackageError> {
        if let Ok(source) = self.local.fetch_source(name, version) {
            return Ok(source);
        }
        if let Some(remote) = &self.remote {
            return remote.fetch_source(name, version);
        }
        Err(PackageError::Io(std::io::Error::other(
            "Package not found in any registry",
        )))
    }

    fn publish(&self, package: PublishPackage, token: &str) -> Result<(), PackageError> {
        if let Some(remote) = &self.remote {
            remote.publish(package, token)
        } else {
            Err(PackageError::Io(std::io::Error::other(
                "No remote registry configured for publishing",
            )))
        }
    }
}

impl Registry for PackageManager {
    fn query(&self, _name: &str, _req: &VersionReq) -> Result<Vec<Version>, PackageError> {
        // Mock local registry: only has version 0.1.0
        Ok(vec![Version::new(0, 1, 0)])
    }

    fn fetch_source(
        &self,
        _name: &str,
        _version: &Version,
    ) -> Result<(String, Option<String>), PackageError> {
        // Mock local source: doesn't know where to fetch from
        Err(PackageError::Io(std::io::Error::other(
            "Local registry fetch not implemented",
        )))
    }

    fn publish(&self, _package: PublishPackage, _token: &str) -> Result<(), PackageError> {
        Err(PackageError::Io(std::io::Error::other(
            "Local registry does not support publishing",
        )))
    }
}

impl PackageManager {
    fn fetch_package(
        &self,
        name: &str,
        source: &str,
        expected_sha: Option<&str>,
    ) -> Result<Option<String>, PackageError> {
        let mut resolved_sha = expected_sha.map(|s| s.to_string());
        let dest = self.packages_dir.join(name);

        // 1. Try global cache first if we have a SHA
        if let Some(cache_path) = self.get_cache_path(name, expected_sha)
            && cache_path.exists()
        {
            if dest.exists() {
                std::fs::remove_dir_all(&dest)?;
            }
            std::fs::create_dir_all(&dest)?;
            self.copy_dir(&cache_path, &dest)?;
            return Ok(expected_sha.map(|s| s.to_string()));
        }

        // 2. Fetch to a temporary location or directly to cache
        // Then copy to dest.

        if self.offline {
            return Err(PackageError::Io(std::io::Error::other(format!(
                "Cannot fetch {name} in offline mode: not found in cache"
            ))));
        }

        // For simplicity, we fetch to 'dest' first, verify, then mirror to cache.
        if dest.exists() {
            std::fs::remove_dir_all(&dest)?;
        }
        std::fs::create_dir_all(&dest)?;

        if let Some(url) = source.strip_prefix("git+") {
            let status = std::process::Command::new("git")
                .arg("clone")
                .arg("--depth")
                .arg("1")
                .arg(url)
                .arg(&dest)
                .status()?;
            if !status.success() {
                return Err(PackageError::Io(std::io::Error::other(format!(
                    "git clone failed for {url}"
                ))));
            }
            // Resolve actual commit SHA
            resolved_sha = Some(self.get_git_commit(&dest)?);
        } else if source.starts_with("http") {
            let resp = reqwest::blocking::get(source)
                .map_err(|e| PackageError::Io(std::io::Error::other(e)))?;
            let bytes = resp
                .bytes()
                .map_err(|e| PackageError::Io(std::io::Error::other(e)))?;

            if let Some(sha) = expected_sha {
                self.verify_checksum(&bytes, sha)?;
            }

            let file_name = source.split('/').next_back().unwrap_or("package.pln");
            std::fs::write(dest.join(file_name), bytes)?;
        } else if source.starts_with("./") || source.starts_with("../") || source.starts_with("/") {
            let src_path = PathBuf::from(source);
            self.copy_dir(&src_path, &dest)?;
        } else {
            return Err(PackageError::Io(std::io::Error::other(format!(
                "Unsupported source: {source}"
            ))));
        }

        // 3. Mirror to cache if we have a SHA
        if let Some(sha_to_cache) = &resolved_sha
            && let Some(cache_path) = self.get_cache_path(name, Some(sha_to_cache))
            && !cache_path.exists()
        {
            std::fs::create_dir_all(&cache_path)?;
            self.copy_dir(&dest, &cache_path)?;
        }

        Ok(resolved_sha)
    }

    fn get_git_commit(&self, path: &Path) -> Result<String, PackageError> {
        let output = std::process::Command::new("git")
            .args(["rev-parse", "HEAD"])
            .current_dir(path)
            .output()?;
        if !output.status.success() {
            return Err(PackageError::Io(std::io::Error::other(
                "git rev-parse failed",
            )));
        }
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }

    fn get_cache_path(&self, name: &str, sha: Option<&str>) -> Option<PathBuf> {
        sha.map(|s| self.cache_dir.join("packages").join(name).join(s))
    }

    fn verify_checksum(&self, bytes: &[u8], expected: &str) -> Result<(), PackageError> {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(bytes);
        let result = hasher.finalize();
        let hex_result = hex::encode(result);
        if hex_result != expected {
            return Err(PackageError::Io(std::io::Error::other(format!(
                "Checksum mismatch! Expected {}, got {}",
                expected, hex_result
            ))));
        }
        Ok(())
    }

    fn copy_dir(&self, src: &Path, dst: &Path) -> Result<(), PackageError> {
        if !src.exists() {
            return Err(PackageError::NotFound(src.to_path_buf()));
        }
        for entry in std::fs::read_dir(src)? {
            let entry = entry?;
            let ty = entry.file_type()?;
            let dst_path = dst.join(entry.file_name());
            if ty.is_dir() {
                std::fs::create_dir_all(&dst_path)?;
                self.copy_dir(&entry.path(), &dst_path)?;
            } else {
                std::fs::copy(entry.path(), dst_path)?;
            }
        }
        Ok(())
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
        let mut parser = Parser::new(tokens);
        let program = parser
            .parse_program()
            .map_err(|e| PackageError::ParseError(format!("{:?}", e)))?;

        let mut compiler = Compiler::new(source);
        let chunk = compiler
            .compile(&program)
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
