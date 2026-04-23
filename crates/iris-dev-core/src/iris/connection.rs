//! IRIS connection types and Atelier REST API fingerprinting.

// (serde imports removed — no types in this module derive Serialize/Deserialize)

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
        format!(
            "{}/api/atelier{}",
            self.base_url.trim_end_matches('/'),
            path
        )
    }

    /// Build a versioned Atelier URL using the connection's detected API version.
    pub fn atelier_url_versioned(&self, path: &str) -> String {
        let v = self.atelier_version.version_str();
        self.atelier_url(&format!("/{}/{}{}", v, self.namespace, path))
    }

    /// Probe this connection: fetch IRIS version and Atelier API level from `/api/atelier/`.
    pub async fn probe(&mut self) {
        let client = match Self::http_client() {
            Ok(c) => c,
            Err(_) => return,
        };

        let url = self.atelier_url("/");
        if let Ok(resp) = client
            .get(&url)
            .basic_auth(&self.username, Some(&self.password))
            .send()
            .await
        {
            let status = resp.status();
            if status.is_success() {
                if let Ok(body) = resp.json::<serde_json::Value>().await {
                    tracing::debug!("Atelier root response: {}", body);
                    let content = &body["result"]["content"];
                    self.version = content["version"].as_str().map(|v| v.to_string());
                    self.atelier_version = match content["api"].as_u64() {
                        Some(v) if v >= 8 => AtelierVersion::V8,
                        Some(v) if v >= 2 => AtelierVersion::V2,
                        _ => AtelierVersion::V1,
                    };
                }
            } else {
                tracing::debug!("Atelier root probe got HTTP {}", status);
            }
        }
    }

    /// Detect the highest available Atelier API version by probing.
    /// Sets self.atelier_version in place.
    pub async fn detect_version(&mut self, client: &reqwest::Client) {
        // Try v8 first
        let v8_url = self.atelier_url(&format!("/v8/{}/", self.namespace));
        if let Ok(resp) = client
            .get(&v8_url)
            .basic_auth(&self.username, Some(&self.password))
            .send()
            .await
        {
            if resp.status().is_success() {
                self.atelier_version = AtelierVersion::V8;
                return;
            }
        }
        // Try v2
        let v2_url = self.atelier_url(&format!("/v2/{}/", self.namespace));
        if let Ok(resp) = client
            .get(&v2_url)
            .basic_auth(&self.username, Some(&self.password))
            .send()
            .await
        {
            if resp.status().is_success() {
                self.atelier_version = AtelierVersion::V2;
                return;
            }
        }
        // Fall back to v1
        self.atelier_version = AtelierVersion::V1;
    }

    /// Execute ObjectScript code via docker exec (requires IRIS_CONTAINER env var).
    /// Returns stdout from the IRIS session. No Python required — pure Rust via tokio::process.
    pub async fn execute(&self, code: &str, namespace: &str) -> anyhow::Result<String> {
        let container =
            std::env::var("IRIS_CONTAINER").map_err(|_| anyhow::anyhow!("DOCKER_REQUIRED"))?;

        use tokio::io::AsyncWriteExt;

        let mut child = tokio::process::Command::new("docker")
            .args([
                "exec", "-i", &container, "iris", "session", "IRIS", "-U", namespace,
            ])
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .spawn()
            .map_err(|e| anyhow::anyhow!("docker not available: {e}"))?;

        if let Some(mut stdin) = child.stdin.take() {
            let _ = stdin.write_all(code.as_bytes()).await;
            let _ = stdin.write_all(b"\nhalt\n").await;
        }

        let output =
            tokio::time::timeout(std::time::Duration::from_secs(30), child.wait_with_output())
                .await
                .map_err(|_| anyhow::anyhow!("docker exec timed out after 30s"))??;

        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    /// Run a SQL query via the Atelier query endpoint. Returns the response body.
    pub async fn query(
        &self,
        sql: &str,
        params: Vec<serde_json::Value>,
        client: &reqwest::Client,
    ) -> anyhow::Result<serde_json::Value> {
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
