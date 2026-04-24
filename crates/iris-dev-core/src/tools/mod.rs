use crate::elicitation::ElicitationStore;
use crate::iris::connection::IrisConnection;
use rmcp::{
    handler::server::router::tool::ToolRouter, handler::server::wrapper::Parameters, model::*,
    tool, tool_handler, tool_router, ErrorData as McpError, ServerHandler,
};
use schemars::JsonSchema;
use serde::Deserialize;
use std::collections::VecDeque;
use std::sync::Arc;
pub mod doc;
pub mod info;
pub mod interop;
pub mod scm;
pub mod search;
pub mod skills_tools;

pub use doc::{DocMode, IrisDocParams};
pub use scm::ScmParams;

/// A single tool call entry for the session history ring buffer.
#[derive(Debug, Clone)]
pub struct ToolCallEntry {
    pub tool: String,
    pub success: bool,
    pub timestamp: std::time::Instant,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct CompileParams {
    pub target: String,
    #[serde(default = "default_flags")]
    pub flags: String,
    #[serde(default = "default_namespace")]
    pub namespace: String,
    #[serde(default)]
    pub force_writable: bool,
}
#[derive(Debug, Deserialize, JsonSchema)]
pub struct TestParams {
    pub pattern: String,
    #[serde(default = "default_namespace")]
    pub namespace: String,
}
#[derive(Debug, Deserialize, JsonSchema)]
pub struct SymbolsParams {
    pub query: String,
    #[serde(default = "default_limit")]
    pub limit: usize,
    #[serde(default = "default_namespace")]
    pub namespace: String,
}
#[derive(Debug, Deserialize, JsonSchema)]
pub struct IntrospectParams {
    pub class_name: String,
    #[serde(default = "default_namespace")]
    pub namespace: String,
}
#[derive(Debug, Deserialize, JsonSchema)]
pub struct DebugMapParams {
    #[serde(default)]
    pub routine: String,
    #[serde(default)]
    pub offset: i64,
    #[serde(default)]
    pub error_string: String,
    #[serde(default = "default_namespace")]
    pub namespace: String,
}
#[derive(Debug, Deserialize, JsonSchema)]
pub struct GenerateClassParams {
    pub description: String,
    #[serde(default)]
    pub overwrite: bool,
    #[serde(default = "default_namespace")]
    pub namespace: String,
}
#[derive(Debug, Deserialize, JsonSchema)]
pub struct GenerateTestParams {
    pub class_name: String,
    #[serde(default = "default_namespace")]
    pub namespace: String,
}
#[derive(Debug, Deserialize, JsonSchema)]
pub struct SkillNameParams {
    pub name: String,
}
#[derive(Debug, Deserialize, JsonSchema)]
pub struct SkillSearchParams {
    pub query: String,
    #[serde(default = "default_limit")]
    pub top_k: usize,
}
#[derive(Debug, Deserialize, JsonSchema)]
pub struct KbIndexParams {
    pub workspace_path: Option<String>,
}
#[derive(Debug, Deserialize, JsonSchema)]
pub struct KbRecallParams {
    pub query: String,
    #[serde(default = "default_limit")]
    pub top_k: usize,
}
#[derive(Debug, Deserialize, JsonSchema)]
pub struct AgentHistoryParams {
    #[serde(default = "default_limit")]
    pub limit: usize,
}
#[derive(Debug, Deserialize, JsonSchema)]
pub struct SymbolsLocalParams {
    pub workspace_path: Option<String>,
}
#[derive(Debug, Deserialize, JsonSchema)]
pub struct CapturePacketParams {
    #[serde(default = "default_namespace")]
    pub namespace: String,
}
#[derive(Debug, Deserialize, JsonSchema)]
pub struct ErrorLogsParams {
    #[serde(default = "default_namespace")]
    pub namespace: String,
    #[serde(default = "default_max_entries")]
    pub max_entries: usize,
}
#[derive(Debug, Deserialize, JsonSchema)]
pub struct CommunityPkgParams {
    pub name: String,
}
#[derive(Debug, Deserialize, JsonSchema)]
pub struct NoParams {}
#[derive(Debug, Deserialize, JsonSchema)]
pub struct SourceMapParams {
    pub cls_text: String,
    pub cls_name: String,
    pub workspace_path: Option<String>,
    #[serde(default = "default_namespace")]
    pub namespace: String,
}
#[derive(Debug, Deserialize, JsonSchema)]
pub struct ExecuteParams {
    pub code: String,
    #[serde(default = "default_namespace")]
    pub namespace: String,
    #[serde(default = "default_execute_timeout")]
    pub timeout: u64,
    #[serde(default)]
    pub confirmed: bool,
}
#[derive(Debug, Deserialize, JsonSchema)]
pub struct QueryParams {
    pub query: String,
    /// Query parameters as strings (e.g. ["Alice", "42"])
    #[serde(default)]
    pub parameters: Vec<String>,
    #[serde(default = "default_namespace")]
    pub namespace: String,
}
#[derive(Debug, Deserialize, JsonSchema)]
pub struct ListContainersParams {
    pub workspace_root: Option<String>,
}
#[derive(Debug, Deserialize, JsonSchema)]
pub struct SelectContainerParams {
    pub name: String,
    #[serde(default = "default_namespace")]
    pub namespace: String,
    #[serde(default = "default_username")]
    pub username: String,
    #[serde(default = "default_password")]
    pub password: String,
}
#[derive(Debug, Deserialize, JsonSchema)]
pub struct StartSandboxParams {
    #[serde(default)]
    pub name: String,
    #[serde(default = "default_edition")]
    pub edition: String,
}

fn default_flags() -> String {
    "cuk".to_string()
}
fn default_namespace() -> String {
    "USER".to_string()
}
fn default_limit() -> usize {
    20
}
fn default_max_entries() -> usize {
    50
}
fn default_execute_timeout() -> u64 {
    30
}
fn default_username() -> String {
    "_SYSTEM".to_string()
}
fn default_password() -> String {
    "SYS".to_string()
}
fn default_edition() -> String {
    "community".to_string()
}

fn iris_unreachable() -> McpError {
    McpError::invalid_request("IRIS_UNREACHABLE: no IRIS connection. Set IRIS_HOST and IRIS_WEB_PORT env vars, or ensure IRIS is reachable on a discoverable port (52773, 41773, 51773, 8080).", None)
}
fn ok_json(v: serde_json::Value) -> Result<CallToolResult, McpError> {
    Ok(CallToolResult::success(vec![Content::text(v.to_string())]))
}
fn err_json(code: &str, msg: &str) -> Result<CallToolResult, McpError> {
    ok_json(serde_json::json!({"success": false, "error_code": code, "error": msg}))
}
pub fn write_open_hint(namespace: &str, document: &str) {
    if let Some(home) = dirs::home_dir() {
        let dir = home.join(".iris-dev");
        let _ = std::fs::create_dir_all(&dir);
        let uri = format!("isfs://{}/{}", namespace, document);
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0);
        let json = serde_json::json!({"uri": uri, "ts": ts});
        let _ = std::fs::write(dir.join("open-hint.json"), json.to_string());
    }
}

fn err_json_with_url(
    code: &str,
    msg: &str,
    attempted_url: &str,
) -> Result<CallToolResult, McpError> {
    ok_json(serde_json::json!({
        "success": false,
        "error_code": code,
        "error": msg,
        "attempted_url": attempted_url,
        "hint": "Check IRIS_HOST and IRIS_WEB_PORT (and IRIS_WEB_PREFIX if using a non-root gateway)"
    }))
}
// Bug 20: delegate to the canonical implementation in iris::discovery instead of duplicating.
fn score_container(name: &str, workspace_basename: &str) -> i64 {
    crate::iris::discovery::score_container_name(name, workspace_basename) as i64
}

fn extract_port(ports: &str, container_port: &str) -> Option<u16> {
    let pat = format!("(\\d+)->{}", regex::escape(container_port));
    regex::Regex::new(&pat)
        .ok()?
        .captures(ports)
        .and_then(|c| c[1].parse().ok())
}

async fn list_iris_containers(workspace_basename: &str) -> Vec<serde_json::Value> {
    let mut containers: Vec<serde_json::Value> = Vec::new();

    if let Ok(out) = tokio::process::Command::new("idt")
        .args(["container", "list", "--format", "json"])
        .output()
        .await
    {
        if out.status.success() {
            if let Ok(items) = serde_json::from_slice::<Vec<serde_json::Value>>(&out.stdout) {
                for item in items {
                    let name = item["name"].as_str().unwrap_or("").to_string();
                    let ports = item["ports"].as_str().unwrap_or("");
                    let sp = extract_port(ports, "1972")
                        .map(|p| serde_json::json!(p))
                        .unwrap_or(serde_json::Value::Null);
                    let wp = extract_port(ports, "52773")
                        .map(|p| serde_json::json!(p))
                        .unwrap_or(serde_json::Value::Null);
                    let score = score_container(&name, workspace_basename);
                    containers.push(serde_json::json!({
                        "name": name, "port_superserver": sp, "port_web": wp,
                        "image": item["image"], "status": item.get("status").unwrap_or(&serde_json::json!("running")),
                        "age": item.get("age").unwrap_or(&serde_json::json!("")), "score": score,
                    }));
                }
                return sort_containers(containers);
            }
        }
    }

    if let Ok(out) = tokio::process::Command::new("docker")
        .args([
            "ps",
            "--format",
            "{{.Names}}\t{{.Image}}\t{{.Ports}}\t{{.Status}}\t{{.RunningFor}}",
        ])
        .output()
        .await
    {
        if out.status.success() {
            for line in String::from_utf8_lossy(&out.stdout).lines() {
                let parts: Vec<&str> = line.splitn(5, '\t').collect();
                if parts.len() < 5 {
                    continue;
                }
                let (name, image, ports_raw, age) = (parts[0], parts[1], parts[2], parts[4]);
                if !image.to_lowercase().contains("intersystems")
                    && !image.to_lowercase().contains("iris")
                {
                    continue;
                }
                let sp = extract_port(ports_raw, "1972")
                    .map(|p| serde_json::json!(p))
                    .unwrap_or(serde_json::Value::Null);
                let wp = extract_port(ports_raw, "52773")
                    .map(|p| serde_json::json!(p))
                    .unwrap_or(serde_json::Value::Null);
                let score = score_container(name, workspace_basename);
                containers.push(serde_json::json!({
                    "name": name, "port_superserver": sp, "port_web": wp,
                    "image": image, "status": "running", "age": age, "score": score,
                }));
            }
        }
    }
    sort_containers(containers)
}

fn sort_containers(mut v: Vec<serde_json::Value>) -> Vec<serde_json::Value> {
    v.sort_by(|a, b| {
        let sa = a["score"].as_i64().unwrap_or(0);
        let sb = b["score"].as_i64().unwrap_or(0);
        sb.cmp(&sa).then_with(|| {
            a["name"]
                .as_str()
                .unwrap_or("")
                .cmp(b["name"].as_str().unwrap_or(""))
        })
    });
    v
}

#[derive(Clone)]
pub struct IrisTools {
    pub iris: Option<Arc<IrisConnection>>,
    pub registry: Arc<crate::skills::SkillRegistry>,
    /// Shared HTTP client — created once, reused across all tool calls.
    pub client: Arc<reqwest::Client>,
    /// Ring buffer of recent tool calls for skill_propose pattern mining.
    pub history: Arc<std::sync::Mutex<VecDeque<ToolCallEntry>>>,
    /// Pending elicitation state for SCM dialogs.
    pub elicitation_store: Arc<ElicitationStore>,
    tool_router: ToolRouter<IrisTools>,
}

#[tool_router]
impl IrisTools {
    pub fn new(iris: Option<IrisConnection>) -> anyhow::Result<Self> {
        let client = Arc::new(IrisConnection::http_client()?);
        Ok(Self {
            iris: iris.map(Arc::new),
            registry: Arc::new(crate::skills::SkillRegistry::new()),
            client,
            history: Arc::new(std::sync::Mutex::new(VecDeque::with_capacity(50))),
            elicitation_store: Arc::new(ElicitationStore::new()),
            tool_router: Self::tool_router(),
        })
    }
    pub fn with_registry(
        iris: Option<IrisConnection>,
        registry: crate::skills::SkillRegistry,
    ) -> anyhow::Result<Self> {
        let client = Arc::new(IrisConnection::http_client()?);
        Ok(Self {
            iris: iris.map(Arc::new),
            registry: Arc::new(registry),
            client,
            history: Arc::new(std::sync::Mutex::new(VecDeque::with_capacity(50))),
            elicitation_store: Arc::new(ElicitationStore::new()),
            tool_router: Self::tool_router(),
        })
    }
    fn get_iris(&self) -> Result<&IrisConnection, McpError> {
        self.iris.as_deref().ok_or_else(iris_unreachable)
    }
    fn http_client(&self) -> &reqwest::Client {
        &self.client
    }
    fn record_call(&self, tool: &str, success: bool) {
        if let Ok(mut h) = self.history.lock() {
            if h.len() == 50 {
                h.pop_front();
            }
            h.push_back(ToolCallEntry {
                tool: tool.to_string(),
                success,
                timestamp: std::time::Instant::now(),
            });
        }
    }

    #[tool(
        description = "Compile an ObjectScript class, routine, or wildcard package on IRIS via Atelier REST. Supports 'MyApp.*.cls' for package-level compilation. Returns structured errors with line numbers, columns, and severity. No Python required."
    )]
    async fn iris_compile(
        &self,
        Parameters(p): Parameters<CompileParams>,
    ) -> Result<CallToolResult, McpError> {
        let iris = self.get_iris()?;
        tracing::info!(namespace = %p.namespace, target = %p.target, "iris_compile");
        let client = self.http_client();

        // Expand wildcards: resolve "MyApp.*.cls" to a list of matching class names.
        // Bug 8: use p.namespace (not iris.namespace) and the correct /docnames/CLS endpoint.
        let targets: Vec<String> = if p.target.contains('*') {
            let list_url = iris.versioned_ns_url(&p.namespace, "/docnames/CLS");
            match client
                .get(&list_url)
                .basic_auth(&iris.username, Some(&iris.password))
                .send()
                .await
            {
                Ok(resp) if resp.status().is_success() => {
                    let body: serde_json::Value = resp.json().await.unwrap_or_default();
                    let pattern = p.target.replace('.', "\\.").replace('*', ".*");
                    let re = regex::Regex::new(&format!("(?i)^{}$", pattern))
                        .unwrap_or_else(|_| regex::Regex::new(".*").unwrap());
                    // /docnames/ returns an array of strings, not objects with a "name" key.
                    body["result"]["content"]
                        .as_array()
                        .unwrap_or(&vec![])
                        .iter()
                        .filter_map(|d| d.as_str())
                        .filter(|n| re.is_match(n))
                        .map(|n| n.to_string())
                        .collect()
                }
                _ => vec![p.target.clone()],
            }
        } else {
            vec![p.target.clone()]
        };

        if targets.is_empty() {
            return err_json(
                "NOT_FOUND",
                &format!("No documents match pattern: {}", p.target),
            );
        }

        // force_writable: attempt to enable namespace via docker exec if available
        if p.force_writable {
            let code = format!(
                "do ##class(%Library.EnsembleMgr).EnableNamespace(\"{}\",1)",
                p.namespace
            );
            let _ = iris.execute(&code, &p.namespace).await;
        }

        // Atelier compile: POST with JSON array of document names (with extensions)
        // e.g. ["MyApp.Patient.cls", "MyApp.Utils.cls"]
        let compile_url = iris.versioned_ns_url(
            &p.namespace,
            &format!("/action/compile?flags={}", urlencoding::encode(&p.flags)),
        );

        // Ensure targets have extensions.
        // Bug 16: the old check `t.contains('.')` skipped top-level classes (no package dot).
        // Correct check: append .cls only when no known extension is already present.
        let targets_with_ext: Vec<String> = targets
            .iter()
            .map(|t| {
                if !t.ends_with(".cls")
                    && !t.ends_with(".mac")
                    && !t.ends_with(".inc")
                    && !t.ends_with(".int")
                {
                    format!("{}.cls", t)
                } else {
                    t.clone()
                }
            })
            .collect();

        let resp = client
            .post(&compile_url)
            .basic_auth(&iris.username, Some(&iris.password))
            .json(&targets_with_ext)
            .send()
            .await
            .map_err(|e| McpError::internal_error(format!("HTTP error: {e}"), None))?;

        // Bug 17: `&& != 200` was dead code since 200 is always is_success().
        if !resp.status().is_success() {
            let url_str = compile_url.clone();
            let status = resp.status().as_u16();
            return err_json_with_url("IRIS_UNREACHABLE", &format!("HTTP {}", status), &url_str);
        }

        let body: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| McpError::internal_error(format!("JSON parse error: {e}"), None))?;

        // Parse compiler output — console is at top level for query-param compile
        let console = body["console"]
            .as_array()
            .or_else(|| body["result"]["console"].as_array())
            .cloned()
            .unwrap_or_default();

        // Also check status.errors for any compile errors
        if let Some(status_errors) = body["status"]["errors"].as_array() {
            if !status_errors.is_empty() {
                let msg = status_errors[0]["error"]
                    .as_str()
                    .unwrap_or("Compile error");
                return err_json("COMPILE_ERROR", msg);
            }
        }
        let mut errors = vec![];
        let mut warnings = vec![];
        for line in &console {
            let text = line.as_str().unwrap_or("");
            // Atelier compile errors: "  1 ERROR #<code>:<line>: <message>"
            // Warnings: "  2 WARNING #<code>:<line>: <message>"
            if let Some(rest) = text.trim().strip_prefix("ERROR ") {
                let parts: Vec<&str> = rest.splitn(3, ':').collect();
                let (code, line_num, msg) = if parts.len() >= 3 {
                    (
                        parts[0].trim(),
                        parts[1].trim().parse::<u32>().unwrap_or(0),
                        parts[2].trim(),
                    )
                } else {
                    ("", 0, rest)
                };
                errors.push(serde_json::json!({"severity":"error","code":code,"line":line_num,"column":0,"text":msg}));
            } else if let Some(rest) = text.trim().strip_prefix("WARNING ") {
                let parts: Vec<&str> = rest.splitn(3, ':').collect();
                let (code, line_num, msg) = if parts.len() >= 3 {
                    (
                        parts[0].trim(),
                        parts[1].trim().parse::<u32>().unwrap_or(0),
                        parts[2].trim(),
                    )
                } else {
                    ("", 0, rest)
                };
                warnings.push(serde_json::json!({"severity":"warning","code":code,"line":line_num,"column":0,"text":msg}));
            }
        }

        let success = errors.is_empty();
        self.record_call("iris_compile", success);

        // Write open hint for single non-wildcard successful compile
        let open_uri = if success && !p.target.contains('*') && targets.len() == 1 {
            write_open_hint(&p.namespace, &p.target);
            Some(format!("isfs://{}/{}", p.namespace, p.target))
        } else {
            None
        };

        let mut resp = serde_json::json!({
            "success": success,
            "target": p.target,
            "targets_compiled": targets.len(),
            "namespace": p.namespace,
            "errors": errors,
            "warnings": warnings,
            "console": console,
        });
        if let Some(uri) = open_uri {
            resp["open_uri"] = serde_json::Value::String(uri);
        }
        ok_json(resp)
    }

    #[tool(
        description = "Run %UnitTest.Manager tests on IRIS via docker exec. Set IRIS_CONTAINER=<container_name> to enable. Pass a class pattern like 'MyApp.Tests' or 'MyApp.Tests.Order'. Returns structured pass/fail counts and full trace output. No Python required."
    )]
    async fn iris_test(
        &self,
        Parameters(p): Parameters<TestParams>,
    ) -> Result<CallToolResult, McpError> {
        tracing::info!(namespace = %p.namespace, pattern = %p.pattern, "iris_test");
        let iris = self.get_iris()?;
        let code = format!(
            "do ##class(%UnitTest.Manager).RunTest(\"{}\",\"/noload/run\")",
            p.pattern.replace('"', "\\\"")
        );
        match iris.execute(&code, &p.namespace).await {
            Err(e) => {
                let msg = e.to_string();
                self.record_call("iris_test", false);
                if msg == "DOCKER_REQUIRED" {
                    ok_json(serde_json::json!({
                        "success": false,
                        "error_code": "DOCKER_REQUIRED",
                        "error": "iris_test requires docker exec. Set IRIS_CONTAINER=<container_name>. The Atelier REST API has no ObjectScript execution endpoint.",
                    }))
                } else {
                    ok_json(serde_json::json!({
                        "success": false,
                        "error_code": "EXECUTION_FAILED",
                        "error": msg,
                    }))
                }
            }
            Ok(output_lines) => {
                let passed = output_lines
                    .lines()
                    .find(|l| l.to_lowercase().contains("passed:"))
                    .and_then(|l| {
                        l.split(':')
                            .nth(1)?
                            .split_whitespace()
                            .next()?
                            .parse::<u64>()
                            .ok()
                    })
                    .unwrap_or(0);
                let failed = output_lines
                    .lines()
                    .find(|l| l.to_lowercase().contains("failed:"))
                    .and_then(|l| {
                        l.split(':')
                            .nth(1)?
                            .split_whitespace()
                            .next()?
                            .parse::<u64>()
                            .ok()
                    })
                    .unwrap_or(0);
                let total = passed + failed;
                // FR-015/Mo1: distinguish "no tests found" from "test failure".
                if total == 0 {
                    self.record_call("iris_test", false);
                    return ok_json(serde_json::json!({
                        "success": false,
                        "error_code": "NO_TESTS_FOUND",
                        "error": "Pattern matched no test classes",
                        "pattern": p.pattern,
                        "namespace": p.namespace,
                        "passed": 0,
                        "failed": 0,
                        "total": 0,
                    }));
                }
                let success = failed == 0;
                self.record_call("iris_test", success);
                ok_json(serde_json::json!({
                    "success": success,
                    "pattern": p.pattern,
                    "namespace": p.namespace,
                    "passed": passed,
                    "failed": failed,
                    "total": total,
                    "output": output_lines.trim(),
                }))
            }
        }
    }

    #[tool(
        description = "Execute arbitrary ObjectScript code on IRIS and return stdout. Uses pure-HTTP execution via CodeMode=objectgenerator (write temp class, compile, query result, delete). Falls back to docker exec if IRIS_CONTAINER env var is set and HTTP fails. Example: code='write $ZVERSION,!' returns the IRIS version string."
    )]
    async fn iris_execute(
        &self,
        Parameters(p): Parameters<ExecuteParams>,
    ) -> Result<CallToolResult, McpError> {
        let iris = self.get_iris()?;
        tracing::info!(namespace = %p.namespace, "iris_execute");
        let client = self.http_client();
        let timeout = std::time::Duration::from_secs(p.timeout);

        // Try pure-HTTP execution first (write-compile-query via CodeMode=objectgenerator).
        let gen_result = tokio::time::timeout(
            timeout,
            iris.execute_via_generator(&p.code, &p.namespace, client),
        )
        .await;

        match gen_result {
            Err(_) => {
                self.record_call("iris_execute", false);
                return ok_json(serde_json::json!({
                    "success": false,
                    "error_code": "TIMEOUT",
                    "error": format!("execution timed out after {}s", p.timeout),
                }));
            }
            Ok(Ok(output)) => {
                self.record_call("iris_execute", true);
                return ok_json(serde_json::json!({
                    "success": true,
                    "output": output.trim(),
                    "namespace": p.namespace,
                    "method": "http",
                }));
            }
            Ok(Err(_)) => {
                // HTTP path failed — fall through to docker exec.
            }
        }

        // Fallback: docker exec (requires IRIS_CONTAINER env var).
        let docker_result =
            tokio::time::timeout(timeout, iris.execute(&p.code, &p.namespace)).await;
        match docker_result {
            Err(_) => {
                self.record_call("iris_execute", false);
                ok_json(serde_json::json!({
                    "success": false,
                    "error_code": "TIMEOUT",
                    "error": format!("execution timed out after {}s", p.timeout),
                }))
            }
            Ok(Err(e)) => {
                let msg = e.to_string();
                self.record_call("iris_execute", false);
                if msg == "DOCKER_REQUIRED" {
                    ok_json(serde_json::json!({
                        "success": false,
                        "error_code": "DOCKER_REQUIRED",
                        "error": "iris_execute: HTTP execution failed and IRIS_CONTAINER is not set for docker exec fallback.",
                    }))
                } else {
                    ok_json(serde_json::json!({
                        "success": false,
                        "error_code": "EXECUTION_FAILED",
                        "error": msg,
                    }))
                }
            }
            Ok(Ok(output)) => {
                self.record_call("iris_execute", true);
                ok_json(serde_json::json!({
                    "success": true,
                    "output": output.trim(),
                    "namespace": p.namespace,
                    "method": "docker",
                }))
            }
        }
    }

    #[tool(
        description = "Read, write, delete, or check an IRIS document. mode='get' fetches source, mode='put' writes (with automatic SCM checkout if needed), mode='delete' removes, mode='head' checks existence. Supports batch ops via 'names' array and elicitation_id/elicitation_answer for SCM dialog resumption. No Python required."
    )]
    async fn iris_doc(
        &self,
        Parameters(p): Parameters<IrisDocParams>,
    ) -> Result<CallToolResult, McpError> {
        let iris = self.get_iris()?;
        tracing::info!(namespace = %p.namespace, "iris_doc");
        let client = self.http_client();
        let result = doc::handle_iris_doc(iris, client, p, &self.elicitation_store).await;
        self.record_call("iris_doc", result.is_ok());
        result
    }

    #[tool(
        description = "Execute a SQL query on IRIS via Atelier REST. Returns rows as a JSON array with column names as keys. Supports SELECT, INSERT, UPDATE, DELETE. No Python required."
    )]
    async fn iris_query(
        &self,
        Parameters(p): Parameters<QueryParams>,
    ) -> Result<CallToolResult, McpError> {
        let iris = self.get_iris()?;
        tracing::info!(namespace = %p.namespace, "iris_query");
        let client = self.http_client();
        let query_url = iris.versioned_ns_url(&p.namespace, "/action/query");
        let resp = client
            .post(&query_url)
            .basic_auth(&iris.username, Some(&iris.password))
            .json(&serde_json::json!({"query": p.query, "parameters": p.parameters}))
            .send()
            .await
            .map_err(|e| McpError::internal_error(format!("HTTP error: {e}"), None))?;

        if !resp.status().is_success() {
            return err_json_with_url(
                "IRIS_UNREACHABLE",
                &format!("HTTP {}", resp.status()),
                &query_url,
            );
        }

        let body: serde_json::Value = resp.json().await.unwrap_or_default();

        if let Some(errors) = body["status"]["errors"].as_array() {
            if !errors.is_empty() {
                let msg = errors[0]["error"].as_str().unwrap_or("SQL error");
                self.record_call("iris_query", false);
                return err_json("SQL_ERROR", msg);
            }
        }

        let rows = body["result"]["content"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        let count = rows.len();
        self.record_call("iris_query", true);
        ok_json(
            serde_json::json!({"success": true, "rows": rows, "count": count, "namespace": p.namespace}),
        )
    }

    #[tool(
        description = "List running IRIS Docker containers with name-match scoring. Tries iris-devtester first, falls back to docker ps. Containers sorted by score (name similarity to workspace) descending."
    )]
    async fn iris_list_containers(
        &self,
        Parameters(p): Parameters<ListContainersParams>,
    ) -> Result<CallToolResult, McpError> {
        let workspace_basename = p
            .workspace_root
            .as_deref()
            .map(|r| {
                std::path::Path::new(r)
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("")
                    .to_string()
            })
            .unwrap_or_default();

        let containers = list_iris_containers(&workspace_basename).await;
        let suggestion = containers.first().map(|c: &serde_json::Value| {
            format!(
                "iris_select_container(name='{}')",
                c["name"].as_str().unwrap_or("")
            )
        });
        ok_json(serde_json::json!({
            "status": "ok",
            "containers": containers,
            "workspace_basename": workspace_basename,
            "suggestion": suggestion,
        }))
    }

    #[tool(
        description = "Validate and return connection parameters for the specified IRIS container. Does NOT hot-swap the active connection (restart the MCP session to switch containers). Returns probed version and port info so the caller can configure a new session."
    )]
    async fn iris_select_container(
        &self,
        Parameters(p): Parameters<SelectContainerParams>,
    ) -> Result<CallToolResult, McpError> {
        let workspace_basename = String::new();

        let containers = list_iris_containers(&workspace_basename).await;
        let found = containers
            .iter()
            .find(|c| c["name"].as_str() == Some(&p.name));

        let container = match found {
            Some(c) => c.clone(),
            None => {
                let available: Vec<_> = containers
                    .iter()
                    .filter_map(|c| c["name"].as_str())
                    .collect();
                return ok_json(serde_json::json!({
                    "error": "CONTAINER_NOT_FOUND",
                    "requested": p.name,
                    "available": available,
                }));
            }
        };

        let port_superserver = container["port_superserver"].as_u64().unwrap_or(1972) as u16;
        let port_web = container["port_web"].as_u64().unwrap_or(52773) as u16;
        let base_url = format!("http://localhost:{}", port_web);

        // Bug 5: the old code built new_conn and immediately dropped it without storing it.
        // IrisTools.iris is Arc<IrisConnection> behind &self — can't be mutated here.
        // Instead, probe the connection to verify it works and return accurate info.
        let mut new_conn = crate::iris::connection::IrisConnection::new(
            &base_url,
            &p.namespace,
            &p.username,
            &p.password,
            crate::iris::connection::DiscoverySource::Docker {
                container_name: p.name.clone(),
            },
        );
        new_conn.port_superserver = Some(port_superserver);
        new_conn.probe().await;
        let version = new_conn.version.clone();

        ok_json(serde_json::json!({
            "status": "ok",
            "container": p.name,
            "port_superserver": port_superserver,
            "port_web": port_web,
            "namespace": p.namespace,
            "version": version,
            "note": "Connection parameters validated. Restart the MCP session (set IRIS_HOST/IRIS_WEB_PORT) to switch containers.",
        }))
    }

    #[tool(
        description = "Start a dedicated IRIS container for the current project via iris-devtester CLI. Idempotent — returns existing container if already running."
    )]
    async fn iris_start_sandbox(
        &self,
        Parameters(p): Parameters<StartSandboxParams>,
    ) -> Result<CallToolResult, McpError> {
        let workspace = std::env::current_dir().unwrap_or_default();
        let workspace_basename = workspace
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("project")
            .to_string();
        let container_name = if p.name.is_empty() {
            format!("{}-iris", workspace_basename)
        } else {
            p.name.clone()
        };

        let containers = list_iris_containers(&workspace_basename).await;
        if let Some(c) = containers
            .iter()
            .find(|c| c["name"].as_str() == Some(&container_name))
        {
            if c["port_superserver"].is_number() {
                return ok_json(serde_json::json!({
                    "name": container_name,
                    "port_superserver": c["port_superserver"],
                    "port_web": c["port_web"],
                    "started": false,
                    "idempotent": true,
                }));
            }
        }

        let output = tokio::process::Command::new("idt")
            .args([
                "container",
                "up",
                "--name",
                &container_name,
                "--edition",
                &p.edition,
            ])
            .output()
            .await;

        match output {
            Err(e) => err_json(
                "INTERNAL_ERROR",
                &format!("idt not found: {e}. Install with: pip install iris-devtester"),
            ),
            Ok(out) if !out.status.success() => {
                let msg = String::from_utf8_lossy(&out.stderr);
                err_json("INTERNAL_ERROR", &format!("idt container up failed: {msg}"))
            }
            Ok(_) => {
                let containers2 = list_iris_containers(&workspace_basename).await;
                match containers2
                    .iter()
                    .find(|c| c["name"].as_str() == Some(&container_name))
                {
                    Some(c) => ok_json(serde_json::json!({
                        "name": container_name,
                        "port_superserver": c["port_superserver"],
                        "port_web": c["port_web"],
                        "started": true,
                    })),
                    None => ok_json(serde_json::json!({
                        "name": container_name,
                        "started": true,
                        "warning": "Container started but not yet visible in container list.",
                    })),
                }
            }
        }
    }

    #[tool(description = "Search for ObjectScript classes matching a query in the IRIS namespace.")]
    async fn iris_symbols(
        &self,
        Parameters(p): Parameters<SymbolsParams>,
    ) -> Result<CallToolResult, McpError> {
        let iris = self.get_iris()?;
        let client = self.http_client();
        let sql = format!(
            "SELECT TOP {} Name FROM %Dictionary.ClassDefinition WHERE Name LIKE ? ORDER BY Name",
            p.limit
        );
        match iris
            .query(
                &sql,
                vec![serde_json::Value::String(format!("%{}%", p.query))],
                &p.namespace,
                client,
            )
            .await
        {
            Ok(resp) => ok_json(
                serde_json::json!({"source": "iris_dictionary", "symbols": resp["result"]["content"], "count": resp["result"]["content"].as_array().map(|a| a.len()).unwrap_or(0)}),
            ),
            Err(e) => err_json("IRIS_UNREACHABLE", &e.to_string()),
        }
    }

    #[tool(
        description = "Search for ObjectScript symbols in local .cls files without IRIS connection."
    )]
    async fn iris_symbols_local(
        &self,
        Parameters(_p): Parameters<SymbolsLocalParams>,
    ) -> Result<CallToolResult, McpError> {
        err_json("NOT_IMPLEMENTED", "iris_symbols_local requires tree-sitter integration (pending). Use iris_symbols for IRIS-connected symbol search.")
    }

    #[tool(
        description = "Introspect an ObjectScript class — returns methods, properties, and type information."
    )]
    async fn docs_introspect(
        &self,
        Parameters(p): Parameters<IntrospectParams>,
    ) -> Result<CallToolResult, McpError> {
        let iris = self.get_iris()?;
        let client = self.http_client();
        // Bug 15: use parameterized queries instead of manual string escaping.
        let methods = iris.query(
            "SELECT Name,FormalSpec,ReturnType FROM %Dictionary.CompiledMethod WHERE parent=? ORDER BY Name",
            vec![serde_json::Value::String(p.class_name.clone())],
            &p.namespace,
            client,
        ).await.unwrap_or_default();
        let props = iris
            .query(
                "SELECT Name,Type FROM %Dictionary.CompiledProperty WHERE parent=? ORDER BY Name",
                vec![serde_json::Value::String(p.class_name.clone())],
                &p.namespace,
                client,
            )
            .await
            .unwrap_or_default();
        ok_json(
            serde_json::json!({"success": true, "class_name": p.class_name, "methods": methods["result"]["content"], "properties": props["result"]["content"]}),
        )
    }

    #[tool(
        description = "Map a .INT routine offset to the original .CLS source line. Pass routine+offset OR a raw IRIS error string like '<UNDEFINED>x+3^MyApp.Foo.1'."
    )]
    async fn debug_map_int_to_cls(
        &self,
        Parameters(mut p): Parameters<DebugMapParams>,
    ) -> Result<CallToolResult, McpError> {
        if !p.error_string.is_empty() {
            if let Some((r, o)) = parse_iris_error_string(&p.error_string) {
                p.routine = r;
                p.offset = o;
            }
        }
        let iris = self.get_iris()?;
        let _client = self.http_client();
        let code = format!(
            "Write ##class(%Studio.Debugger).SourceLine(\"{}\",{})",
            p.routine.replace('"', "\\\""),
            p.offset
        );
        match iris.execute(&code, &p.namespace).await {
            Ok(raw) => {
                let (cls_name, cls_line) = parse_source_line(raw.trim());
                ok_json(
                    serde_json::json!({"success": true, "mapping_available": cls_name.is_some(), "cls_name": cls_name, "cls_line": cls_line, "routine": p.routine, "offset": p.offset, "raw_error": if p.error_string.is_empty() { serde_json::Value::Null } else { p.error_string.into() }}),
                )
            }
            Err(e) if e.to_string() == "DOCKER_REQUIRED" => ok_json(serde_json::json!({
                "success": false, "error_code": "DOCKER_REQUIRED",
                "error": "debug_map_int requires docker exec. Set IRIS_CONTAINER=<container_name>.",
            })),
            Err(e) => err_json("IRIS_UNREACHABLE", &e.to_string()),
        }
    }

    #[tool(description = "Capture IRIS error state and recent error log entries for debugging.")]
    async fn debug_capture_packet(
        &self,
        Parameters(_p): Parameters<CapturePacketParams>,
    ) -> Result<CallToolResult, McpError> {
        let iris = self.get_iris()?;
        let client = self.http_client();
        match iris.query("SELECT TOP 20 ErrorCode,ErrorText,TimeStamp FROM %SYSTEM.Error ORDER BY TimeStamp DESC", vec![], &_p.namespace, client).await {
            Ok(resp) => ok_json(serde_json::json!({"success": true, "errors": resp["result"]["content"]})),
            Err(e) => err_json("IRIS_UNREACHABLE", &e.to_string()),
        }
    }

    #[tool(description = "Retrieve recent IRIS error log entries.")]
    async fn debug_get_error_logs(
        &self,
        Parameters(p): Parameters<ErrorLogsParams>,
    ) -> Result<CallToolResult, McpError> {
        let iris = self.get_iris()?;
        let client = self.http_client();
        // FR-012: cap max_entries to prevent runaway queries.
        let max_entries = p.max_entries.min(1000);
        let sql = format!("SELECT TOP {} ErrorCode,ErrorText,TimeStamp FROM %SYSTEM.Error ORDER BY TimeStamp DESC", max_entries);
        match iris.query(&sql, vec![], &p.namespace, client).await {
            Ok(resp) => {
                ok_json(serde_json::json!({"success": true, "logs": resp["result"]["content"]}))
            }
            Err(e) => err_json("IRIS_UNREACHABLE", &e.to_string()),
        }
    }

    #[tool(
        description = "Build a .INT source map for a compiled ObjectScript class via Atelier xecute. Maps .INT routine line offsets back to .CLS source lines for stack trace resolution. No Python required."
    )]
    async fn debug_source_map(
        &self,
        Parameters(p): Parameters<SourceMapParams>,
    ) -> Result<CallToolResult, McpError> {
        let iris = self.get_iris()?;
        let _client = self.http_client();
        let cls_name = p.cls_name.trim_end_matches(".cls");
        // Build source map by querying %Studio.Debugger for each .INT method
        let code = format!(
            "set cls=\"{}\" set rtn=$translate(cls,\".\",\".\") set map=\"{{\" set first=1 set method=\"\" for {{ set method=$order(^rIndex(rtn,method)) quit:method=\"\"  set intline=$get(^rIndex(rtn,method)) if 'first {{ set map=map_\",\" }} set map=map_\"\\\"\"_method_\"\\\":\\\"\"_intline_\"\\\"\" set first=0 }} set map=map_\"}}\" write map",
            cls_name.replace('"', "\\\"")
        );
        // Bug 23: use p.namespace, not the hardcoded "USER".
        match iris.execute(&code, &p.namespace).await {
            Ok(output) => {
                let map: serde_json::Value =
                    serde_json::from_str(output.trim()).unwrap_or(serde_json::json!({}));
                ok_json(
                    serde_json::json!({"success": true, "cls_name": cls_name, "source_map": map}),
                )
            }
            Err(e) if e.to_string() == "DOCKER_REQUIRED" => ok_json(serde_json::json!({
                "success": false, "error_code": "DOCKER_REQUIRED",
                "error": "debug_source_map requires docker exec. Set IRIS_CONTAINER=<container_name>.",
            })),
            Err(e) => err_json("IRIS_UNREACHABLE", &e.to_string()),
        }
    }

    #[tool(
        description = "Generate an ObjectScript class from a natural language description. Requires IRIS_GENERATE_CLASS_MODEL + OPENAI_API_KEY env vars."
    )]
    async fn iris_generate_class(
        &self,
        Parameters(p): Parameters<GenerateClassParams>,
    ) -> Result<CallToolResult, McpError> {
        use crate::generate::{
            extract_class_name, validate_cls_syntax, LlmClient, GENERATE_CLASS_SYSTEM,
            RETRY_TEMPLATE,
        };
        let llm = LlmClient::from_env().ok_or_else(|| {
            McpError::invalid_request(
                "LLM_UNAVAILABLE: Set IRIS_GENERATE_CLASS_MODEL and OPENAI_API_KEY",
                None,
            )
        })?;

        let class_text = llm
            .complete(GENERATE_CLASS_SYSTEM, &p.description)
            .await
            .map_err(|e| McpError {
                code: rmcp::model::ErrorCode::INTERNAL_ERROR,
                message: format!("LLM_TIMEOUT: {}", e).into(),
                data: None,
            })?;

        if !validate_cls_syntax(&class_text) {
            return ok_json(
                serde_json::json!({"success": false, "error_code": "INVALID_OUTPUT", "raw_llm_output": class_text}),
            );
        }
        let class_name =
            extract_class_name(&class_text).unwrap_or_else(|| "Generated.Class".to_string());

        if let Some(iris) = self.iris.as_deref() {
            let _client = self.http_client();
            let code = format!(
                "Set sc=$SYSTEM.OBJ.Compile(\"{}\",\"ck-d\") Write $System.Status.IsOK(sc)",
                class_name
            );
            let compile_ok = iris
                .execute(&code, &p.namespace)
                .await
                .map(|o| o.trim() == "1")
                .unwrap_or(false);

            if !compile_ok {
                let retry_prompt = RETRY_TEMPLATE.replace("{errors}", "compilation failed");
                if let Ok(fixed) = llm
                    .complete(
                        GENERATE_CLASS_SYSTEM,
                        &format!(
                            "{}

Original: {}",
                            retry_prompt, class_text
                        ),
                    )
                    .await
                {
                    let fixed_name = extract_class_name(&fixed).unwrap_or(class_name.clone());
                    let code2 = format!(
                        "Set sc=$SYSTEM.OBJ.Compile(\"{}\",\"ck-d\") Write $System.Status.IsOK(sc)",
                        fixed_name
                    );
                    let ok2 = iris
                        .execute(&code2, &p.namespace)
                        .await
                        .map(|o| o.trim() == "1")
                        .unwrap_or(false);
                    return ok_json(
                        serde_json::json!({"success": true, "class_name": fixed_name, "class_text": fixed, "compiled": ok2, "retried": true}),
                    );
                }
            }
            return ok_json(
                serde_json::json!({"success": true, "class_name": class_name, "class_text": class_text, "compiled": compile_ok, "retried": false}),
            );
        }
        ok_json(
            serde_json::json!({"success": true, "class_name": class_name, "class_text": class_text, "compiled": false, "retried": false, "note": "No IRIS connection — could not compile"}),
        )
    }

    #[tool(
        description = "Generate a %UnitTest.TestCase for an existing ObjectScript class. Introspects the class first. Requires IRIS_GENERATE_CLASS_MODEL + OPENAI_API_KEY."
    )]
    async fn iris_generate_test(
        &self,
        Parameters(p): Parameters<GenerateTestParams>,
    ) -> Result<CallToolResult, McpError> {
        use crate::generate::{
            extract_class_name, validate_cls_syntax, LlmClient, GENERATE_TEST_SYSTEM,
        };
        let llm = LlmClient::from_env().ok_or_else(|| {
            McpError::invalid_request(
                "LLM_UNAVAILABLE: Set IRIS_GENERATE_CLASS_MODEL and OPENAI_API_KEY",
                None,
            )
        })?;

        let introspection_context = if let Some(iris) = self.iris.as_deref() {
            let client = self.http_client();
            // FR-001/C1: use parameterized query to prevent SQL injection via class_name.
            iris.query(
                "SELECT Name,FormalSpec,ReturnType FROM %Dictionary.CompiledMethod WHERE parent=? ORDER BY Name",
                vec![serde_json::Value::String(p.class_name.clone())],
                &p.namespace,
                client,
            )
                .await
                .map(|r| {
                    format!(
                        "Class: {}
Methods:
{}",
                        p.class_name,
                        serde_json::to_string_pretty(&r["result"]["content"]).unwrap_or_default()
                    )
                })
                .unwrap_or_else(|_| format!("Class: {} (introspection unavailable)", p.class_name))
        } else {
            format!(
                "Class: {} (no IRIS connection — generating scaffold)",
                p.class_name
            )
        };

        let prompt = format!(
            "Generate tests for the following ObjectScript class:

{}",
            introspection_context
        );
        let test_text = llm
            .complete(GENERATE_TEST_SYSTEM, &prompt)
            .await
            .map_err(|e| McpError {
                code: rmcp::model::ErrorCode::INTERNAL_ERROR,
                message: format!("LLM_TIMEOUT: {}", e).into(),
                data: None,
            })?;

        if !validate_cls_syntax(&test_text) {
            return ok_json(
                serde_json::json!({"success": false, "error_code": "INVALID_OUTPUT", "raw_llm_output": test_text}),
            );
        }
        let test_class_name =
            extract_class_name(&test_text).unwrap_or_else(|| format!("Test.{}", p.class_name));
        ok_json(
            serde_json::json!({"success": true, "class_name": p.class_name, "test_class_name": test_class_name, "test_text": test_text, "introspected": !introspection_context.contains("unavailable")}),
        )
    }

    #[tool(description = "List all synthesized skills in the registry.")]
    async fn skill_list(&self, _: Parameters<NoParams>) -> Result<CallToolResult, McpError> {
        if let Some(iris) = self.iris.as_deref() {
            let code = "Set key=\"\" Set result=\"[\" Set sep=\"\" For { Set key=$Order(^SKILLS(key)) Quit:key=\"\" Set skill=$Get(^SKILLS(key)) Set result=result_sep_skill Set sep=\",\" } Set result=result_\"]\" Write result";
            if let Ok(output) = iris.execute(code, &crate::tools::skills_tools::skills_namespace()).await {
                if let Ok(skills) = serde_json::from_str::<serde_json::Value>(output.trim()) {
                    let count = skills.as_array().map(|a| a.len()).unwrap_or(0);
                    return ok_json(serde_json::json!({"skills": skills, "count": count}));
                }
            }
        }
        ok_json(serde_json::json!({"skills": [], "count": 0}))
    }

    #[tool(description = "Describe a skill by name.")]
    async fn skill_describe(
        &self,
        Parameters(p): Parameters<SkillNameParams>,
    ) -> Result<CallToolResult, McpError> {
        if let Some(iris) = self.iris.as_deref() {
            let code = format!("Write $Get(^SKILLS(\"{}\"))", p.name.replace('"', "\\\""));
            if let Ok(output) = iris.execute(&code, &crate::tools::skills_tools::skills_namespace()).await {
                if let Ok(skill) = serde_json::from_str::<serde_json::Value>(output.trim()) {
                    return ok_json(serde_json::json!({"success": true, "skill": skill}));
                }
            }
        }
        err_json("NOT_FOUND", &format!("Skill '{}' not found", p.name))
    }

    #[tool(
        description = "Search synthesized skills by name and description. Returns skills whose name or description contains the query terms."
    )]
    async fn skill_search(
        &self,
        Parameters(p): Parameters<SkillSearchParams>,
    ) -> Result<CallToolResult, McpError> {
        if let Some(iris) = self.iris.as_deref() {
            let query_lower = p.query.to_lowercase();
            let q = query_lower.replace('"', "");
            let code = format!(
                concat!(
                    r#"Set key="",results="[",sep="" "#,
                    r#"For {{ Set key=$Order(^SKILLS(key)) Quit:key="" "#,
                    r#"Set skill=$Get(^SKILLS(key)) "#,
                    r#"If ($ZConvert(skill,"L")["{0}")||($ZConvert(key,"L")["{0}") "#,
                    r#"{{ Set results=results_sep_skill Set sep="," }} }} "#,
                    r#"Set results=results_"]" Write results"#
                ),
                q
            );
            if let Ok(output) = iris.execute(&code, &crate::tools::skills_tools::skills_namespace()).await {
                if let Ok(skills) = serde_json::from_str::<Vec<serde_json::Value>>(output.trim()) {
                    let limited: Vec<_> = skills.into_iter().take(p.top_k).collect();
                    let count = limited.len();
                    return ok_json(
                        serde_json::json!({"query": p.query, "results": limited, "count": count}),
                    );
                }
            }
        }
        ok_json(serde_json::json!({"query": p.query, "results": [], "count": 0}))
    }

    #[tool(description = "Remove a skill from the registry by name.")]
    async fn skill_forget(
        &self,
        Parameters(p): Parameters<SkillNameParams>,
    ) -> Result<CallToolResult, McpError> {
        if let Some(iris) = self.iris.as_deref() {
            let code = format!(
                "Kill ^SKILLS(\"{}\") Write \"OK\"",
                p.name.replace('"', "\\\"")
            );
            if iris.execute(&code, &crate::tools::skills_tools::skills_namespace()).await.is_ok() {
                return ok_json(serde_json::json!({"success": true, "name": p.name}));
            }
        }
        err_json(
            "DOCKER_REQUIRED",
            "skill_forget requires docker exec. Set IRIS_CONTAINER=<container_name>.",
        )
    }

    #[tool(
        description = "Trigger pattern miner to synthesize new skills from recorded tool calls."
    )]
    async fn skill_propose(&self, _: Parameters<NoParams>) -> Result<CallToolResult, McpError> {
        err_json("NOT_IMPLEMENTED", "skill_propose: pattern mining not yet implemented")
    }

    #[tool(description = "Optimize a skill using DSPy. Requires OBJECTSCRIPT_DSPY=true.")]
    async fn skill_optimize(
        &self,
        Parameters(_p): Parameters<SkillNameParams>,
    ) -> Result<CallToolResult, McpError> {
        err_json("NOT_IMPLEMENTED", "skill_optimize: DSPy optimization not yet implemented")
    }

    #[tool(description = "Share a skill to the community via GitHub PR.")]
    async fn skill_share(
        &self,
        Parameters(_p): Parameters<SkillNameParams>,
    ) -> Result<CallToolResult, McpError> {
        err_json("NOT_IMPLEMENTED", "skill_share: GitHub PR integration not yet implemented")
    }

    #[tool(
        description = "List all skills loaded from --subscribe packages. Use --subscribe owner/repo when starting iris-dev mcp to load community skills."
    )]
    async fn skill_community_list(
        &self,
        _: Parameters<NoParams>,
    ) -> Result<CallToolResult, McpError> {
        let skills: Vec<_> = self
            .registry
            .list_skills()
            .iter()
            .map(|s| {
                serde_json::json!({
                    "name": s.name,
                    "description": s.description,
                    "source": s.source_repo,
                })
            })
            .collect();
        let kb_items: Vec<_> = self
            .registry
            .list_kb_items()
            .iter()
            .map(|k| {
                serde_json::json!({
                    "title": k.title,
                    "source": k.source_repo,
                })
            })
            .collect();
        ok_json(serde_json::json!({
            "skills": skills,
            "kb_items": kb_items,
            "skill_count": skills.len(),
            "kb_count": kb_items.len(),
            "hint": "Start iris-dev mcp with --subscribe owner/repo to load community packages"
        }))
    }

    #[tool(description = "Install a community skill from the GitHub community repo.")]
    async fn skill_community_install(
        &self,
        Parameters(_p): Parameters<CommunityPkgParams>,
    ) -> Result<CallToolResult, McpError> {
        err_json("NOT_IMPLEMENTED", "skill_community_install: community registry not yet implemented")
    }

    #[tool(description = "Index markdown files into the IRIS knowledge base for semantic search.")]
    async fn kb_index(
        &self,
        Parameters(p): Parameters<KbIndexParams>,
    ) -> Result<CallToolResult, McpError> {
        let iris = self.get_iris()?;
        skills_tools::handle_kb(
            iris,
            self.http_client(),
            skills_tools::KbParams {
                action: "index".into(),
                path: p.workspace_path,
                query: None,
                top_k: 0,
            },
        )
        .await
    }

    #[tool(
        description = "Search the knowledge base for relevant guidance. Searches subscribed KB packages and any indexed content."
    )]
    async fn kb_recall(
        &self,
        Parameters(p): Parameters<KbRecallParams>,
    ) -> Result<CallToolResult, McpError> {
        let q = p.query.to_lowercase();
        let mut results: Vec<serde_json::Value> = vec![];

        // Search subscribed KB items (BM25 substring match)
        for item in self.registry.list_kb_items() {
            let content_lower = item.content.to_lowercase();
            if content_lower.contains(&q) || item.title.to_lowercase().contains(&q) {
                // Extract a relevant snippet around the match
                let snippet = content_lower
                    .find(&q)
                    .and_then(|pos| {
                        // FR-018/Mo4: use char-boundary-safe slicing to prevent None on multibyte UTF-8.
                        let snippet_start = {
                            let mut s = pos.saturating_sub(150);
                            while s > 0 && !item.content.is_char_boundary(s) {
                                s -= 1;
                            }
                            s
                        };
                        let snippet_end = {
                            let mut e = (pos + q.len() + 300).min(item.content.len());
                            while e < item.content.len() && !item.content.is_char_boundary(e) {
                                e += 1;
                            }
                            e
                        };
                        item.content.get(snippet_start..snippet_end)
                    })
                    .map(|s| format!("...{}...", s.trim()))
                    .unwrap_or_else(|| item.content.chars().take(300).collect());
                results.push(serde_json::json!({
                    "title": item.title,
                    "snippet": snippet,
                    "source": item.source_repo,
                    "score": if item.title.to_lowercase().contains(&q) { 0.9 } else { 0.7 }
                }));
            }
        }

        // Sort by score descending, limit to top_k
        results.sort_by(|a, b| {
            b["score"]
                .as_f64()
                .unwrap_or(0.0)
                .partial_cmp(&a["score"].as_f64().unwrap_or(0.0))
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        results.truncate(p.top_k);

        let count = results.len();
        ok_json(serde_json::json!({"query": p.query, "results": results, "count": count}))
    }

    #[tool(description = "Return recent tool call history for this session.")]
    async fn agent_history(
        &self,
        Parameters(p): Parameters<AgentHistoryParams>,
    ) -> Result<CallToolResult, McpError> {
        let calls: Vec<serde_json::Value> = self
            .history
            .lock()
            .map(|h| {
                h.iter()
                    .rev()
                    .take(p.limit)
                    .map(|c| {
                        serde_json::json!({
                            "tool": c.tool,
                            "success": c.success,
                            "ago_secs": c.timestamp.elapsed().as_secs(),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();
        ok_json(serde_json::json!({"calls": calls, "limit": p.limit}))
    }

    #[tool(description = "Return learning agent status: skill count, pattern count, KB size.")]
    async fn agent_stats(&self, _: Parameters<NoParams>) -> Result<CallToolResult, McpError> {
        let skill_count = self.registry.list_skills().len();
        let session_calls = self.history.lock().map(|h| h.len()).unwrap_or(0);
        let learning_enabled = std::env::var("OBJECTSCRIPT_LEARNING")
            .map(|v| v != "false")
            .unwrap_or(true);
        ok_json(serde_json::json!({
            "status": "ok",
            "skill_count": skill_count,
            "session_calls": session_calls,
            "learning_enabled": learning_enabled,
        }))
    }

    #[tool(
        description = "Returns the current state of the running IRIS Interoperability production. With full_status=true, includes per-component breakdown."
    )]
    async fn interop_production_status(
        &self,
        Parameters(p): Parameters<interop::ProductionStatusParams>,
    ) -> Result<CallToolResult, McpError> {
        interop::interop_production_status_impl(self.iris.as_deref(), p).await
    }

    #[tool(description = "Start a named IRIS Interoperability production.")]
    async fn interop_production_start(
        &self,
        Parameters(p): Parameters<interop::ProductionNameParams>,
    ) -> Result<CallToolResult, McpError> {
        interop::interop_production_start_impl(self.iris.as_deref(), p).await
    }

    #[tool(
        description = "Stop the running IRIS Interoperability production with optional timeout and force."
    )]
    async fn interop_production_stop(
        &self,
        Parameters(p): Parameters<interop::ProductionStopParams>,
    ) -> Result<CallToolResult, McpError> {
        interop::interop_production_stop_impl(self.iris.as_deref(), p).await
    }

    #[tool(description = "Hot-apply configuration changes to the running production.")]
    async fn interop_production_update(
        &self,
        Parameters(p): Parameters<interop::ProductionUpdateParams>,
    ) -> Result<CallToolResult, McpError> {
        interop::interop_production_update_impl(self.iris.as_deref(), p).await
    }

    #[tool(
        description = "Check if the production configuration has changed and needs to be updated."
    )]
    async fn interop_production_needs_update(
        &self,
        Parameters(p): Parameters<interop::ProductionNeedsUpdateParams>,
    ) -> Result<CallToolResult, McpError> {
        interop::interop_production_needs_update_impl(self.iris.as_deref(), p).await
    }

    #[tool(description = "Recover a troubled IRIS Interoperability production.")]
    async fn interop_production_recover(
        &self,
        Parameters(p): Parameters<interop::ProductionRecoverParams>,
    ) -> Result<CallToolResult, McpError> {
        interop::interop_production_recover_impl(self.iris.as_deref(), p).await
    }

    #[tool(
        description = "Get recent Interoperability production log entries. Filter by log_type (comma-separated: error,warning,info,alert) and component name."
    )]
    async fn interop_logs(
        &self,
        Parameters(p): Parameters<interop::LogsParams>,
    ) -> Result<CallToolResult, McpError> {
        interop::interop_logs_impl(self.iris.as_deref(), p).await
    }

    #[tool(description = "Get all current Interoperability message queues and their depths.")]
    async fn interop_queues(&self, _: Parameters<NoParams>) -> Result<CallToolResult, McpError> {
        interop::interop_queues_impl(self.iris.as_deref()).await
    }

    #[tool(
        description = "Search the Interoperability message archive by source, target, or message class."
    )]
    async fn interop_message_search(
        &self,
        Parameters(p): Parameters<interop::MessageSearchParams>,
    ) -> Result<CallToolResult, McpError> {
        interop::interop_message_search_impl(self.iris.as_deref(), p).await
    }

    #[tool(
        description = "Full-text search across IRIS documents via Atelier REST v2. Auto-upgrades to async polling for large namespaces. Supports regex, case sensitivity, category filter (CLS/MAC/INT/INC/ALL), and wildcard document scopes."
    )]
    async fn iris_search(
        &self,
        Parameters(p): Parameters<search::SearchParams>,
    ) -> Result<CallToolResult, McpError> {
        let iris = self.get_iris()?;
        let result = search::handle_iris_search(iris, self.http_client(), p).await;
        self.record_call("iris_search", result.is_ok());
        result
    }

    #[tool(
        description = "Discover IRIS namespace contents. what=documents lists all docs, what=modified lists recently changed, what=namespace returns config, what=metadata returns IRIS version, what=jobs lists active jobs, what=csp_apps lists CSP apps, what=csp_debug returns debug ID, what=sa_schema returns SQL Analytics schema."
    )]
    async fn iris_info(
        &self,
        Parameters(p): Parameters<info::InfoParams>,
    ) -> Result<CallToolResult, McpError> {
        let iris = self.get_iris()?;
        let result = info::handle_iris_info(iris, self.http_client(), p).await;
        self.record_call("iris_info", result.is_ok());
        result
    }

    #[tool(
        description = "Inspect IRIS macros. action=list returns all macros, action=signature returns parameters, action=location finds definition file/line, action=definition returns text, action=expand expands with arguments."
    )]
    async fn iris_macro(
        &self,
        Parameters(p): Parameters<info::MacroParams>,
    ) -> Result<CallToolResult, McpError> {
        let iris = self.get_iris()?;
        let result = info::handle_iris_macro(iris, self.http_client(), p).await;
        self.record_call("iris_macro", result.is_ok());
        result
    }

    #[tool(
        description = "IRIS debug tools. action=map_int maps a runtime error offset to source line, action=error_logs fetches recent error log entries, action=capture captures current error state, action=source_map builds .INT to .CLS mapping."
    )]
    async fn iris_debug(
        &self,
        Parameters(p): Parameters<info::DebugParams>,
    ) -> Result<CallToolResult, McpError> {
        let iris = self.get_iris()?;
        let result = info::handle_iris_debug(iris, self.http_client(), p).await;
        self.record_call("iris_debug", result.is_ok());
        result
    }

    #[tool(
        description = "Prepare context for generating an ObjectScript class or %UnitTest. Returns a ready-to-use prompt plus IRIS namespace context (existing class names, method signatures). No API key needed — the calling AI agent does the generation using the returned prompt, then saves with iris_doc(mode=put) and compiles with iris_compile. gen_type=class for new classes, gen_type=test for %UnitTest scaffolding."
    )]
    async fn iris_generate(
        &self,
        Parameters(p): Parameters<info::GenerateParams>,
    ) -> Result<CallToolResult, McpError> {
        let iris = self.get_iris()?;
        let result = info::handle_iris_generate(iris, self.http_client(), p).await;
        self.record_call("iris_generate", result.is_ok());
        result
    }

    #[tool(
        description = "Manage the learning agent skill registry. action=list returns all skills, action=describe returns one skill, action=search finds skills by keyword, action=forget removes a skill, action=propose mines recent tool calls and synthesizes a new skill (requires ≥5 calls)."
    )]
    async fn skill(
        &self,
        Parameters(p): Parameters<skills_tools::SkillParams>,
    ) -> Result<CallToolResult, McpError> {
        let iris = self.get_iris()?;
        let result = skills_tools::handle_skill(iris, self.http_client(), p, &self.history).await;
        self.record_call("skill", result.is_ok());
        result
    }

    #[tool(
        description = "Community skill registry. action=list browses published skills from subscribed GitHub repos, action=install writes a community skill to the local ^SKILLS global."
    )]
    async fn skill_community(
        &self,
        Parameters(p): Parameters<skills_tools::SkillCommunityParams>,
    ) -> Result<CallToolResult, McpError> {
        let iris = self.get_iris()?;
        let result =
            skills_tools::handle_skill_community(iris, self.http_client(), p, &self.registry).await;
        self.record_call("skill_community", result.is_ok());
        result
    }

    #[tool(
        description = "Knowledge base tools. action=index reads markdown/text files and stores them in ^KBCHUNKS, action=recall searches the KB for relevant content by keyword."
    )]
    async fn kb(
        &self,
        Parameters(p): Parameters<skills_tools::KbParams>,
    ) -> Result<CallToolResult, McpError> {
        let iris = self.get_iris()?;
        let result = skills_tools::handle_kb(iris, self.http_client(), p).await;
        self.record_call("kb", result.is_ok());
        result
    }

    #[tool(
        description = "Session and learning agent information. what=stats returns skill count and session call count, what=history returns recent tool call history."
    )]
    async fn agent_info(
        &self,
        Parameters(p): Parameters<skills_tools::AgentInfoParams>,
    ) -> Result<CallToolResult, McpError> {
        let iris = self.get_iris()?;
        let result =
            skills_tools::handle_agent_info(iris, self.http_client(), p, &self.history).await;
        self.record_call("agent_info", result.is_ok());
        result
    }

    #[tool(
        description = "IRIS source control operations. action=status checks lock state and owner, action=menu lists available SCM actions, action=checkout checks out the document, action=execute runs a specific SCM action by ID. Handles elicitation for interactive SCM dialogs. Pass elicitation_id+answer to resume a pending SCM interaction."
    )]
    async fn iris_source_control(
        &self,
        Parameters(p): Parameters<ScmParams>,
    ) -> Result<CallToolResult, McpError> {
        let iris = self.get_iris()?;
        let result =
            scm::handle_iris_source_control(iris, self.http_client(), p, &self.elicitation_store)
                .await;
        self.record_call("iris_source_control", result.is_ok());
        result
    }
}

#[tool_handler]
impl ServerHandler for IrisTools {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::from_build_env())
            .with_instructions("iris-dev v2: 20 composable tools for ObjectScript and IRIS development. No Python required.".to_string())
    }
}

fn parse_iris_error_string(s: &str) -> Option<(String, i64)> {
    let re = regex::Regex::new(r"<[A-Z]+>\s*[^+\s]+\+(\d+)\^([\w.%]+)").ok()?;
    let caps = re.captures(s)?;
    Some((caps[2].to_string(), caps[1].parse().ok()?))
}

fn parse_source_line(raw: &str) -> (Option<String>, Option<i64>) {
    if raw.is_empty() {
        return (None, None);
    }
    if let Some((cls, line)) = raw.split_once(':') {
        return (
            Some(cls.trim_end_matches(".cls").to_string()),
            line.trim().parse().ok(),
        );
    }
    (None, None)
}
