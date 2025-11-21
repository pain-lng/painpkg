// Package registry - manages package discovery and retrieval

use crate::package::{Package, PackageSource};
use crate::registry_metadata::RegistryPackageMetadata;
use semver::VersionReq;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct IndexEntry {
    version: String,
    path: String,
}

// Default GitHub registry URL
const GITHUB_REGISTRY_URL: &str = "https://github.com/pain-lng/pain-registry.git";

pub struct Registry {
    local_registry_path: PathBuf,
    cache: HashMap<String, Vec<Package>>,
    index_path: PathBuf,
    github_registry_path: Option<PathBuf>, // Cloned GitHub registry path
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
            github_registry_path: None,
        })
    }

    /// Get or clone GitHub registry
    pub fn get_github_registry(&mut self) -> anyhow::Result<PathBuf> {
        if let Some(ref path) = self.github_registry_path {
            if path.exists() {
                return Ok(path.clone());
            }
        }

        let registry_cache = self.local_registry_path.join("github-registry");
        std::fs::create_dir_all(&registry_cache)?;

        let registry_path = registry_cache.join("pain-registry");

        // Clone or update registry
        if !registry_path.exists() {
            println!("Cloning GitHub registry...");
            let output = Command::new("git")
                .args(["clone", "--depth", "1", GITHUB_REGISTRY_URL, registry_path.to_str().unwrap()])
                .output()?;

            if !output.status.success() {
                return Err(anyhow::anyhow!(
                    "Failed to clone registry: {}",
                    String::from_utf8_lossy(&output.stderr)
                ));
            }
        } else {
            // Update existing registry
            // First, ensure we're on main/master branch
            let _ = Command::new("git")
                .args(["-C", registry_path.to_str().unwrap(), "checkout", "main"])
                .output();
            let _ = Command::new("git")
                .args(["-C", registry_path.to_str().unwrap(), "checkout", "master"])
                .output();
            
            // Then pull latest changes
            let output = Command::new("git")
                .args(["-C", registry_path.to_str().unwrap(), "pull"])
                .output()?;

            if !output.status.success() {
                eprintln!("Warning: Failed to update registry: {}", String::from_utf8_lossy(&output.stderr));
            }
        }

        self.github_registry_path = Some(registry_path.clone());
        Ok(registry_path)
    }

    /// Get index path for package name (hierarchical: index/p/a/pa/pain-math)
    fn get_index_path(&self, registry_path: &Path, name: &str) -> PathBuf {
        let mut index_path = registry_path.join("index");
        
        // Create hierarchical path: first 1, 2, 4 chars
        // Example: "pain-math" -> index/p/a/pa/pain-math
        let chars: Vec<char> = name.chars().collect();
        if !chars.is_empty() {
            let prefix: String = chars[..1].iter().collect();
            index_path.push(&prefix);
        }
        if chars.len() >= 2 {
            let prefix: String = chars[..2].iter().collect();
            index_path.push(&prefix);
        }
        if chars.len() >= 4 {
            let prefix: String = chars[..4].iter().collect();
            index_path.push(&prefix);
        }
        index_path.push(name);
        
        index_path
    }

    /// Load package metadata from GitHub registry
    pub fn load_from_github_registry(&mut self, name: &str, version: &str) -> anyhow::Result<RegistryPackageMetadata> {
        let registry_path = self.get_github_registry()?;
        let metadata_path = registry_path
            .join("packages")
            .join(name)
            .join(format!("{}.toml", version));

        if !metadata_path.exists() {
            return Err(anyhow::anyhow!(
                "Package {} version {} not found in registry",
                name,
                version
            ));
        }

        let content = std::fs::read_to_string(&metadata_path)?;
        let metadata: RegistryPackageMetadata = toml::from_str(&content)?;
        Ok(metadata)
    }

    /// Find all versions of a package in GitHub registry
    pub fn find_versions_in_github_registry(&mut self, name: &str) -> anyhow::Result<Vec<String>> {
        let registry_path = self.get_github_registry()?;
        let package_dir = registry_path.join("packages").join(name);

        if !package_dir.exists() {
            return Ok(Vec::new());
        }

        let mut versions = Vec::new();
        for entry in std::fs::read_dir(&package_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_file() {
                if let Some(file_name) = path.file_stem().and_then(|s| s.to_str()) {
                    // File name is version.toml
                    if semver::Version::parse(file_name).is_ok() {
                        versions.push(file_name.to_string());
                    }
                }
            }
        }

        // Sort versions (newest first)
        versions.sort_by(|a, b| {
            semver::Version::parse(b)
                .unwrap()
                .cmp(&semver::Version::parse(a).unwrap())
        });

        Ok(versions)
    }

    /// Search packages in GitHub registry by name or description
    pub fn search_github_registry(&mut self, query: &str) -> anyhow::Result<Vec<(String, String, Option<String>)>> {
        let registry_path = self.get_github_registry()?;
        let packages_dir = registry_path.join("packages");

        if !packages_dir.exists() {
            return Ok(Vec::new());
        }

        let query_lower = query.to_lowercase();
        let mut results = Vec::new();

        for entry in std::fs::read_dir(&packages_dir)? {
            let entry = entry?;
            let package_dir = entry.path();
            if !package_dir.is_dir() {
                continue;
            }

            let package_name = package_dir.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("")
                .to_string();

            // Check if name matches
            if package_name.to_lowercase().contains(&query_lower) {
                // Get latest version
                if let Ok(versions) = self.find_versions_in_github_registry(&package_name) {
                    if let Some(latest_version) = versions.first() {
                        if let Ok(metadata) = self.load_from_github_registry(&package_name, latest_version) {
                            results.push((
                                package_name.clone(),
                                latest_version.clone(),
                                metadata.description,
                            ));
                        }
                    }
                }
                continue;
            }

            // Check description in all versions
            if let Ok(versions) = self.find_versions_in_github_registry(&package_name) {
                for version in versions {
                    if let Ok(metadata) = self.load_from_github_registry(&package_name, &version) {
                        if let Some(ref desc) = metadata.description {
                            if desc.to_lowercase().contains(&query_lower) {
                                results.push((
                                    package_name.clone(),
                                    version,
                                    metadata.description,
                                ));
                                break;
                            }
                        }
                    }
                }
            }
        }

        Ok(results)
    }

    /// Prepare package for publishing to GitHub registry
    /// Returns paths to created files that should be committed
    pub fn prepare_github_publish(
        &mut self,
        package: &Package,
        repository_url: &str,
    ) -> anyhow::Result<(PathBuf, PathBuf)> {
        let registry_path = self.get_github_registry()?;

        // Create package metadata
        let metadata = RegistryPackageMetadata::from_manifest(
            &package.name,
            &package.version.to_string(),
            package.description.as_deref(),
            repository_url,
            package.license.as_deref(),
            &package.authors,
            &package.dependencies,
        );

        // Create packages/{name}/{version}.toml
        let package_dir = registry_path.join("packages").join(&package.name);
        std::fs::create_dir_all(&package_dir)?;
        let metadata_path = package_dir.join(format!("{}.toml", package.version));

        let toml_content = toml::to_string_pretty(&metadata)?;
        std::fs::write(&metadata_path, toml_content)?;

        // Create or update index entry: index/p/a/pa/pain-math
        let index_path = self.get_index_path(&registry_path, &package.name);
        std::fs::create_dir_all(index_path.parent().unwrap())?;

        // Read existing index if it exists
        let mut index_entries: Vec<String> = if index_path.exists() {
            let content = std::fs::read_to_string(&index_path)?;
            content.lines().map(|s| s.to_string()).collect()
        } else {
            Vec::new()
        };

        // Add version if not already present
        let version_line = format!("{}", package.version);
        if !index_entries.contains(&version_line) {
            index_entries.push(version_line);
            index_entries.sort();
        }

        // Write index file (one version per line)
        std::fs::write(&index_path, index_entries.join("\n") + "\n")?;

        Ok((metadata_path, index_path))
    }

    #[allow(dead_code)]
    pub fn find_package(
        &mut self,
        name: &str,
        version_req: &VersionReq,
    ) -> anyhow::Result<Option<Package>> {
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

        // Try GitHub registry first
        if let Ok(versions) = self.find_versions_in_github_registry(name) {
            let mut packages = Vec::new();
            for version in versions {
                if let Ok(metadata) = self.load_from_github_registry(name, &version) {
                    // Create Package from registry metadata
                    let version_parsed = semver::Version::parse(&version)?;
                    let package = Package {
                        name: metadata.name.clone(),
                        version: version_parsed,
                        description: metadata.description.clone(),
                        authors: metadata.authors.clone(),
                        license: metadata.license.clone(),
                        dependencies: metadata.dependencies.clone(),
                        source: PackageSource::Git {
                            url: metadata.repository.clone(),
                            rev: Some(format!("v{}", version)),
                        },
                        path: PathBuf::from(""), // Will be set when installed
                    };
                    packages.push(package);
                }
            }
            if !packages.is_empty() {
                self.cache.insert(name.to_string(), packages.clone());
                return Ok(packages);
            }
        }

        // Try to load from local index
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

        let entries: Vec<IndexEntry> = packages
            .iter()
            .map(|pkg| IndexEntry {
                version: pkg.version.to_string(),
                path: pkg
                    .path
                    .strip_prefix(&self.local_registry_path)
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

    #[allow(dead_code)]
    pub fn publish_package(&self, package: &Package) -> anyhow::Result<()> {
        // Create package directory in registry
        let package_dir = self
            .local_registry_path
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

    pub fn fetch_from_git(
        &mut self,
        _name: &str,
        url: &str,
        rev: Option<&str>,
    ) -> anyhow::Result<Package> {
        // Create cache directory for git packages
        let git_cache = self.local_registry_path.join("git");
        std::fs::create_dir_all(&git_cache)?;

        // Create a unique directory name from URL
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(url.as_bytes());
        let url_hash = hex::encode(hasher.finalize());
        let cache_dir = git_cache.join(&url_hash[..16]); // Use first 16 chars

        // Clone or update repository
        if !cache_dir.exists() {
            let output = std::process::Command::new("git")
                .args(["clone", url, cache_dir.to_str().unwrap()])
                .output()?;

            if !output.status.success() {
                return Err(anyhow::anyhow!(
                    "Failed to clone git repository: {}",
                    String::from_utf8_lossy(&output.stderr)
                ));
            }
        } else {
            // Update existing repository
            let output = std::process::Command::new("git")
                .args(["-C", cache_dir.to_str().unwrap(), "fetch"])
                .output()?;

            if !output.status.success() {
                return Err(anyhow::anyhow!(
                    "Failed to fetch git repository: {}",
                    String::from_utf8_lossy(&output.stderr)
                ));
            }
        }

        // Checkout specific revision if provided
        if let Some(revision) = rev {
            let output = std::process::Command::new("git")
                .args(["-C", cache_dir.to_str().unwrap(), "checkout", revision])
                .output()?;

            if !output.status.success() {
                return Err(anyhow::anyhow!(
                    "Failed to checkout revision {}: {}",
                    revision,
                    String::from_utf8_lossy(&output.stderr)
                ));
            }
        }

        // Load package from cloned directory
        Package::load_from_path(&cache_dir)
    }

    pub fn install_package(&mut self, package: &Package, target_dir: &Path) -> anyhow::Result<()> {
        let install_path = target_dir
            .join(&package.name)
            .join(package.version.to_string());
        std::fs::create_dir_all(&install_path)?;

        // If package source is Git, fetch it first
        let source_path = match &package.source {
            PackageSource::Git { url, rev } => {
                // Fetch from git
                self.fetch_from_git(&package.name, url, rev.as_deref())?.path
            }
            _ => package.path.clone(),
        };

        // Copy package files
        self.copy_package_files(&source_path, &install_path)?;

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
