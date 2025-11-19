// Package metadata and structure

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Package {
    pub name: String,
    pub version: semver::Version,
    pub description: Option<String>,
    pub authors: Vec<String>,
    pub license: Option<String>,
    pub dependencies: HashMap<String, String>,
    pub source: PackageSource,
    pub path: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PackageSource {
    Local(PathBuf),
    File(PathBuf),
    Registry(String),
    Git { url: String, rev: Option<String> },
}

impl Package {
    pub fn from_manifest(
        manifest: &crate::manifest::Manifest,
        source: PackageSource,
        path: PathBuf,
    ) -> anyhow::Result<Self> {
        let version = semver::Version::parse(&manifest.version)?;

        Ok(Self {
            name: manifest.name.clone(),
            version,
            description: manifest.description.clone(),
            authors: manifest.authors.clone(),
            license: manifest.license.clone(),
            dependencies: manifest.dependencies.clone(),
            source,
            path,
        })
    }

    pub fn load_from_path(path: &Path) -> anyhow::Result<Self> {
        let manifest_path = path.join("pain.toml");
        if !manifest_path.exists() {
            return Err(anyhow::anyhow!("pain.toml not found in package directory"));
        }

        let manifest = crate::manifest::Manifest::load_from_file(&manifest_path)?;
        let source = PackageSource::Local(path.to_path_buf());

        Self::from_manifest(&manifest, source, path.to_path_buf())
    }

    pub fn get_source_files(&self) -> anyhow::Result<Vec<PathBuf>> {
        let src_dir = self.path.join("src");
        if !src_dir.exists() {
            return Ok(Vec::new());
        }

        let mut files = Vec::new();
        for entry in walkdir::WalkDir::new(&src_dir) {
            let entry = entry?;
            if entry.file_type().is_file() {
                if let Some(ext) = entry.path().extension() {
                    if ext == "pain" {
                        files.push(entry.path().to_path_buf());
                    }
                }
            }
        }

        Ok(files)
    }
}
