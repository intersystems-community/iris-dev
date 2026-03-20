use serde::{Deserialize, Serialize};

/// A resolved connection to a running IRIS instance via Atelier REST API.
#[derive(Debug, Clone)]
pub struct IrisConnection {
    /// Base URL e.g. http://localhost:52773
    pub base_url: String,
    pub namespace: String,
    pub username: String,
    pub password: String,
    /// IRIS version string from /api/atelier/ if discovered
    pub version: Option<String>,
    pub source: DiscoverySource,
}

#[derive(Debug, Clone)]
pub enum DiscoverySource {
    LocalhostScan { port: u16 },
    Docker { container_name: String },
    VsCodeSettings,
    EnvVar,
    ExplicitFlag,
}

impl IrisConnection {
    pub fn new(base_url: impl Into<String>, namespace: impl Into<String>,
               username: impl Into<String>, password: impl Into<String>,
               source: DiscoverySource) -> Self {
        Self {
            base_url: base_url.into(),
            namespace: namespace.into(),
            username: username.into(),
            password: password.into(),
            version: None,
            source,
        }
    }

    pub fn atelier_url(&self, path: &str) -> String {
        format!("{}/api/atelier{}", self.base_url, path)
    }
}
