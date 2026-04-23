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

    /// Execute ObjectScript code via the write-compile-query cycle (pure HTTP, no docker).
    ///
    /// Writes a temp class whose `CodeMode = objectgenerator` method runs the user's code at
    /// compile time, captures its `Write` output to a temp file, and embeds the result as a
    /// static return value in the generated method. The method is called via SQL to retrieve
    /// the output, then the temp class is deleted.
    pub async fn execute_via_generator(
        &self,
        code: &str,
        namespace: &str,
        client: &reqwest::Client,
    ) -> anyhow::Result<String> {
        let id: String = uuid::Uuid::new_v4()
            .simple()
            .to_string()
            .chars()
            .take(12)
            .collect();
        let class_name = format!("User.IrisDevRun{}", id);
        let doc_name = format!("{}.cls", class_name);
        // IRIS SQL stored procedure: schema "User", proc name "IrisDevRun{id}_Execute"
        let sql_func = format!("User.IrisDevRun{}_Execute", id);
        let tmpfile = format!("/tmp/irisd_{}.txt", id);

        let content = Self::build_exec_class(&class_name, &tmpfile, code);

        // 1. PUT the class document
        let put_url = self.atelier_url(&format!(
            "/v1/{}/doc/{}",
            namespace,
            urlencoding::encode(&doc_name)
        ));
        let put_resp = client
            .put(&put_url)
            .basic_auth(&self.username, Some(&self.password))
            .json(&serde_json::json!({"content": content}))
            .send()
            .await?;
        if !put_resp.status().is_success() {
            anyhow::bail!("PUT doc failed: HTTP {}", put_resp.status());
        }

        // 2. Compile — triggers the generator, which runs user code and captures output
        let compile_url =
            self.atelier_url(&format!("/v1/{}/action/compile?flags=cuk", namespace));
        let compile_resp = client
            .post(&compile_url)
            .basic_auth(&self.username, Some(&self.password))
            .json(&serde_json::json!([doc_name]))
            .send()
            .await?;
        if !compile_resp.status().is_success() {
            let _ = self.delete_doc(&doc_name, namespace, client).await;
            anyhow::bail!("compile HTTP {}", compile_resp.status());
        }
        let compile_body: serde_json::Value = compile_resp.json().await.unwrap_or_default();
        let has_errors = compile_body["result"]["log"]
            .as_array()
            .map(|entries| {
                entries.iter().any(|e| {
                    e["type"]
                        .as_str()
                        .map(|t| t.eq_ignore_ascii_case("error"))
                        .unwrap_or(false)
                })
            })
            .unwrap_or(false);
        if has_errors {
            let _ = self.delete_doc(&doc_name, namespace, client).await;
            anyhow::bail!("compile errors: {:?}", compile_body["result"]["log"]);
        }

        // 3. Query via SQL — the generated method returns the captured output as a literal string
        let sql = format!("SELECT {}() AS output", sql_func);
        let query_url = self.atelier_url(&format!("/v1/{}/action/query", namespace));
        let query_resp = client
            .post(&query_url)
            .basic_auth(&self.username, Some(&self.password))
            .json(&serde_json::json!({"query": sql}))
            .send()
            .await?;
        let query_body: serde_json::Value = query_resp.json().await.unwrap_or_default();
        let output = query_body["result"]["content"][0]["output"]
            .as_str()
            .unwrap_or("")
            .to_string();

        // 4. Delete the temp class (best-effort)
        let _ = self.delete_doc(&doc_name, namespace, client).await;

        Ok(output)
    }

    /// Build the `.cls` source lines for the temp executor class.
    ///
    /// The generated class has a `CodeMode = objectgenerator` method whose body runs at
    /// compile time: it redirects Write output to a temp file, executes user code inside a
    /// Try/Catch, reads back the captured output, then writes `Quit "output"` as the sole
    /// line of the generated method body. SQL call retrieves that literal string.
    fn build_exec_class(class_name: &str, tmpfile: &str, code: &str) -> Vec<String> {
        let mut lines: Vec<String> = vec![
            format!("Class {} [ Final ]", class_name),
            "{".into(),
            "".into(),
            "ClassMethod Execute() As %String [ CodeMode = objectgenerator, SqlProc ]".into(),
            "{".into(),
            format!("  Set tmpfile = \"{}\"", tmpfile),
            "  Set savedIO = $IO".into(),
            "  Open tmpfile:(\"WNS\"):5".into(),
            // ObjectScript: "" inside a string = one literal " — so ""ERROR..."" = "ERROR..."
            "  If '$TEST { Do %code.WriteLine(\" Quit \"\"ERROR: output capture unavailable\"\"\") Quit }".into(),
            "  Use tmpfile".into(),
            "  Try {".into(),
        ];
        for line in code.lines() {
            lines.push(format!("    {}", line));
        }
        lines.extend([
            "  } Catch ex {".into(),
            "    Write \"ERROR: \",ex.DisplayString(),!".into(),
            "  }".into(),
            "  Close tmpfile".into(),
            "  Use savedIO".into(),
            "  Set out = \"\"".into(),
            "  Open tmpfile:(\"RNS\"):1".into(),
            "  If $TEST {".into(),
            "    Set line = \"\"".into(),
            // Read line-by-line until EOF ($TEST=0 on timeout with :0)
            "    For  { Read line:0  If '$TEST Quit  Set out = out_line_$Char(10) }".into(),
            "    Close tmpfile".into(),
            "  }".into(),
            "  Do ##class(%Library.File).Delete(tmpfile)".into(),
            "  Set qout = $Replace(out,$Char(34),$Char(34)_$Char(34))".into(),
            // Generate: Quit "captured_output" — $Char(34) avoids nested quote escaping
            "  Do %code.WriteLine(\" Quit \"_$Char(34)_qout_$Char(34))".into(),
            "}".into(),
            "".into(),
            "}".into(),
        ]);
        lines
    }

    /// Delete an Atelier document (best-effort).
    async fn delete_doc(
        &self,
        doc_name: &str,
        namespace: &str,
        client: &reqwest::Client,
    ) -> anyhow::Result<()> {
        let url = self.atelier_url(&format!(
            "/v1/{}/doc/{}",
            namespace,
            urlencoding::encode(doc_name)
        ));
        client
            .delete(&url)
            .basic_auth(&self.username, Some(&self.password))
            .send()
            .await?;
        Ok(())
    }

    /// Execute ObjectScript code via docker exec (requires IRIS_CONTAINER env var).
    /// Returns stdout from the IRIS session. No Python required — pure Rust via tokio::process.
    pub async fn execute(&self, code: &str, namespace: &str) -> anyhow::Result<String> {
        let container = std::env::var("IRIS_CONTAINER")
            .map_err(|_| anyhow::anyhow!("DOCKER_REQUIRED"))?;

        use tokio::io::AsyncWriteExt;

        let mut child = tokio::process::Command::new("docker")
            .args(["exec", "-i", &container, "iris", "session", "IRIS", "-U", namespace])
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .spawn()
            .map_err(|e| anyhow::anyhow!("docker not available: {e}"))?;

        if let Some(mut stdin) = child.stdin.take() {
            let _ = stdin.write_all(code.as_bytes()).await;
            let _ = stdin.write_all(b"\nhalt\n").await;
        }

        let output = tokio::time::timeout(
            std::time::Duration::from_secs(30),
            child.wait_with_output(),
        )
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
