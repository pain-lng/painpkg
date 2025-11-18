// Manifest (pain.toml) handling

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Manifest {
    pub name: String,
    pub version: String,
    pub description: Option<String>,
    pub authors: Vec<String>,
    pub license: Option<String>,
    pub dependencies: HashMap<String, String>,
    pub dev_dependencies: HashMap<String, String>,
}

impl Manifest {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            version: "0.1.0".to_string(),
            description: None,
            authors: vec!["Pain Team".to_string()],
            license: Some("MIT OR Apache-2.0".to_string()),
            dependencies: HashMap::new(),
            dev_dependencies: HashMap::new(),
        }
    }

    pub fn load_from_file(path: &Path) -> anyhow::Result<Self> {
        let content = fs::read_to_string(path)?;
        let manifest: Manifest = toml::from_str(&content)?;
        Ok(manifest)
    }

    pub fn write_to_file(&self, path: &Path) -> anyhow::Result<()> {
        let content = toml::to_string_pretty(self)?;
        fs::write(path, content)?;
        Ok(())
    }

    pub fn add_dependency(&mut self, name: &str, version: &str) -> anyhow::Result<()> {
        if self.dependencies.contains_key(name) {
            return Err(anyhow::anyhow!("Dependency {} already exists", name));
        }
        self.dependencies.insert(name.to_string(), version.to_string());
        Ok(())
    }

    pub fn add_dev_dependency(&mut self, name: &str, version: &str) -> anyhow::Result<()> {
        if self.dev_dependencies.contains_key(name) {
            return Err(anyhow::anyhow!("Dev dependency {} already exists", name));
        }
        self.dev_dependencies.insert(name.to_string(), version.to_string());
        Ok(())
    }

    pub fn remove_dependency(&mut self, name: &str) -> anyhow::Result<()> {
        if self.dependencies.remove(name).is_none() && self.dev_dependencies.remove(name).is_none() {
            return Err(anyhow::anyhow!("Dependency {} not found", name));
        }
        Ok(())
    }

    pub fn validate(&self) -> anyhow::Result<()> {
        if self.name.is_empty() {
            return Err(anyhow::anyhow!("Package name cannot be empty"));
        }
        if self.version.is_empty() {
            return Err(anyhow::anyhow!("Package version cannot be empty"));
        }
        // Validate version format (basic semver check)
        semver::Version::parse(&self.version)
            .map_err(|e| anyhow::anyhow!("Invalid version format: {}", e))?;
        Ok(())
    }
}

