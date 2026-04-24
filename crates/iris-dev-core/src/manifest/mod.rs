pub mod resolve;
mod schema;

pub use resolve::Resolve;
pub use schema::{DependencySpec, Manifest, PackageInfo, Provides};

use anyhow::{Context, Result};
use std::path::Path;

pub fn parse_manifest(path: impl AsRef<Path>) -> Result<Manifest> {
    let content = std::fs::read_to_string(path.as_ref())
        .with_context(|| format!("reading {}", path.as_ref().display()))?;
    toml::from_str(&content).with_context(|| format!("parsing {}", path.as_ref().display()))
}
