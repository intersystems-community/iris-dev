//! IRIS connection types and Atelier REST API fingerprinting.

use serde::{Deserialize, Serialize};

/// Which version of the Atelier REST API to use.
#[derive(Debug, Clone, PartialEq)]
pub enum AtelierVersion {
    V8,
    V2,
    V1,
}

impl AtelierVersion {
    pub fn version_str(&self) -> &'static str {
        match self {
            AtelierVersion::V8 => "v8",
            AtelierVersion::V2 => "v2",
            AtelierVersion::V1 => "v1",
        }
    }
}

/// A resolved connection to a running IRIS instance via Atelier REST API.
#[derive(Debug, Clone)]
pub struct IrisConnection {
    /// Base URL e.g. "http://localhost:52773" or "http://localhost:80/prefix"
    pub base_url: String,
    pub namespace: String,
    pub username: String,
    pub password: String,
    pub version: Option<String>,
    pub atelier_version: AtelierVersion,
    pub source: DiscoverySource,
    pub port_superserver: Option<u16>,
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
    pub fn new(
        base_url: impl Into<String>,
        namespace: impl Into<String>,
        username: impl Into<String>,
        password: impl Into<String>,
        source: DiscoverySource,
    ) -> Self {
        Self {
            base_url: base_url.into(),
            namespace: namespace.into(),
            username: username.into(),
            password: password.into(),
            version: None,
            atelier_version: AtelierVersion::V1,
            source,
            port_superserver: None,
        }
    }

    /// Build the full Atelier REST URL for a given path suffix.
    /// Handles optional path prefix already baked into base_url.
    /// e.g. atelier_url("/v8/USER/action/compile") → "http://host:port[/prefix]/api/atelier/v8/USER/action/compile"
    pub fn atelier_url(&self, path: &str) -> String {
        format!("{}/api/atelier{}", self.base_url.trim_end_matches('/'), path)
    }

    /// Build a versioned Atelier URL using the connection's detected API version.
    pub fn atelier_url_versioned(&self, path: &str) -> String {
        let v = self.atelier_version.version_str();
        self.atelier_url(&format!("/{}/{}{}", v, self.namespace, path))
    }

    /// Detect the highest available Atelier API version by probing.
    /// Sets self.atelier_version in place.
    pub async fn detect_version(&mut self, client: &reqwest::Client) {
        // Try v8 first
        let v8_url = self.atelier_url(&format!("/v8/{}/", self.namespace));
        if let Ok(resp) = client.get(&v8_url)
            .basic_auth(&self.username, Some(&self.password))
            .send().await
        {
            if resp.status().is_success() {
                self.atelier_version = AtelierVersion::V8;
                return;
            }
        }
        // Try v2
        let v2_url = self.atelier_url(&format!("/v2/{}/", self.namespace));
        if let Ok(resp) = client.get(&v2_url)
            .basic_auth(&self.username, Some(&self.password))
            .send().await
        {
            if resp.status().is_success() {
                self.atelier_version = AtelierVersion::V2;
                return;
            }
        }
        // Fall back to v1
        self.atelier_version = AtelierVersion::V1;
    }

    /// Execute ObjectScript code via xecute endpoint. Returns the response body.
    pub async fn xecute(&self, code: &str, client: &reqwest::Client) -> anyhow::Result<serde_json::Value> {
        let url = self.atelier_url(&format!("/v1/{}/action/xecute", self.namespace));
        let resp = client
            .post(&url)
            .basic_auth(&self.username, Some(&self.password))
            .json(&serde_json::json!({"expression": code}))
            .send()
            .await?;
        Ok(resp.json().await?)
    }

    /// Run a SQL query via the Atelier query endpoint. Returns the response body.
    pub async fn query(&self, sql: &str, params: Vec<serde_json::Value>, client: &reqwest::Client) -> anyhow::Result<serde_json::Value> {
        let url = self.atelier_url(&format!("/v1/{}/action/query", self.namespace));
        let resp = client
            .post(&url)
            .basic_auth(&self.username, Some(&self.password))
            .json(&serde_json::json!({"query": sql, "parameters": params}))
            .send()
            .await?;
        Ok(resp.json().await?)
    }

    /// Build a reqwest Client suitable for Atelier REST calls.
    pub fn http_client() -> anyhow::Result<reqwest::Client> {
        Ok(reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .danger_accept_invalid_certs(true)
            .build()?)
    }
}
