// Package registry - manages package discovery and retrieval

use crate::package::Package;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use semver::VersionReq;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct IndexEntry {
    version: String,
    path: String,
}

pub struct Registry {
    local_registry_path: PathBuf,
    cache: HashMap<String, Vec<Package>>,
    index_path: PathBuf,
}

impl Registry {
    pub fn new() -> anyhow::Result<Self> {
        // Default local registry: ~/.pain/registry or ./.pain/registry
        let local_registry = if let Ok(home) = std::env::var("HOME") {
            PathBuf::from(home).join(".pain").join("registry")
        } else if let Ok(home) = std::env::var("USERPROFILE") {
            // Windows
            PathBuf::from(home).join(".pain").join("registry")
        } else {
            PathBuf::from(".pain").join("registry")
        };

        std::fs::create_dir_all(&local_registry)?;

        let index_path = local_registry.join("index.json");

        Ok(Self {
            local_registry_path: local_registry,
            cache: HashMap::new(),
            index_path,
        })
    }

    pub fn find_package(&mut self, name: &str, version_req: &VersionReq) -> anyhow::Result<Option<Package>> {
        let packages = self.find_all_versions(name)?;
        
        for pkg in packages {
            if version_req.matches(&pkg.version) {
                return Ok(Some(pkg));
            }
        }

        Ok(None)
    }

    pub fn find_all_versions(&mut self, name: &str) -> anyhow::Result<Vec<Package>> {
        // Check cache first
        if let Some(packages) = self.cache.get(name) {
            return Ok(packages.clone());
        }

        // Try to load from index first
        if let Ok(packages) = self.load_from_index(name) {
            if !packages.is_empty() {
                self.cache.insert(name.to_string(), packages.clone());
                return Ok(packages);
            }
        }

        // Search in local registry
        let packages = self.scan_local_registry(name)?;
        self.cache.insert(name.to_string(), packages.clone());

        // Update index
        if let Err(e) = self.update_index(name, &packages) {
            eprintln!("Warning: Failed to update index: {}", e);
        }

        Ok(packages)
    }

    fn load_from_index(&self, name: &str) -> anyhow::Result<Vec<Package>> {
        if !self.index_path.exists() {
            return Ok(Vec::new());
        }

        let content = std::fs::read_to_string(&self.index_path)?;
        let index: HashMap<String, Vec<IndexEntry>> = serde_json::from_str(&content)?;

        if let Some(entries) = index.get(name) {
            let mut packages = Vec::new();
            for entry in entries {
                let path = self.local_registry_path.join(&entry.path);
                if path.exists() {
                    if let Ok(pkg) = Package::load_from_path(&path) {
                        packages.push(pkg);
                    }
                }
            }
            return Ok(packages);
        }

        Ok(Vec::new())
    }

    fn update_index(&self, name: &str, packages: &[Package]) -> anyhow::Result<()> {
        let mut index: HashMap<String, Vec<IndexEntry>> = if self.index_path.exists() {
            let content = std::fs::read_to_string(&self.index_path)?;
            serde_json::from_str(&content).unwrap_or_default()
        } else {
            HashMap::new()
        };

        let entries: Vec<IndexEntry> = packages.iter()
            .map(|pkg| IndexEntry {
                version: pkg.version.to_string(),
                path: pkg.path.strip_prefix(&self.local_registry_path)
                    .unwrap_or(&pkg.path)
                    .to_string_lossy()
                    .to_string(),
            })
            .collect();

        index.insert(name.to_string(), entries);

        let content = serde_json::to_string_pretty(&index)?;
        std::fs::write(&self.index_path, content)?;

        Ok(())
    }

    pub fn publish_package(&self, package: &Package) -> anyhow::Result<()> {
        // Create package directory in registry
        let package_dir = self.local_registry_path
            .join(&package.name)
            .join(package.version.to_string());
        std::fs::create_dir_all(&package_dir)?;

        // Copy package files
        self.copy_package_files(&package.path, &package_dir)?;

        // Update index
        let packages = vec![package.clone()];
        self.update_index(&package.name, &packages)?;

        Ok(())
    }

    fn scan_local_registry(&self, name: &str) -> anyhow::Result<Vec<Package>> {
        let mut packages = Vec::new();

        // Scan registry directory for packages
        if !self.local_registry_path.exists() {
            return Ok(packages);
        }

        for entry in std::fs::read_dir(&self.local_registry_path)? {
            let entry = entry?;
            let path = entry.path();

            if path.is_dir() {
                // Check if this directory contains a package with matching name
                let manifest_path = path.join("pain.toml");
                if manifest_path.exists() {
                    if let Ok(pkg) = Package::load_from_path(&path) {
                        if pkg.name == name {
                            packages.push(pkg);
                        }
                    }
                }
            }
        }

        // Also check for packages in local directories (for development)
        // Look in common locations like ../packages or ./packages
        let local_packages = [
            PathBuf::from("packages"),
            PathBuf::from("../packages"),
            PathBuf::from("../../packages"),
        ];

        for packages_dir in &local_packages {
            if packages_dir.exists() {
                for entry in std::fs::read_dir(packages_dir)? {
                    let entry = entry?;
                    let path = entry.path();
                    if path.is_dir() {
                        let manifest_path = path.join("pain.toml");
                        if manifest_path.exists() {
                            if let Ok(pkg) = Package::load_from_path(&path) {
                                if pkg.name == name {
                                    packages.push(pkg);
                                }
                            }
                        }
                    }
                }
            }
        }

        Ok(packages)
    }

    pub fn fetch_from_git(&mut self, _name: &str, url: &str, rev: Option<&str>) -> anyhow::Result<Package> {
        // Create cache directory for git packages
        let git_cache = self.local_registry_path.join("git");
        std::fs::create_dir_all(&git_cache)?;

        // Create a unique directory name from URL
        use sha2::{Sha256, Digest};
        let mut hasher = Sha256::new();
        hasher.update(url.as_bytes());
        let url_hash = hex::encode(hasher.finalize());
        let cache_dir = git_cache.join(&url_hash[..16]); // Use first 16 chars

        // Clone or update repository
        if !cache_dir.exists() {
            let output = std::process::Command::new("git")
                .args(&["clone", url, cache_dir.to_str().unwrap()])
                .output()?;
            
            if !output.status.success() {
                return Err(anyhow::anyhow!("Failed to clone git repository: {}", String::from_utf8_lossy(&output.stderr)));
            }
        } else {
            // Update existing repository
            let output = std::process::Command::new("git")
                .args(&["-C", cache_dir.to_str().unwrap(), "fetch"])
                .output()?;
            
            if !output.status.success() {
                return Err(anyhow::anyhow!("Failed to fetch git repository: {}", String::from_utf8_lossy(&output.stderr)));
            }
        }

        // Checkout specific revision if provided
        if let Some(revision) = rev {
            let output = std::process::Command::new("git")
                .args(&["-C", cache_dir.to_str().unwrap(), "checkout", revision])
                .output()?;
            
            if !output.status.success() {
                return Err(anyhow::anyhow!("Failed to checkout revision {}: {}", revision, String::from_utf8_lossy(&output.stderr)));
            }
        }

        // Load package from cloned directory
        Package::load_from_path(&cache_dir)
    }

    pub fn install_package(&self, package: &Package, target_dir: &Path) -> anyhow::Result<()> {
        let install_path = target_dir.join(&package.name).join(package.version.to_string());
        std::fs::create_dir_all(&install_path)?;

        // Copy package files
        self.copy_package_files(&package.path, &install_path)?;

        Ok(())
    }

    fn copy_package_files(&self, source: &Path, dest: &Path) -> anyhow::Result<()> {
        // Copy src directory
        let src_source = source.join("src");
        let src_dest = dest.join("src");
        
        if src_source.exists() {
            copy_dir_all(&src_source, &src_dest)?;
        }

        // Copy pain.toml
        let manifest_source = source.join("pain.toml");
        if manifest_source.exists() {
            std::fs::copy(&manifest_source, dest.join("pain.toml"))?;
        }

        Ok(())
    }
}

fn copy_dir_all(src: &Path, dst: &Path) -> anyhow::Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());

        if ty.is_dir() {
            copy_dir_all(&src_path, &dst_path)?;
        } else {
            std::fs::copy(&src_path, &dst_path)?;
        }
    }
    Ok(())
}

