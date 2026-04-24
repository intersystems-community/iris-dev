use crate::manifest::schema::{DependencySpec, Manifest};
use anyhow::{anyhow, Result};
use semver::{Version, VersionReq};
use std::collections::HashSet;

pub struct Resolve {
    pub packages: Vec<ResolvedPackage>,
}

pub struct ResolvedPackage {
    pub name: String,
    pub version: Version,
    pub source: ResolvedSource,
}

#[derive(Debug, Clone)]
pub enum ResolvedSource {
    Local(std::path::PathBuf),
    Git(String),
    GitHub { owner: String, repo: String },
    OpenExchange(String),
}

impl Resolve {
    pub fn from_manifest(manifest: &Manifest) -> Result<Self> {
        let mut packages = vec![];
        let mut seen: HashSet<String> = HashSet::new();

        for (name, dep) in &manifest.dependencies {
            if seen.contains(name) {
                continue;
            }
            seen.insert(name.clone());

            let version_req = VersionReq::parse(&dep.version).map_err(|e| {
                anyhow!("invalid semver '{}' for dep '{}': {}", dep.version, name, e)
            })?;

            let source = dep_to_source(name, dep)?;
            let version = resolve_version(&version_req, &source)?;

            packages.push(ResolvedPackage {
                name: name.clone(),
                version,
                source,
            });
        }

        Ok(Self { packages })
    }

    pub fn to_lock(&self) -> ResolveLock {
        ResolveLock {
            packages: self
                .packages
                .iter()
                .map(|p| {
                    // Bug 11: format repository as a proper URL string, not Rust Debug output.
                    let repository = match &p.source {
                        ResolvedSource::GitHub { owner, repo } => {
                            format!("https://github.com/{}/{}", owner, repo)
                        }
                        ResolvedSource::Git(url) => url.clone(),
                        ResolvedSource::Local(path) => {
                            path.to_string_lossy().into_owned()
                        }
                        ResolvedSource::OpenExchange(id) => {
                            format!("openexchange:{}", id)
                        }
                    };
                    PackageLock {
                        name: p.name.clone(),
                        version: p.version.to_string(),
                        repository,
                        checksum: None,
                    }
                })
                .collect(),
        }
    }
}

fn dep_to_source(name: &str, dep: &DependencySpec) -> Result<ResolvedSource> {
    if let Some(github) = &dep.github {
        let parts: Vec<_> = github.splitn(2, '/').collect();
        if parts.len() == 2 {
            return Ok(ResolvedSource::GitHub {
                owner: parts[0].to_string(),
                repo: parts[1].to_string(),
            });
        }
    }
    if let Some(git) = &dep.git {
        return Ok(ResolvedSource::Git(git.clone()));
    }
    if let Some(repo) = &dep.repository {
        return Ok(ResolvedSource::Local(std::path::PathBuf::from(repo)));
    }
    if let Some(ox) = &dep.openexchange {
        return Ok(ResolvedSource::OpenExchange(ox.clone()));
    }
    Err(anyhow!(
        "dependency '{}' has no source (git, github, repository, or openexchange)",
        name
    ))
}

fn resolve_version(req: &VersionReq, source: &ResolvedSource) -> Result<Version> {
    // FR-019/Mo5: return explicit error instead of stub version.
    anyhow::bail!(
        "version resolution not yet implemented for source {:?} (requirement: {})",
        source,
        req
    )
}

/// Test accessor for resolve_version. Exposed for integration tests.
#[doc(hidden)]
pub fn resolve_version_for_test(req: &semver::VersionReq) -> anyhow::Result<semver::Version> {
    resolve_version(req, &ResolvedSource::GitHub { owner: "test".into(), repo: "test".into() })
}

pub struct ResolveLock {
    pub packages: Vec<PackageLock>,
}

pub struct PackageLock {
    pub name: String,
    pub version: String,
    pub repository: String,
    pub checksum: Option<String>,
}

impl ResolveLock {
    pub fn to_toml(&self) -> String {
        let mut out = String::from("[metadata]\nformat-version = 1\n\n");
        for pkg in &self.packages {
            // Bug 11: use proper TOML string quoting, not Rust Debug format ({:?}).
            out.push_str(&format!(
                "[[package]]\nname = \"{}\"\nversion = \"{}\"\nrepository = \"{}\"\n\n",
                pkg.name.replace('\\', "\\\\").replace('"', "\\\""),
                pkg.version.replace('\\', "\\\\").replace('"', "\\\""),
                pkg.repository.replace('\\', "\\\\").replace('"', "\\\""),
            ));
        }
        out
    }
}
