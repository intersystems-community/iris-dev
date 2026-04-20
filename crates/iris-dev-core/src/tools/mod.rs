use rmcp::{
    ServerHandler, RoleServer,
    model::*,
    tool, tool_handler, tool_router,
    handler::server::wrapper::Parameters,
    service::RequestContext,
    ErrorData as McpError,
    handler::server::router::tool::ToolRouter,
};
use serde::{Deserialize, Serialize};
use schemars::JsonSchema;
use std::sync::Arc;
use std::collections::VecDeque;
use crate::iris::connection::IrisConnection;
pub mod interop;

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
pub struct SkillNameParams { pub name: String }
#[derive(Debug, Deserialize, JsonSchema)]
pub struct SkillSearchParams {
    pub query: String,
    #[serde(default = "default_limit")]
    pub top_k: usize,
}
#[derive(Debug, Deserialize, JsonSchema)]
pub struct KbIndexParams { pub workspace_path: Option<String> }
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
pub struct SymbolsLocalParams { pub workspace_path: Option<String> }
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
pub struct CommunityPkgParams { pub name: String }
#[derive(Debug, Deserialize, JsonSchema)]
pub struct SourceMapParams {
    pub cls_text: String,
    pub cls_name: String,
    pub workspace_path: Option<String>,
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

fn default_flags() -> String { "cuk".to_string() }
fn default_namespace() -> String { "USER".to_string() }
fn default_limit() -> usize { 20 }
fn default_max_entries() -> usize { 50 }
fn default_execute_timeout() -> u64 { 30 }
fn default_username() -> String { "_SYSTEM".to_string() }
fn default_password() -> String { "SYS".to_string() }
fn default_edition() -> String { "community".to_string() }

fn iris_unreachable() -> McpError {
    McpError::invalid_request("IRIS_UNREACHABLE: no IRIS connection. Set IRIS_HOST and IRIS_WEB_PORT env vars, or ensure IRIS is reachable on a discoverable port (52773, 41773, 51773, 8080).", None)
}
fn ok_json(v: serde_json::Value) -> Result<CallToolResult, McpError> {
    Ok(CallToolResult::success(vec![Content::text(v.to_string())]))
}
fn err_json(code: &str, msg: &str) -> Result<CallToolResult, McpError> {
    ok_json(serde_json::json!({"success": false, "error_code": code, "error": msg}))
}
fn err_json_with_url(code: &str, msg: &str, attempted_url: &str) -> Result<CallToolResult, McpError> {
    ok_json(serde_json::json!({
        "success": false,
        "error_code": code,
        "error": msg,
        "attempted_url": attempted_url,
        "hint": "Check IRIS_HOST and IRIS_WEB_PORT (and IRIS_WEB_PREFIX if using a non-root gateway)"
    }))
}
fn is_network_error(msg: &str) -> bool {
    msg.contains("error sending request") || msg.contains("connection") || msg.contains("dns")
}

fn score_container(name: &str, workspace_basename: &str) -> i64 {
    if workspace_basename.is_empty() { return 0; }
    let cn = name.to_lowercase();
    let wb = workspace_basename.to_lowercase();
    let base: i64 = if cn == wb { 100 } else if cn.starts_with(&wb) { 80 } else if cn.contains(&wb) { 60 } else { 0 };
    if base == 0 { return 0; }
    let suffix = if cn.ends_with("-iris") || cn.ends_with("_iris") { 10i64 } else { 0 }
        + if cn.ends_with("-test") || cn.ends_with("_test") { 5 } else { 0 };
    base + suffix
}

fn extract_port(ports: &str, container_port: &str) -> Option<u16> {
    let pat = format!("(\\d+)->{}", regex::escape(container_port));
    regex::Regex::new(&pat).ok()?.captures(ports)
        .and_then(|c| c[1].parse().ok())
}

async fn list_iris_containers(workspace_basename: &str) -> Vec<serde_json::Value> {
    let mut containers: Vec<serde_json::Value> = Vec::new();

    if let Ok(out) = tokio::process::Command::new("idt")
        .args(["container", "list", "--format", "json"])
        .output().await
    {
        if out.status.success() {
            if let Ok(items) = serde_json::from_slice::<Vec<serde_json::Value>>(&out.stdout) {
                for item in items {
                    let name = item["name"].as_str().unwrap_or("").to_string();
                    let ports = item["ports"].as_str().unwrap_or("");
                    let sp = extract_port(ports, "1972").map(|p| serde_json::json!(p)).unwrap_or(serde_json::Value::Null);
                    let wp = extract_port(ports, "52773").map(|p| serde_json::json!(p)).unwrap_or(serde_json::Value::Null);
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
        .args(["ps", "--format", "{{.Names}}\t{{.Image}}\t{{.Ports}}\t{{.Status}}\t{{.RunningFor}}"])
        .output().await
    {
        if out.status.success() {
            for line in String::from_utf8_lossy(&out.stdout).lines() {
                let parts: Vec<&str> = line.splitn(5, '\t').collect();
                if parts.len() < 5 { continue; }
                let (name, image, ports_raw, age) = (parts[0], parts[1], parts[2], parts[4]);
                if !image.to_lowercase().contains("intersystems") && !image.to_lowercase().contains("iris") { continue; }
                let sp = extract_port(ports_raw, "1972").map(|p| serde_json::json!(p)).unwrap_or(serde_json::Value::Null);
                let wp = extract_port(ports_raw, "52773").map(|p| serde_json::json!(p)).unwrap_or(serde_json::Value::Null);
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
            a["name"].as_str().unwrap_or("").cmp(b["name"].as_str().unwrap_or(""))
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
    tool_router: ToolRouter<IrisTools>,
}

#[tool_router]
impl IrisTools {
    pub fn new(iris: Option<IrisConnection>) -> Self {
        let client = Arc::new(IrisConnection::http_client().unwrap_or_default());
        Self {
            iris: iris.map(Arc::new),
            registry: Arc::new(crate::skills::SkillRegistry::new()),
            client,
            history: Arc::new(std::sync::Mutex::new(VecDeque::with_capacity(50))),
            tool_router: Self::tool_router(),
        }
    }
    pub fn with_registry(iris: Option<IrisConnection>, registry: crate::skills::SkillRegistry) -> Self {
        let client = Arc::new(IrisConnection::http_client().unwrap_or_default());
        Self {
            iris: iris.map(Arc::new),
            registry: Arc::new(registry),
            client,
            history: Arc::new(std::sync::Mutex::new(VecDeque::with_capacity(50))),
            tool_router: Self::tool_router(),
        }
    }
    fn get_iris(&self) -> Result<&IrisConnection, McpError> {
        self.iris.as_deref().ok_or_else(iris_unreachable)
    }
    fn http_client(&self) -> &reqwest::Client {
        &self.client
    }
    fn record_call(&self, tool: &str, success: bool) {
        if let Ok(mut h) = self.history.lock() {
            if h.len() == 50 { h.pop_front(); }
            h.push_back(ToolCallEntry { tool: tool.to_string(), success, timestamp: std::time::Instant::now() });
        }
    }

    #[tool(description = "Compile an ObjectScript class, routine, or wildcard package on IRIS via Atelier REST. Supports 'MyApp.*.cls' for package-level compilation. Returns structured errors with line numbers, columns, and severity. No Python required.")]
    async fn iris_compile(&self, Parameters(p): Parameters<CompileParams>) -> Result<CallToolResult, McpError> {
        let iris = self.get_iris()?;
        let client = self.http_client();

        // Expand wildcards: resolve "MyApp.*.cls" to a list of matching class names
        let targets: Vec<String> = if p.target.contains('*') {
            let list_url = iris.atelier_url(&format!("/v8/{}/docs?category=CLS", iris.namespace));
            match client.get(&list_url)
                .basic_auth(&iris.username, Some(&iris.password))
                .send().await
            {
                Ok(resp) if resp.status().is_success() => {
                    let body: serde_json::Value = resp.json().await.unwrap_or_default();
                    let pattern = p.target.replace('.', "\\.").replace('*', ".*");
                    let re = regex::Regex::new(&format!("(?i)^{}$", pattern)).unwrap_or_else(|_| regex::Regex::new(".*").unwrap());
                    body["result"]["content"].as_array().unwrap_or(&vec![])
                        .iter()
                        .filter_map(|d| d["name"].as_str())
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
            return err_json("NOT_FOUND", &format!("No documents match pattern: {}", p.target));
        }

        // Optional force_writable: unlock read-only databases
        if p.force_writable {
            let xecute_url = iris.atelier_url(&format!("/v1/{}/action/xecute", p.namespace));
            let _ = client.post(&xecute_url)
                .basic_auth(&iris.username, Some(&iris.password))
                .json(&serde_json::json!({"expression": format!("do ##class(%Library.EnsembleMgr).EnableNamespace(\"{}\",1)", p.namespace)}))
                .send().await;
        }

        let compile_url = iris.atelier_url(&format!("/v8/{}/action/compile", p.namespace));
        let resp = client.post(&compile_url)
            .basic_auth(&iris.username, Some(&iris.password))
            .json(&serde_json::json!({"docs": targets, "flags": p.flags}))
            .send().await
            .map_err(|e| McpError::internal_error(format!("HTTP error: {e}"), None))?;

        if !resp.status().is_success() && resp.status().as_u16() != 200 {
            let url_str = compile_url.clone();
            let status = resp.status().as_u16();
            return err_json_with_url("IRIS_UNREACHABLE", &format!("HTTP {}", status), &url_str);
        }

        let body: serde_json::Value = resp.json().await
            .map_err(|e| McpError::internal_error(format!("JSON parse error: {e}"), None))?;

        // Parse compiler output: extract structured errors from console array
        let console = body["result"]["console"].as_array().cloned().unwrap_or_default();
        let mut errors = vec![];
        let mut warnings = vec![];
        for line in &console {
            let text = line.as_str().unwrap_or("");
            // Atelier compile errors: "  1 ERROR #<code>:<line>: <message>"
            // Warnings: "  2 WARNING #<code>:<line>: <message>"
            if let Some(rest) = text.trim().strip_prefix("ERROR ") {
                let parts: Vec<&str> = rest.splitn(3, ':').collect();
                let (code, line_num, msg) = if parts.len() >= 3 {
                    (parts[0].trim(), parts[1].trim().parse::<u32>().unwrap_or(0), parts[2].trim())
                } else {
                    ("", 0, rest)
                };
                errors.push(serde_json::json!({"severity":"error","code":code,"line":line_num,"column":0,"text":msg}));
            } else if let Some(rest) = text.trim().strip_prefix("WARNING ") {
                let parts: Vec<&str> = rest.splitn(3, ':').collect();
                let (code, line_num, msg) = if parts.len() >= 3 {
                    (parts[0].trim(), parts[1].trim().parse::<u32>().unwrap_or(0), parts[2].trim())
                } else {
                    ("", 0, rest)
                };
                warnings.push(serde_json::json!({"severity":"warning","code":code,"line":line_num,"column":0,"text":msg}));
            }
        }

        let success = errors.is_empty();
        self.record_call("iris_compile", success);
        ok_json(serde_json::json!({
            "success": success,
            "target": p.target,
            "targets_compiled": targets.len(),
            "namespace": p.namespace,
            "errors": errors,
            "warnings": warnings,
            "console": console,
        }))
    }

    #[tool(description = "Run %UnitTest.Manager tests on IRIS via Atelier REST. Pass a class pattern like 'MyApp.Tests' or 'MyApp.Tests.Order'. Returns structured pass/fail counts and full trace output. No Python required.")]
    async fn iris_test(&self, Parameters(p): Parameters<TestParams>) -> Result<CallToolResult, McpError> {
        let iris = self.get_iris()?;
        let client = self.http_client();
        let code = format!(
            "do ##class(%UnitTest.Manager).RunTest(\"{}\",\"/noload/run\")",
            p.pattern.replace('"', "\\\"")
        );
        let xecute_url = iris.atelier_url(&format!("/v1/{}/action/xecute", p.namespace));
        let resp = client.post(&xecute_url)
            .basic_auth(&iris.username, Some(&iris.password))
            .json(&serde_json::json!({"expression": code}))
            .send().await
            .map_err(|e| McpError::internal_error(format!("HTTP error: {e}"), None))?;

        if !resp.status().is_success() {
            return err_json_with_url("IRIS_UNREACHABLE", &format!("HTTP {}", resp.status()), &xecute_url);
        }

        let body: serde_json::Value = resp.json().await.unwrap_or_default();
        let output_lines = body["result"]["content"][0]["content"]
            .as_array()
            .map(|a| a.iter().filter_map(|v| v.as_str()).collect::<Vec<_>>().join("\n"))
            .unwrap_or_default();

        let passed = output_lines.lines()
            .find(|l| l.to_lowercase().contains("passed:"))
            .and_then(|l| l.split(':').nth(1)?.trim().split_whitespace().next()?.parse::<u64>().ok())
            .unwrap_or(0);
        let failed = output_lines.lines()
            .find(|l| l.to_lowercase().contains("failed:"))
            .and_then(|l| l.split(':').nth(1)?.trim().split_whitespace().next()?.parse::<u64>().ok())
            .unwrap_or(0);
        let total = passed + failed;
        let success = failed == 0 && total > 0;
        self.record_call("iris_test", success);
        ok_json(serde_json::json!({
            "success": success,
            "pattern": p.pattern,
            "namespace": p.namespace,
            "passed": passed,
            "failed": failed,
            "total": total,
            "output": output_lines,
        }))
    }

    #[tool(description = "Execute arbitrary ObjectScript code on IRIS via Atelier REST and return stdout output. No Python required. Example: 'write $ZVERSION,!' returns the IRIS version.")]
    async fn iris_execute(&self, Parameters(p): Parameters<ExecuteParams>) -> Result<CallToolResult, McpError> {
        let iris = self.get_iris()?;
        let client = self.http_client();
        let xecute_url = iris.atelier_url(&format!("/v1/{}/action/xecute", p.namespace));
        let resp = client.post(&xecute_url)
            .basic_auth(&iris.username, Some(&iris.password))
            .json(&serde_json::json!({"expression": p.code}))
            .send().await
            .map_err(|e| McpError::internal_error(format!("HTTP error: {e}"), None))?;

        if !resp.status().is_success() {
            return err_json_with_url("IRIS_UNREACHABLE", &format!("HTTP {}", resp.status()), &xecute_url);
        }

        let body: serde_json::Value = resp.json().await.unwrap_or_default();

        // Check for IRIS-level errors in response
        if let Some(errors) = body["status"]["errors"].as_array() {
            if !errors.is_empty() {
                let first = &errors[0];
                let iris_err = serde_json::json!({
                    "code": first["code"],
                    "domain": first["domain"].as_str().unwrap_or(""),
                    "id": first["id"].as_str().unwrap_or(""),
                    "params": first["params"],
                });
                self.record_call("iris_execute", false);
                return ok_json(serde_json::json!({
                    "success": false,
                    "error_code": "IRIS_ERROR",
                    "error": first["error"].as_str().unwrap_or("ObjectScript error"),
                    "iris_error": iris_err,
                }));
            }
        }

        let output = body["result"]["content"][0]["content"]
            .as_array()
            .map(|a| a.iter().filter_map(|v| v.as_str()).collect::<Vec<_>>().join("\n"))
            .unwrap_or_default();

        self.record_call("iris_execute", true);
        ok_json(serde_json::json!({
            "success": true,
            "output": output,
            "namespace": p.namespace,
        }))
    }

    #[tool(description = "List running IRIS Docker containers with name-match scoring. Tries iris-devtester first, falls back to docker ps. Containers sorted by score (name similarity to workspace) descending.")]
    async fn iris_list_containers(&self, Parameters(p): Parameters<ListContainersParams>) -> Result<CallToolResult, McpError> {
        let workspace_basename = p.workspace_root
            .as_deref()
            .map(|r| std::path::Path::new(r).file_name().and_then(|n| n.to_str()).unwrap_or("").to_string())
            .unwrap_or_default();

        let containers = list_iris_containers(&workspace_basename).await;
        let suggestion = containers.first().map(|c: &serde_json::Value| {
            format!("iris_select_container(name='{}')", c["name"].as_str().unwrap_or(""))
        });
        ok_json(serde_json::json!({
            "status": "ok",
            "containers": containers,
            "workspace_basename": workspace_basename,
            "suggestion": suggestion,
        }))
    }

    #[tool(description = "Switch the active IRIS container. Reconnects the MCP server to the specified container. Returns new connection info including version.")]
    async fn iris_select_container(&self, Parameters(p): Parameters<SelectContainerParams>) -> Result<CallToolResult, McpError> {
        let workspace_basename = String::new();

        let containers = list_iris_containers(&workspace_basename).await;
        let found = containers.iter().find(|c| c["name"].as_str() == Some(&p.name));

        let container = match found {
            Some(c) => c.clone(),
            None => {
                let available: Vec<_> = containers.iter().filter_map(|c| c["name"].as_str()).collect();
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

        let mut new_conn = crate::iris::connection::IrisConnection::new(
            &base_url, &p.namespace, &p.username, &p.password,
            crate::iris::connection::DiscoverySource::Docker { container_name: p.name.clone() },
        );
        new_conn.port_superserver = Some(port_superserver);

        let client = IrisConnection::http_client().unwrap_or_default();
        let version = new_conn.xecute("Write $ZVERSION", &client).await
            .ok()
            .and_then(|v| v["result"]["content"].as_str().map(|s| s.to_string()));

        ok_json(serde_json::json!({
            "status": "ok",
            "container": p.name,
            "port_superserver": port_superserver,
            "port_web": port_web,
            "namespace": p.namespace,
            "version": version,
            "note": "Restart session to use new container, or call iris_execute/iris_compile directly with the container's credentials.",
        }))
    }

    #[tool(description = "Start a dedicated IRIS container for the current project via iris-devtester CLI. Idempotent — returns existing container if already running.")]
    async fn iris_start_sandbox(&self, Parameters(p): Parameters<StartSandboxParams>) -> Result<CallToolResult, McpError> {
        let workspace = std::env::current_dir().unwrap_or_default();
        let workspace_basename = workspace.file_name().and_then(|n| n.to_str()).unwrap_or("project").to_string();
        let container_name = if p.name.is_empty() { format!("{}-iris", workspace_basename) } else { p.name.clone() };

        let containers = list_iris_containers(&workspace_basename).await;
        if let Some(c) = containers.iter().find(|c| c["name"].as_str() == Some(&container_name)) {
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
            .args(["container", "up", "--name", &container_name, "--edition", &p.edition])
            .output()
            .await;

        match output {
            Err(e) => err_json("INTERNAL_ERROR", &format!("idt not found: {e}. Install with: pip install iris-devtester")),
            Ok(out) if !out.status.success() => {
                let msg = String::from_utf8_lossy(&out.stderr);
                err_json("INTERNAL_ERROR", &format!("idt container up failed: {msg}"))
            }
            Ok(_) => {
                let containers2 = list_iris_containers(&workspace_basename).await;
                match containers2.iter().find(|c| c["name"].as_str() == Some(&container_name)) {
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
    async fn iris_symbols(&self, Parameters(p): Parameters<SymbolsParams>) -> Result<CallToolResult, McpError> {
        let iris = self.get_iris()?;
        let client = self.http_client();
        let sql = format!("SELECT TOP {} Name FROM %Dictionary.ClassDefinition WHERE Name LIKE ? ORDER BY Name", p.limit);
        match iris.query(&sql, vec![serde_json::Value::String(format!("%{}%", p.query))], &client).await {
            Ok(resp) => ok_json(serde_json::json!({"source": "iris_dictionary", "symbols": resp["result"]["content"], "count": resp["result"]["content"].as_array().map(|a| a.len()).unwrap_or(0)})),
            Err(e) => err_json("IRIS_UNREACHABLE", &e.to_string()),
        }
    }

    #[tool(description = "Search for ObjectScript symbols in local .cls files without IRIS connection.")]
    async fn iris_symbols_local(&self, Parameters(p): Parameters<SymbolsLocalParams>) -> Result<CallToolResult, McpError> {
        if std::env::var("IRIS_ISFS").as_deref() == Ok("true") {
            return ok_json(serde_json::json!({"error": "ISFS workspace detected — no local .cls files to parse. Use iris_symbols instead.", "isfs": true}));
        }
        ok_json(serde_json::json!({"source": "local_scan", "workspace": p.workspace_path.unwrap_or_else(|| ".".to_string()), "symbols": [], "note": "tree-sitter integration pending"}))
    }

    #[tool(description = "Introspect an ObjectScript class — returns methods, properties, and type information.")]
    async fn docs_introspect(&self, Parameters(p): Parameters<IntrospectParams>) -> Result<CallToolResult, McpError> {
        let iris = self.get_iris()?;
        let client = self.http_client();
        let cls = p.class_name.replace('\'', "''");
        let methods = iris.query(&format!("SELECT Name,FormalSpec,ReturnType FROM %Dictionary.CompiledMethod WHERE parent='{}'", cls), vec![], &client).await.unwrap_or_default();
        let props = iris.query(&format!("SELECT Name,Type FROM %Dictionary.CompiledProperty WHERE parent='{}'", cls), vec![], &client).await.unwrap_or_default();
        ok_json(serde_json::json!({"success": true, "class_name": p.class_name, "methods": methods["result"]["content"], "properties": props["result"]["content"]}))
    }

    #[tool(description = "Map a .INT routine offset to the original .CLS source line. Pass routine+offset OR a raw IRIS error string like '<UNDEFINED>x+3^MyApp.Foo.1'.")]
    async fn debug_map_int_to_cls(&self, Parameters(mut p): Parameters<DebugMapParams>) -> Result<CallToolResult, McpError> {
        if !p.error_string.is_empty() {
            if let Some((r, o)) = parse_iris_error_string(&p.error_string) { p.routine = r; p.offset = o; }
        }
        let iris = self.get_iris()?;
        let client = self.http_client();
        let code = format!("Write ##class(%Studio.Debugger).SourceLine(\"{}\",{})", p.routine.replace('"', "\\\""), p.offset);
        match iris.xecute(&code, &client).await {
            Ok(resp) => {
                let raw = resp["result"]["content"][0].as_str().unwrap_or("").to_string();
                let (cls_name, cls_line) = parse_source_line(&raw);
                ok_json(serde_json::json!({"success": true, "mapping_available": cls_name.is_some(), "cls_name": cls_name, "cls_line": cls_line, "routine": p.routine, "offset": p.offset, "raw_error": if p.error_string.is_empty() { serde_json::Value::Null } else { p.error_string.into() }}))
            }
            Err(e) => err_json("IRIS_UNREACHABLE", &e.to_string()),
        }
    }

    #[tool(description = "Capture IRIS error state and recent error log entries for debugging.")]
    async fn debug_capture_packet(&self, Parameters(p): Parameters<CapturePacketParams>) -> Result<CallToolResult, McpError> {
        let iris = self.get_iris()?;
        let client = self.http_client();
        match iris.query("SELECT TOP 20 ErrorCode,ErrorText,TimeStamp FROM %SYSTEM.Error ORDER BY TimeStamp DESC", vec![], &client).await {
            Ok(resp) => ok_json(serde_json::json!({"success": true, "errors": resp["result"]["content"]})),
            Err(e) => err_json("IRIS_UNREACHABLE", &e.to_string()),
        }
    }

    #[tool(description = "Retrieve recent IRIS error log entries.")]
    async fn debug_get_error_logs(&self, Parameters(p): Parameters<ErrorLogsParams>) -> Result<CallToolResult, McpError> {
        let iris = self.get_iris()?;
        let client = self.http_client();
        let sql = format!("SELECT TOP {} ErrorCode,ErrorText,TimeStamp FROM %SYSTEM.Error ORDER BY TimeStamp DESC", p.max_entries);
        match iris.query(&sql, vec![], &client).await {
            Ok(resp) => ok_json(serde_json::json!({"success": true, "logs": resp["result"]["content"]})),
            Err(e) => err_json("IRIS_UNREACHABLE", &e.to_string()),
        }
    }

    #[tool(description = "Build a .INT source map for a compiled ObjectScript class, mapping method labels in the compiled routine back to .CLS line numbers. Enables offline stack trace resolution without a live IRIS connection.")]
    async fn debug_source_map(&self, Parameters(p): Parameters<SourceMapParams>) -> Result<CallToolResult, McpError> {
        let iris = self.get_iris()?;
        let python_code = format!(
            "import json, os; \
             os.environ.setdefault('IRIS_HOST', 'localhost'); \
             os.environ.setdefault('IRIS_PORT', '{}'); \
             os.environ.setdefault('IRIS_USERNAME', '{}'); \
             os.environ.setdefault('IRIS_PASSWORD', '{}'); \
             from objectscript_mcp.handlers.debug_source_map import build_source_map; \
             from objectscript_mcp import connection; \
             import intersystems_iris as iris; \
             conn = iris.connect('localhost', int(os.environ['IRIS_PORT']), 'USER', os.environ['IRIS_USERNAME'], os.environ['IRIS_PASSWORD']); \
             result = build_source_map(cls_text={}, cls_name={}, conn=conn, workspace_path={}); \
             conn.close(); \
             print(json.dumps(result or {{'error': 'build_source_map returned None'}}))",
            iris.port_superserver.unwrap_or(1972),
            iris.username,
            iris.password,
            serde_json::to_string(&p.cls_text).unwrap_or_default(),
            serde_json::to_string(&p.cls_name).unwrap_or_default(),
            serde_json::to_string(&p.workspace_path).unwrap_or("null".to_string()),
        );
        let output = tokio::process::Command::new("python3")
            .args(["-c", &python_code])
            .output()
            .await
            .map_err(|e| McpError::internal_error(format!("python3 not available: {e}"), None))?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return err_json("INTERNAL_ERROR", &format!("python3 error: {stderr}"));
        }
        let stdout = String::from_utf8_lossy(&output.stdout);
        match serde_json::from_str::<serde_json::Value>(stdout.trim()) {
            Ok(v) => ok_json(v),
            Err(_) => err_json("INTERNAL_ERROR", &format!("unexpected output: {}", stdout.trim())),
        }
    }

    #[tool(description = "Generate an ObjectScript class from a natural language description. Requires IRIS_GENERATE_CLASS_MODEL + OPENAI_API_KEY env vars.")]
    async fn iris_generate_class(&self, Parameters(p): Parameters<GenerateClassParams>) -> Result<CallToolResult, McpError> {
        use crate::generate::{LlmClient, GENERATE_CLASS_SYSTEM, RETRY_TEMPLATE, validate_cls_syntax, extract_class_name};
        let llm = LlmClient::from_env().ok_or_else(|| McpError::invalid_request("LLM_UNAVAILABLE: Set IRIS_GENERATE_CLASS_MODEL and OPENAI_API_KEY", None))?;

        let class_text = llm.complete(GENERATE_CLASS_SYSTEM, &p.description).await
            .map_err(|e| McpError { code: rmcp::model::ErrorCode::INTERNAL_ERROR, message: format!("LLM_TIMEOUT: {}", e).into(), data: None })?;

        if !validate_cls_syntax(&class_text) {
            return ok_json(serde_json::json!({"success": false, "error_code": "INVALID_OUTPUT", "raw_llm_output": class_text}));
        }
        let class_name = extract_class_name(&class_text).unwrap_or_else(|| "Generated.Class".to_string());

        if let Some(iris) = self.iris.as_deref() {
            let client = self.http_client();
            let code = format!("Set sc=$SYSTEM.OBJ.Compile(\"{}\",\"ck-d\") Write $System.Status.IsOK(sc)", class_name);
            let compile_ok = iris.xecute(&code, &client).await
                .map(|r| r["result"]["content"][0].as_str().unwrap_or("0").trim() == "1")
                .unwrap_or(false);

            if !compile_ok {
                let retry_prompt = RETRY_TEMPLATE.replace("{errors}", "compilation failed");
                if let Ok(fixed) = llm.complete(GENERATE_CLASS_SYSTEM, &format!("{}

Original: {}", retry_prompt, class_text)).await {
                    let fixed_name = extract_class_name(&fixed).unwrap_or(class_name.clone());
                    let code2 = format!("Set sc=$SYSTEM.OBJ.Compile(\"{}\",\"ck-d\") Write $System.Status.IsOK(sc)", fixed_name);
                    let ok2 = iris.xecute(&code2, &client).await.map(|r| r["result"]["content"][0].as_str().unwrap_or("0").trim() == "1").unwrap_or(false);
                    return ok_json(serde_json::json!({"success": true, "class_name": fixed_name, "class_text": fixed, "compiled": ok2, "retried": true}));
                }
            }
            return ok_json(serde_json::json!({"success": true, "class_name": class_name, "class_text": class_text, "compiled": compile_ok, "retried": false}));
        }
        ok_json(serde_json::json!({"success": true, "class_name": class_name, "class_text": class_text, "compiled": false, "retried": false, "note": "No IRIS connection — could not compile"}))
    }

    #[tool(description = "Generate a %UnitTest.TestCase for an existing ObjectScript class. Introspects the class first. Requires IRIS_GENERATE_CLASS_MODEL + OPENAI_API_KEY.")]
    async fn iris_generate_test(&self, Parameters(p): Parameters<GenerateTestParams>) -> Result<CallToolResult, McpError> {
        use crate::generate::{LlmClient, GENERATE_TEST_SYSTEM, validate_cls_syntax, extract_class_name};
        let llm = LlmClient::from_env().ok_or_else(|| McpError::invalid_request("LLM_UNAVAILABLE: Set IRIS_GENERATE_CLASS_MODEL and OPENAI_API_KEY", None))?;

        let introspection_context = if let Some(iris) = self.iris.as_deref() {
            let client = self.http_client();
            let cls = p.class_name.replace("'", "''");
            let sql = format!("SELECT Name,FormalSpec,ReturnType FROM %Dictionary.CompiledMethod WHERE parent='{}' ORDER BY Name", cls);
            iris.query(&sql, vec![], &client).await
                .map(|r| format!("Class: {}
Methods:
{}", p.class_name, serde_json::to_string_pretty(&r["result"]["content"]).unwrap_or_default()))
                .unwrap_or_else(|_| format!("Class: {} (introspection unavailable)", p.class_name))
        } else {
            format!("Class: {} (no IRIS connection — generating scaffold)", p.class_name)
        };

        let prompt = format!("Generate tests for the following ObjectScript class:

{}", introspection_context);
        let test_text = llm.complete(GENERATE_TEST_SYSTEM, &prompt).await
            .map_err(|e| McpError { code: rmcp::model::ErrorCode::INTERNAL_ERROR, message: format!("LLM_TIMEOUT: {}", e).into(), data: None })?;

        if !validate_cls_syntax(&test_text) {
            return ok_json(serde_json::json!({"success": false, "error_code": "INVALID_OUTPUT", "raw_llm_output": test_text}));
        }
        let test_class_name = extract_class_name(&test_text).unwrap_or_else(|| format!("Test.{}", p.class_name));
        ok_json(serde_json::json!({"success": true, "class_name": p.class_name, "test_class_name": test_class_name, "test_text": test_text, "introspected": !introspection_context.contains("unavailable")}))
    }

    #[tool(description = "List all synthesized skills in the registry.")]
    async fn skill_list(&self, _: Parameters<serde_json::Value>) -> Result<CallToolResult, McpError> {
        if let Some(iris) = self.iris.as_deref() {
            let client = self.http_client();
            let code = "Set key=\"\" Set result=\"[\" For { Set key=$Order(^SKILLS(key)) Quit:key=\"\" Set skill=$Get(^SKILLS(key)) Set result=result_skill_\",\" } Set result=$Extract(result,1,$Length(result)-1)_\"]\" Write result";
            if let Ok(resp) = iris.xecute(code, &client).await {
                let raw = resp["result"]["content"][0].as_str().unwrap_or("[]");
                if let Ok(skills) = serde_json::from_str::<serde_json::Value>(raw) {
                    let count = skills.as_array().map(|a| a.len()).unwrap_or(0);
                    return ok_json(serde_json::json!({"skills": skills, "count": count}));
                }
            }
        }
        ok_json(serde_json::json!({"skills": [], "count": 0}))
    }

    #[tool(description = "Describe a skill by name.")]
    async fn skill_describe(&self, Parameters(p): Parameters<SkillNameParams>) -> Result<CallToolResult, McpError> {
        if let Some(iris) = self.iris.as_deref() {
            let client = self.http_client();
            let code = format!("Write $Get(^SKILLS(\"{}\"))", p.name.replace('"', "\\\""));
            if let Ok(resp) = iris.xecute(&code, &client).await {
                let raw = resp["result"]["content"][0].as_str().unwrap_or("{}");
                if let Ok(skill) = serde_json::from_str::<serde_json::Value>(raw) {
                    return ok_json(serde_json::json!({"success": true, "skill": skill}));
                }
            }
        }
        err_json("NOT_FOUND", &format!("Skill '{}' not found", p.name))
    }

    #[tool(description = "Search synthesized skills by name and description. Returns skills whose name or description contains the query terms.")]
    async fn skill_search(&self, Parameters(p): Parameters<SkillSearchParams>) -> Result<CallToolResult, McpError> {
        if let Some(iris) = self.iris.as_deref() {
            let client = self.http_client();
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
            if let Ok(resp) = iris.xecute(&code, &client).await {
                let raw = resp["result"]["content"][0].as_str().unwrap_or("[]");
                if let Ok(skills) = serde_json::from_str::<Vec<serde_json::Value>>(raw) {
                    let limited: Vec<_> = skills.into_iter().take(p.top_k).collect();
                    let count = limited.len();
                    return ok_json(serde_json::json!({"query": p.query, "results": limited, "count": count}));
                }
            }
        }
        ok_json(serde_json::json!({"query": p.query, "results": [], "count": 0}))
    }

    #[tool(description = "Remove a skill from the registry by name.")]
    async fn skill_forget(&self, Parameters(p): Parameters<SkillNameParams>) -> Result<CallToolResult, McpError> {
        if let Some(iris) = self.iris.as_deref() {
            let client = self.http_client();
            let code = format!("Kill ^SKILLS(\"{}\") Write \"OK\"", p.name.replace('"', "\\\""));
            if iris.xecute(&code, &client).await.is_ok() {
                return ok_json(serde_json::json!({"success": true, "name": p.name}));
            }
        }
        err_json("IRIS_UNREACHABLE", "Cannot reach IRIS to delete skill")
    }

    #[tool(description = "Trigger pattern miner to synthesize new skills from recorded tool calls.")]
    async fn skill_propose(&self, _: Parameters<serde_json::Value>) -> Result<CallToolResult, McpError> {
        ok_json(serde_json::json!({"triggered": true, "note": "pattern mining pending full learning agent port"}))
    }

    #[tool(description = "Optimize a skill using DSPy. Requires OBJECTSCRIPT_DSPY=true.")]
    async fn skill_optimize(&self, Parameters(_p): Parameters<SkillNameParams>) -> Result<CallToolResult, McpError> {
        err_json("NOT_AVAILABLE", "DSPy optimization requires OBJECTSCRIPT_DSPY=true")
    }

    #[tool(description = "Share a skill to the community via GitHub PR.")]
    async fn skill_share(&self, Parameters(_p): Parameters<SkillNameParams>) -> Result<CallToolResult, McpError> {
        err_json("NOT_IMPLEMENTED", "Skill sharing pending GitHub integration")
    }

    #[tool(description = "List all skills loaded from --subscribe packages. Use --subscribe owner/repo when starting iris-dev mcp to load community skills.")]
    async fn skill_community_list(&self, _: Parameters<serde_json::Value>) -> Result<CallToolResult, McpError> {
        let skills: Vec<_> = self.registry.list_skills().iter().map(|s| serde_json::json!({
            "name": s.name,
            "description": s.description,
            "source": s.source_repo,
        })).collect();
        let kb_items: Vec<_> = self.registry.list_kb_items().iter().map(|k| serde_json::json!({
            "title": k.title,
            "source": k.source_repo,
        })).collect();
        ok_json(serde_json::json!({
            "skills": skills,
            "kb_items": kb_items,
            "skill_count": skills.len(),
            "kb_count": kb_items.len(),
            "hint": "Start iris-dev mcp with --subscribe owner/repo to load community packages"
        }))
    }

    #[tool(description = "Install a community skill from the GitHub community repo.")]
    async fn skill_community_install(&self, Parameters(_p): Parameters<CommunityPkgParams>) -> Result<CallToolResult, McpError> {
        err_json("NOT_IMPLEMENTED", "Community skill installation pending")
    }

    #[tool(description = "Index markdown files into the IRIS knowledge base for semantic search.")]
    async fn kb_index(&self, Parameters(p): Parameters<KbIndexParams>) -> Result<CallToolResult, McpError> {
        ok_json(serde_json::json!({"indexed": 0, "workspace": p.workspace_path.unwrap_or_else(|| ".".to_string()), "note": "KB indexing pending IRIS vector store integration"}))
    }

    #[tool(description = "Search the knowledge base for relevant guidance. Searches subscribed KB packages and any indexed content.")]
    async fn kb_recall(&self, Parameters(p): Parameters<KbRecallParams>) -> Result<CallToolResult, McpError> {
        let q = p.query.to_lowercase();
        let mut results: Vec<serde_json::Value> = vec![];

        // Search subscribed KB items (BM25 substring match)
        for item in self.registry.list_kb_items() {
            let content_lower = item.content.to_lowercase();
            if content_lower.contains(&q) || item.title.to_lowercase().contains(&q) {
                // Extract a relevant snippet around the match
                let snippet = content_lower.find(&q)
                    .and_then(|pos| {
                        let start = pos.saturating_sub(150);
                        let end = (pos + q.len() + 300).min(item.content.len());
                        item.content.get(start..end)
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
        results.sort_by(|a, b| b["score"].as_f64().unwrap_or(0.0)
            .partial_cmp(&a["score"].as_f64().unwrap_or(0.0))
            .unwrap_or(std::cmp::Ordering::Equal));
        results.truncate(p.top_k);

        let count = results.len();
        ok_json(serde_json::json!({"query": p.query, "results": results, "count": count}))
    }

    #[tool(description = "Return recent tool call history for this session.")]
    async fn agent_history(&self, Parameters(p): Parameters<AgentHistoryParams>) -> Result<CallToolResult, McpError> {
        ok_json(serde_json::json!({"calls": [], "limit": p.limit, "note": "history recording pending"}))
    }

    #[tool(description = "Return learning agent status: skill count, pattern count, KB size.")]
    async fn agent_stats(&self, _: Parameters<serde_json::Value>) -> Result<CallToolResult, McpError> {
        let mut skill_count = serde_json::Value::Null;
        if let Some(iris) = self.iris.as_deref() {
            let client = self.http_client();
            let code = "Set count=0,key=\"\" For { Set key=$Order(^SKILLS(key)) Quit:key=\"\" Set count=count+1 } Write count";
            if let Ok(resp) = iris.xecute(code, &client).await {
                if let Some(n) = resp["result"]["content"][0].as_str().and_then(|s| s.trim().parse::<u64>().ok()) {
                    skill_count = serde_json::Value::Number(n.into());
                }
            }
        }
        ok_json(serde_json::json!({"status": "ok", "skill_count": skill_count, "pattern_count": null, "learning_enabled": false}))
    }

    #[tool(description = "Returns the current state of the running IRIS Interoperability production. With full_status=true, includes per-component breakdown.")]
    async fn interop_production_status(&self, Parameters(p): Parameters<interop::ProductionStatusParams>) -> Result<CallToolResult, McpError> {
        interop::interop_production_status_impl(self.iris.as_deref(), p).await
    }

    #[tool(description = "Start a named IRIS Interoperability production.")]
    async fn interop_production_start(&self, Parameters(p): Parameters<interop::ProductionNameParams>) -> Result<CallToolResult, McpError> {
        interop::interop_production_start_impl(self.iris.as_deref(), p).await
    }

    #[tool(description = "Stop the running IRIS Interoperability production with optional timeout and force.")]
    async fn interop_production_stop(&self, Parameters(p): Parameters<interop::ProductionStopParams>) -> Result<CallToolResult, McpError> {
        interop::interop_production_stop_impl(self.iris.as_deref(), p).await
    }

    #[tool(description = "Hot-apply configuration changes to the running production.")]
    async fn interop_production_update(&self, Parameters(p): Parameters<interop::ProductionUpdateParams>) -> Result<CallToolResult, McpError> {
        interop::interop_production_update_impl(self.iris.as_deref(), p).await
    }

    #[tool(description = "Check if the production configuration has changed and needs to be updated.")]
    async fn interop_production_needs_update(&self, _: Parameters<serde_json::Value>) -> Result<CallToolResult, McpError> {
        interop::interop_production_needs_update_impl(self.iris.as_deref()).await
    }

    #[tool(description = "Recover a troubled IRIS Interoperability production.")]
    async fn interop_production_recover(&self, _: Parameters<serde_json::Value>) -> Result<CallToolResult, McpError> {
        interop::interop_production_recover_impl(self.iris.as_deref()).await
    }

    #[tool(description = "Get recent Interoperability production log entries. Filter by log_type (comma-separated: error,warning,info,alert) and component name.")]
    async fn interop_logs(&self, Parameters(p): Parameters<interop::LogsParams>) -> Result<CallToolResult, McpError> {
        interop::interop_logs_impl(self.iris.as_deref(), p).await
    }

    #[tool(description = "Get all current Interoperability message queues and their depths.")]
    async fn interop_queues(&self, _: Parameters<serde_json::Value>) -> Result<CallToolResult, McpError> {
        interop::interop_queues_impl(self.iris.as_deref()).await
    }

    #[tool(description = "Search the Interoperability message archive by source, target, or message class.")]
    async fn interop_message_search(&self, Parameters(p): Parameters<interop::MessageSearchParams>) -> Result<CallToolResult, McpError> {
        interop::interop_message_search_impl(self.iris.as_deref(), p).await
    }
}


#[tool_handler]
impl ServerHandler for IrisTools {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::from_build_env())
            .with_instructions("iris-dev: 23 tools for ObjectScript and IRIS development.".to_string())
    }
}

fn parse_iris_error_string(s: &str) -> Option<(String, i64)> {
    let re = regex::Regex::new(r"<[A-Z]+>\s*[^+\s]+\+(\d+)\^([\w.%]+)").ok()?;
    let caps = re.captures(s)?;
    Some((caps[2].to_string(), caps[1].parse().ok()?))
}

fn parse_source_line(raw: &str) -> (Option<String>, Option<i64>) {
    if raw.is_empty() { return (None, None); }
    if let Some((cls, line)) = raw.split_once(':') {
        return (Some(cls.trim_end_matches(".cls").to_string()), line.trim().parse().ok());
    }
    (None, None)
}
