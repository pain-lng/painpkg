// Dependency resolution algorithm

use crate::manifest::Manifest;
use crate::package::Package;
use crate::registry::Registry;
use semver::{Version, VersionReq};
use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;

pub struct DependencyResolver {
    registry: Registry,
}

#[derive(Debug, Clone)]
pub struct ResolvedPackage {
    pub package: Package,
    pub depth: usize,
}

impl DependencyResolver {
    pub fn new(registry: Registry) -> Self {
        Self { registry }
    }

    pub fn resolve(&mut self, manifest: &Manifest) -> anyhow::Result<Vec<ResolvedPackage>> {
        // Map of package name -> set of version requirements
        let mut requirements: HashMap<String, Vec<VersionReq>> = HashMap::new();
        // Map of package name -> resolved package
        let mut resolved: HashMap<String, ResolvedPackage> = HashMap::new();
        // Queue of packages to resolve: (name, depth, source_package, source_spec)
        let mut to_resolve: VecDeque<(String, usize, Option<String>, Option<String>)> =
            VecDeque::new();

        // Add root dependencies
        for (name, version_req_str) in &manifest.dependencies {
            let version_req = self.parse_version_req(version_req_str)?;
            requirements
                .entry(name.clone())
                .or_default()
                .push(version_req.clone());
            to_resolve.push_back((name.clone(), 0, None, Some(version_req_str.clone())));
        }

        // Resolve dependencies using BFS
        while let Some((name, depth, _source, source_spec)) = to_resolve.pop_front() {
            // Skip if already resolved
            if resolved.contains_key(&name) {
                continue;
            }

            // Get all version requirements for this package
            let version_reqs = requirements.get(&name).ok_or_else(|| {
                anyhow::anyhow!("No version requirements found for package: {}", name)
            })?;

            // Find a version that satisfies all requirements
            // If source_spec is provided, try to load from that source first
            let package = if let Some(spec) = source_spec {
                self.find_package_from_source(&name, &spec, version_reqs)?
            } else {
                self.find_compatible_version(&name, version_reqs)?
            };

            // Add to resolved
            resolved.insert(
                name.clone(),
                ResolvedPackage {
                    package: package.clone(),
                    depth,
                },
            );

            // Add dependencies of this package
            for (dep_name, dep_version_req_str) in &package.dependencies {
                let dep_version_req = self.parse_version_req(dep_version_req_str)?;

                // Add requirement
                requirements
                    .entry(dep_name.clone())
                    .or_default()
                    .push(dep_version_req.clone());

                // Check if dependency is already resolved
                if let Some(existing) = resolved.get(dep_name) {
                    // Verify that existing version satisfies new requirement
                    if !dep_version_req.matches(&existing.package.version) {
                        return Err(anyhow::anyhow!(
                            "Version conflict: {} requires {} {}, but {} {} is already resolved",
                            name,
                            dep_name,
                            dep_version_req,
                            dep_name,
                            existing.package.version
                        ));
                    }
                } else {
                    // Add to queue if not already resolved
                    to_resolve.push_back((dep_name.clone(), depth + 1, Some(name.clone()), None));
                }
            }
        }

        // Convert to vector and sort by depth
        let mut result: Vec<ResolvedPackage> = resolved.into_values().collect();
        result.sort_by_key(|p| p.depth);

        Ok(result)
    }

    fn find_compatible_version(
        &mut self,
        name: &str,
        version_reqs: &[VersionReq],
    ) -> anyhow::Result<Package> {
        // Get all available versions from registry
        let mut all_packages = self.registry.find_all_versions(name)?;

        if all_packages.is_empty() {
            return Err(anyhow::anyhow!("Package not found: {}", name));
        }

        // Sort by version (latest first)
        all_packages.sort_by(|a, b| b.version.cmp(&a.version));

        // Find first version that satisfies all requirements
        for package in &all_packages {
            if version_reqs.iter().all(|req| req.matches(&package.version)) {
                return Ok(package.clone());
            }
        }

        // No compatible version found
        Err(anyhow::anyhow!(
            "No version of {} found that satisfies all requirements: {:?}",
            name,
            version_reqs
        ))
    }

    fn find_package_from_source(
        &mut self,
        name: &str,
        source_spec: &str,
        version_reqs: &[VersionReq],
    ) -> anyhow::Result<Package> {
        // Parse source specification
        if source_spec.starts_with("file://") {
            // Local file path
            let path_str = source_spec.strip_prefix("file://").unwrap();
            let path = PathBuf::from(path_str);
            let package = Package::load_from_path(&path)?;

            // Verify version matches requirements
            if version_reqs.iter().all(|req| req.matches(&package.version)) {
                return Ok(package);
            }
            return Err(anyhow::anyhow!(
                "Package {} from file://{} has version {} which doesn't satisfy requirements: {:?}",
                name,
                path_str,
                package.version,
                version_reqs
            ));
        } else if source_spec.starts_with("git://") || source_spec.starts_with("git+") {
            // Git repository
            let url = source_spec
                .strip_prefix("git://")
                .or_else(|| source_spec.strip_prefix("git+"))
                .unwrap();

            // Download from git
            let package = self.registry.fetch_from_git(name, url, None)?;

            // Verify version matches requirements
            if version_reqs.iter().all(|req| req.matches(&package.version)) {
                return Ok(package);
            }
            return Err(anyhow::anyhow!(
                "Package {} from git://{} has version {} which doesn't satisfy requirements: {:?}",
                name,
                url,
                package.version,
                version_reqs
            ));
        }

        // Fall back to regular registry lookup
        self.find_compatible_version(name, version_reqs)
    }

    fn parse_version_req(&self, version_str: &str) -> anyhow::Result<VersionReq> {
        // Handle source specifications (file://, git://, git+)
        let is_source_spec = version_str.starts_with("file://")
            || version_str.starts_with("git://")
            || version_str.starts_with("git+");

        let version_str = if is_source_spec {
            // For source specifications, use * as version requirement
            // The actual version will be checked when loading the package
            "*"
        } else {
            version_str.trim()
        };

        if version_str == "*" || version_str.is_empty() {
            return Ok(VersionReq::STAR);
        }

        // Try parsing as exact version
        if let Ok(version) = Version::parse(version_str) {
            return Ok(VersionReq::parse(&format!("={}", version))?);
        }

        // Try parsing as version requirement
        VersionReq::parse(version_str)
            .map_err(|e| anyhow::anyhow!("Invalid version requirement '{}': {}", version_str, e))
    }
}
