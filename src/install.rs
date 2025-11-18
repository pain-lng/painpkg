// Package installation

use crate::package::Package;
use crate::resolver::ResolvedPackage;
use crate::registry::Registry;
use std::path::PathBuf;

pub fn install_packages(packages: &[ResolvedPackage]) -> anyhow::Result<()> {
    // Determine installation directory
    let install_dir = get_install_directory()?;
    std::fs::create_dir_all(&install_dir)?;

    let registry = Registry::new()?;

    for resolved in packages {
        let package = &resolved.package;
        println!("  Installing {} v{}...", package.name, package.version);
        
        registry.install_package(package, &install_dir)?;
    }

    Ok(())
}

fn get_install_directory() -> anyhow::Result<PathBuf> {
    // Check for .pain directory in current project
    let current_dir = std::env::current_dir()?;
    let local_install = current_dir.join(".pain").join("packages");
    
    // Prefer local installation (project-specific)
    Ok(local_install)
}

pub fn get_package_path(package_name: &str, version: &semver::Version) -> anyhow::Result<PathBuf> {
    let install_dir = get_install_directory()?;
    Ok(install_dir.join(package_name).join(version.to_string()))
}

pub fn find_installed_package(package_name: &str) -> anyhow::Result<Option<Package>> {
    let install_dir = get_install_directory()?;
    let package_dir = install_dir.join(package_name);

    if !package_dir.exists() {
        return Ok(None);
    }

    // Find latest version
    let mut versions = Vec::new();
    for entry in std::fs::read_dir(&package_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            if let Some(version_str) = path.file_name().and_then(|n| n.to_str()) {
                if let Ok(version) = semver::Version::parse(version_str) {
                    versions.push((version, path));
                }
            }
        }
    }

    if versions.is_empty() {
        return Ok(None);
    }

    // Get latest version
    versions.sort_by(|a, b| b.0.cmp(&a.0));
    let (_, latest_path) = &versions[0];

    Package::load_from_path(latest_path).map(Some)
}

