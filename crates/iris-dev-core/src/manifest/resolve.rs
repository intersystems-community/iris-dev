// Semver dependency resolver.
// Reference: //Users/gangwang/ipm/src/resolve/resolve.rs (Gang Wang, ISC, 2019)
// Reimplemented in Rust 2021 with modern error handling.
// TODO: implement full graph-based semver resolution

use anyhow::Result;
use crate::manifest::schema::Manifest;

pub struct Resolve {
    // TODO: implement dependency graph (see gangwang/ipm resolve.rs for algorithm)
}

impl Resolve {
    pub fn from_manifest(_manifest: &Manifest) -> Result<Self> {
        // TODO: resolve transitive deps, check semver compatibility
        Ok(Self {})
    }
}
