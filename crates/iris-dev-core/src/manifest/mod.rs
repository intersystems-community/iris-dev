mod schema;
mod resolve;

pub use schema::{Manifest, PackageInfo, Provides, DependencySpec};
pub use resolve::Resolve;

use anyhow::{Context, Result};
use std::path::Path;

pub fn parse_manifest(path: impl AsRef<Path>) -> Result<Manifest> {
    let content = std::fs::read_to_string(path.as_ref())
        .with_context(|| format!("reading {}", path.as_ref().display()))?;
    toml::from_str(&content)
        .with_context(|| format!("parsing {}", path.as_ref().display()))
}
