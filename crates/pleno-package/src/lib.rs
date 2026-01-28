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
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Manifest {
    pub package: PackageSection,
    #[serde(default)]
    pub scripts: HashMap<String, String>,
    #[serde(default)]
    pub dependencies: HashMap<String, String>,
    #[serde(rename = "dev-dependencies", default)]
    pub dev_dependencies: HashMap<String, String>,
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
}

impl PackageManager {
    pub fn new<P: AsRef<Path>>(packages_dir: P) -> Self {
        Self {
            packages_dir: packages_dir.as_ref().to_path_buf(),
        }
    }

    pub fn install_all(&self, manifest: &Manifest) -> Result<(), PackageError> {
        let mut installed = HashMap::new();
        let lockfile_path = Path::new("pleno.lock");
        let lockfile = Lockfile::load_from_file(lockfile_path)?;

        let mut stack = Vec::new();
        self.resolve_recursive(manifest, &mut installed, &lockfile, &mut stack)?;

        // Save new lockfile
        let mut packages = Vec::new();
        for (name, source) in installed {
            packages.push(LockPackage {
                name,
                version: "0.0.0".to_string(), // Placeholder until we have real versioning
                source,
                commit: None,
                checksum: None,
            });
        }
        let new_lock = Lockfile { packages };
        new_lock.save_to_file(lockfile_path)?;

        Ok(())
    }

    fn resolve_recursive(
        &self,
        manifest: &Manifest,
        installed: &mut HashMap<String, String>,
        lockfile: &Lockfile,
        stack: &mut Vec<String>,
    ) -> Result<(), PackageError> {
        if stack.contains(&manifest.package.name) {
            println!(
                "Warning: Dependency cycle detected at {}",
                manifest.package.name
            );
            return Ok(());
        }
        stack.push(manifest.package.name.clone());

        for (name, source) in &manifest.dependencies {
            // Check lockfile first
            let resolved_source = lockfile.get_dependency_source(name).unwrap_or(source);

            if let Some(existing_source) = installed.get(name) {
                if existing_source != resolved_source {
                    println!(
                        "Warning: Version mismatch for {}: {} vs {}",
                        name, existing_source, resolved_source
                    );
                }
                continue;
            }

            println!("Installing {} from {}...", name, resolved_source);
            self.fetch_package(name, resolved_source)?;
            installed.insert(name.clone(), resolved_source.clone());

            // Load manifest of the installed package and resolve its dependencies
            let pkg_dir = self.packages_dir.join(name);
            let pkg_manifest_path = pkg_dir.join("pleno.toml");
            if pkg_manifest_path.exists() {
                let sub_manifest = Manifest::load_from_file(pkg_manifest_path)?;
                self.resolve_recursive(&sub_manifest, installed, lockfile, stack)?;
            }
        }
        stack.pop();
        Ok(())
    }

    fn fetch_package(&self, name: &str, source: &str) -> Result<(), PackageError> {
        let dest = self.packages_dir.join(name);
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
        } else if source.starts_with("./") || source.starts_with("../") || source.starts_with("/") {
            let src_path = PathBuf::from(source);
            self.copy_dir(&src_path, &dest)?;
        } else {
            return Err(PackageError::Io(std::io::Error::other(format!(
                "Unsupported source: {source}"
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
}
