use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum PackageError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Parse error: {0}")]
    Parse(#[from] serde_json::Error),
    #[error("Package manifest not found at {0}")]
    NotFound(PathBuf),
    #[error("Missing 'name' field in manifest")]
    MissingName,
    #[error("Missing 'version' field in manifest")]
    MissingVersion,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Manifest {
    pub name: String,
    pub version: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default = "default_main")]
    pub main: String,
    #[serde(default)]
    pub scripts: HashMap<String, String>,
    #[serde(default)]
    pub dependencies: HashMap<String, String>,
    #[serde(rename = "devDependencies", default)]
    pub dev_dependencies: HashMap<String, String>,
}

fn default_main() -> String {
    "src/main.pln".to_string()
}

impl Manifest {
    pub fn new(name: &str, version: &str) -> Self {
        Self {
            name: name.to_string(),
            version: version.to_string(),
            description: None,
            main: default_main(),
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
        let manifest: Manifest = serde_json::from_str(&content)?;

        if manifest.name.is_empty() {
            return Err(PackageError::MissingName);
        }

        Ok(manifest)
    }

    pub fn save_to_file<P: AsRef<Path>>(&self, path: P) -> Result<(), PackageError> {
        let content = serde_json::to_string_pretty(self)?;
        std::fs::write(path, content)?;
        Ok(())
    }
}
