// Registry metadata format for GitHub-based registry
// Format: packages/{name}/{version}.toml

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistryPackageMetadata {
    pub name: String,
    pub version: String,
    pub description: Option<String>,
    pub repository: String, // Git repository URL
    pub license: Option<String>,
    pub authors: Vec<String>,
    pub dependencies: HashMap<String, String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub readme: Option<String>, // Path to README in repository
}

impl RegistryPackageMetadata {
    pub fn from_manifest(
        name: &str,
        version: &str,
        description: Option<&str>,
        repository: &str,
        license: Option<&str>,
        authors: &[String],
        dependencies: &HashMap<String, String>,
    ) -> Self {
        Self {
            name: name.to_string(),
            version: version.to_string(),
            description: description.map(|s| s.to_string()),
            repository: repository.to_string(),
            license: license.map(|s| s.to_string()),
            authors: authors.to_vec(),
            dependencies: dependencies.clone(),
            readme: None,
        }
    }
}

