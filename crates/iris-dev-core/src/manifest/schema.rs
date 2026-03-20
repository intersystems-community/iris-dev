use serde::Deserialize;
use std::collections::HashMap;

/// Root iris-dev.toml manifest.
/// Designed to be extensible: [provides] covers developer tooling now;
/// [iris_app] is reserved for future IRIS application deployment.
#[derive(Debug, Deserialize)]
pub struct Manifest {
    pub package: PackageInfo,
    pub provides: Option<Provides>,
    #[serde(default)]
    pub dependencies: HashMap<String, DependencySpec>,
    // pub iris_app: Option<IrisApp>,  // Future: IRIS application deployment
}

#[derive(Debug, Deserialize)]
pub struct PackageInfo {
    pub name: String,
    pub version: String,
    pub description: Option<String>,
    #[serde(default)]
    pub authors: Vec<String>,
    pub license: Option<String>,
    pub repository: Option<String>,
}

/// Developer tooling package contents.
#[derive(Debug, Deserialize, Default)]
pub struct Provides {
    /// Relative paths to SKILL.md files
    #[serde(default)]
    pub skills: Vec<String>,
    /// Relative paths to KB markdown files
    #[serde(default)]
    pub kb_items: Vec<String>,
    /// iris-dev-* binary names this package provides
    #[serde(default)]
    pub plugins: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct DependencySpec {
    pub version: String,
    pub git: Option<String>,
    pub github: Option<String>,
    pub openexchange: Option<String>,
    pub repository: Option<String>,
}
