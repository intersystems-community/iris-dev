//! IRIS connection types and Atelier REST API fingerprinting.

use serde::{Deserialize, Serialize};

/// A resolved connection to a running IRIS instance via Atelier REST API.
#[derive(Debug, Clone)]
pub struct IrisConnection {
    /// Base URL e.g. "http://localhost:52773"
    pub base_url: String,
    pub namespace: String,
    pub username: String,
    pub password: String,
    pub version: Option<String>,
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
            source,
            port_superserver: None,
        }
    }

    /// Build the full Atelier REST URL for a given path suffix.
    /// e.g. atelier_url("/v1/USER/action/query") → "http://localhost:52773/api/atelier/v1/USER/action/query"
    pub fn atelier_url(&self, path: &str) -> String {
        format!("{}/api/atelier{}", self.base_url.trim_end_matches('/'), path)
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
