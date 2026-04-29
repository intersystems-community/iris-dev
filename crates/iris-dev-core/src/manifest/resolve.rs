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
                        ResolvedSource::Local(path) => path.to_string_lossy().into_owned(),
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
    // Sync wrapper — spins up a tokio runtime for the async GitHub fetch.
    // Called from Resolve::from_manifest which is sync.
    match source {
        ResolvedSource::GitHub { .. } => {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()?;
            rt.block_on(resolve_github_version_async(req, source))
        }
        ResolvedSource::Local(path) => {
            // Read version from a local iris-dev.toml or Cargo.toml
            let manifest_path = path.join("iris-dev.toml");
            if manifest_path.exists() {
                let content = std::fs::read_to_string(&manifest_path)?;
                let parsed: toml::Value = toml::from_str(&content)?;
                let v_str = parsed
                    .get("package")
                    .and_then(|p| p.get("version"))
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow!("no [package].version in {:?}", manifest_path))?;
                let v = Version::parse(v_str)?;
                if req.matches(&v) {
                    return Ok(v);
                }
                anyhow::bail!("local version {} does not satisfy {}", v, req);
            }
            anyhow::bail!("local source {:?} has no iris-dev.toml", path)
        }
        _ => anyhow::bail!(
            "version resolution not yet implemented for source {:?} (requirement: {})",
            source,
            req
        ),
    }
}

/// Fetch GitHub tags and return the highest version satisfying `req`.
/// Exported for use in async tests.
pub async fn resolve_github_version_async(
    req: &VersionReq,
    source: &ResolvedSource,
) -> Result<Version> {
    let (owner, repo) = match source {
        ResolvedSource::GitHub { owner, repo } => (owner.as_str(), repo.as_str()),
        _ => anyhow::bail!("resolve_github_version_async called with non-GitHub source"),
    };

    let url = format!("https://api.github.com/repos/{}/{}/tags?per_page=100", owner, repo);
    let client = reqwest::Client::builder()
        .user_agent("iris-dev/resolver")
        .build()?;

    let resp = client.get(&url).send().await?;
    if resp.status() == reqwest::StatusCode::NOT_FOUND {
        anyhow::bail!("GitHub repo {}/{} not found", owner, repo);
    }
    if !resp.status().is_success() {
        anyhow::bail!(
            "GitHub API returned {} for {}/{}",
            resp.status(),
            owner,
            repo
        );
    }

    let tags: serde_json::Value = resp.json().await?;
    let tag_array = tags.as_array().ok_or_else(|| anyhow!("unexpected GitHub tags response"))?;

    let mut candidates: Vec<Version> = tag_array
        .iter()
        .filter_map(|t| t.get("name").and_then(|n| n.as_str()))
        .filter_map(|name| {
            // Accept "v1.2.3" and "1.2.3" tag formats
            let stripped = name.strip_prefix('v').unwrap_or(name);
            Version::parse(stripped).ok()
        })
        .filter(|v| req.matches(v))
        .collect();

    if candidates.is_empty() {
        anyhow::bail!(
            "no tags in {}/{} satisfy version requirement {}",
            owner,
            repo,
            req
        );
    }

    candidates.sort();
    Ok(candidates.into_iter().last().unwrap())
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
