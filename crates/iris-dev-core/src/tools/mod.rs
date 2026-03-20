//! All 23 iris-dev MCP tools registered via rmcp #[tool_router].

use rmcp::{
    ServerHandler, RoleServer,
    model::*,
    tool, tool_handler, tool_router,
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    service::RequestContext,
    ErrorData as McpError,
};
use serde::{Deserialize, Serialize};
use schemars::JsonSchema;
use std::sync::Arc;
use crate::iris::connection::IrisConnection;

// ── Input schemas ──────────────────────────────────────────────────────────

#[derive(Debug, Deserialize, JsonSchema)]
pub struct CompileParams {
    /// Class name (e.g. "MyApp.Patient") or path to .cls file
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
    /// %UnitTest pattern e.g. "Test.*" or specific class
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

fn default_flags() -> String { "cuk".to_string() }
fn default_namespace() -> String { "USER".to_string() }
fn default_limit() -> usize { 20 }
fn default_max_entries() -> usize { 50 }

// ── Helper ────────────────────────────────────────────────────────────────

fn iris_unreachable() -> McpError {
    McpError::invalid_request("IRIS_UNREACHABLE: no IRIS connection available", None)
}

fn ok_json(value: serde_json::Value) -> Result<CallToolResult, McpError> {
    Ok(CallToolResult::success(vec![Content::text(value.to_string())]))
}

fn err_json(error_code: &str, message: &str) -> Result<CallToolResult, McpError> {
    ok_json(serde_json::json!({"success": false, "error_code": error_code, "error": message}))
}

// ── IrisTools ─────────────────────────────────────────────────────────────

#[derive(Clone)]
pub struct IrisTools {
    pub iris: Option<Arc<IrisConnection>>,
    tool_router: ToolRouter<IrisTools>,
}

#[tool_router]
impl IrisTools {
    pub fn new(iris: Option<IrisConnection>) -> Self {
        Self {
            iris: iris.map(Arc::new),
            tool_router: Self::tool_router(),
        }
    }

    fn get_iris(&self) -> Result<&IrisConnection, McpError> {
        self.iris.as_deref().ok_or_else(iris_unreachable)
    }

    fn http_client(&self) -> reqwest::Client {
        IrisConnection::http_client().unwrap_or_default()
    }

    // ── IRIS tools ──────────────────────────────────────────────────────

    #[tool(description = "Compile an ObjectScript class or .cls file on IRIS. Pass class name (e.g. 'MyApp.Patient') or .cls file path. Returns compiler output with any errors.")]
    async fn iris_compile(&self, Parameters(p): Parameters<CompileParams>) -> Result<CallToolResult, McpError> {
        let iris = self.get_iris()?;
        let client = self.http_client();
        // Use xecute to compile
        let code = format!(
            "Set sc=$SYSTEM.OBJ.Compile(\"{}\",\"{}\")\nIf $System.Status.IsOK(sc) {{Write \"OK\"}} Else {{Write $System.Status.GetErrorText(sc)}}",
            p.target.replace("\"", "\\\""), p.flags
        );
        match iris.xecute(&code, &client).await {
            Ok(resp) => ok_json(serde_json::json!({
                "success": true, "target": p.target, "namespace": p.namespace,
                "result": resp["result"]
            })),
            Err(e) => {
                let msg = e.to_string();
                let code = if msg.contains("error sending request") || msg.contains("connection") || msg.contains("dns") {
                    "IRIS_UNREACHABLE"
                } else {
                    "IRIS_COMPILE_FAILED"
                };
                err_json(code, &msg)
            }
        }
    }

    #[tool(description = "Run %UnitTest tests matching a pattern on IRIS. Returns pass/fail counts and error details.")]
    async fn iris_test(&self, Parameters(p): Parameters<TestParams>) -> Result<CallToolResult, McpError> {
        let iris = self.get_iris()?;
        let client = self.http_client();
        let code = format!(
            "Do ##class(%UnitTest.Manager).RunTest(\"{}\",\"/noload\")",
            p.pattern.replace("\"", "\\\"")
        );
        match iris.xecute(&code, &client).await {
            Ok(resp) => ok_json(serde_json::json!({"success": true, "pattern": p.pattern, "result": resp})),
            Err(e) => err_json("IRIS_TEST_FAILED", &e.to_string()),
        }
    }

    #[tool(description = "Search for ObjectScript classes matching a query in the IRIS namespace. Returns class names and types.")]
    async fn iris_symbols(&self, Parameters(p): Parameters<SymbolsParams>) -> Result<CallToolResult, McpError> {
        let iris = self.get_iris()?;
        let client = self.http_client();
        let pattern = format!("%{}%", p.query);
        let sql = format!("SELECT TOP {} Name FROM %Dictionary.ClassDefinition WHERE Name LIKE ? ORDER BY Name", p.limit);
        match iris.query(&sql, vec![serde_json::Value::String(pattern)], &client).await {
            Ok(resp) => ok_json(serde_json::json!({
                "source": "iris_dictionary",
                "symbols": resp["result"]["content"],
                "count": resp["result"]["content"].as_array().map(|a| a.len()).unwrap_or(0)
            })),
            Err(e) => err_json("IRIS_UNREACHABLE", &e.to_string()),
        }
    }

    #[tool(description = "Search for ObjectScript symbols in local .cls files without IRIS connection. Uses tree-sitter parsing.")]
    async fn iris_symbols_local(&self, Parameters(p): Parameters<SymbolsLocalParams>) -> Result<CallToolResult, McpError> {
        let workspace = p.workspace_path.as_deref()
            .or_else(|| std::env::var("OBJECTSCRIPT_WORKSPACE").ok().as_deref().map(|_| ""))
            .unwrap_or(".");
        // TODO: integrate tree-sitter-objectscript for offline parsing
        ok_json(serde_json::json!({
            "source": "local_scan",
            "workspace": workspace,
            "symbols": [],
            "error": "tree-sitter integration pending"
        }))
    }

    #[tool(description = "Introspect an ObjectScript class — returns methods, properties, parameters, and type information.")]
    async fn docs_introspect(&self, Parameters(p): Parameters<IntrospectParams>) -> Result<CallToolResult, McpError> {
        let iris = self.get_iris()?;
        let client = self.http_client();
        let methods_sql = format!(
            "SELECT Name, FormalSpec, ReturnType, Description FROM %Dictionary.CompiledMethod WHERE parent='{}' ORDER BY Name",
            p.class_name.replace("'", "''")
        );
        let props_sql = format!(
            "SELECT Name, Type, Description FROM %Dictionary.CompiledProperty WHERE parent='{}' ORDER BY Name",
            p.class_name.replace("'", "''")
        );
        let methods = iris.query(&methods_sql, vec![], &client).await.unwrap_or_default();
        let props = iris.query(&props_sql, vec![], &client).await.unwrap_or_default();
        ok_json(serde_json::json!({
            "success": true,
            "class_name": p.class_name,
            "methods": methods["result"]["content"],
            "properties": props["result"]["content"]
        }))
    }

    #[tool(description = "Map a .INT routine offset to the original .CLS source line. Pass routine+offset OR a raw IRIS error string like '<UNDEFINED>x+3^MyApp.Foo.1'.")]
    async fn debug_map_int_to_cls(&self, Parameters(mut p): Parameters<DebugMapParams>) -> Result<CallToolResult, McpError> {
        // Parse error_string if provided
        if !p.error_string.is_empty() {
            if let Some((routine, offset)) = parse_iris_error_string(&p.error_string) {
                p.routine = routine;
                p.offset = offset;
            }
        }
        let iris = self.get_iris()?;
        let client = self.http_client();
        let code = format!(
            "Write ##class(%Studio.Debugger).SourceLine(\"{}\",{})",
            p.routine.replace("\"", "\\\""), p.offset
        );
        match iris.xecute(&code, &client).await {
            Ok(resp) => {
                let raw = resp["result"]["content"][0].as_str().unwrap_or("").to_string();
                let (cls_name, cls_line) = parse_source_line(&raw);
                ok_json(serde_json::json!({
                    "success": true,
                    "mapping_available": cls_name.is_some(),
                    "cls_name": cls_name,
                    "cls_line": cls_line,
                    "routine": p.routine,
                    "offset": p.offset,
                    "raw_error": if p.error_string.is_empty() { serde_json::Value::Null } else { p.error_string.into() }
                }))
            },
            Err(e) => err_json("IRIS_UNREACHABLE", &e.to_string()),
        }
    }

    #[tool(description = "Capture IRIS error state and recent error log entries for debugging.")]
    async fn debug_capture_packet(&self, Parameters(p): Parameters<CapturePacketParams>) -> Result<CallToolResult, McpError> {
        let iris = self.get_iris()?;
        let client = self.http_client();
        let sql = "SELECT TOP 20 ErrorCode, ErrorText, TimeStamp FROM %SYSTEM.Error ORDER BY TimeStamp DESC";
        match iris.query(sql, vec![], &client).await {
            Ok(resp) => ok_json(serde_json::json!({"success": true, "errors": resp["result"]["content"]})),
            Err(e) => err_json("IRIS_UNREACHABLE", &e.to_string()),
        }
    }

    #[tool(description = "Retrieve recent IRIS error log entries from messages.log and ^ERRORS global.")]
    async fn debug_get_error_logs(&self, Parameters(p): Parameters<ErrorLogsParams>) -> Result<CallToolResult, McpError> {
        let iris = self.get_iris()?;
        let client = self.http_client();
        let sql = format!("SELECT TOP {} ErrorCode, ErrorText, TimeStamp FROM %SYSTEM.Error ORDER BY TimeStamp DESC", p.max_entries);
        match iris.query(&sql, vec![], &client).await {
            Ok(resp) => ok_json(serde_json::json!({"success": true, "logs": resp["result"]["content"]})),
            Err(e) => err_json("IRIS_UNREACHABLE", &e.to_string()),
        }
    }

    #[tool(description = "Generate an ObjectScript class from a natural language description using LLM. Requires IRIS_GENERATE_CLASS_MODEL env var.")]
    async fn iris_generate_class(&self, Parameters(p): Parameters<GenerateClassParams>) -> Result<CallToolResult, McpError> {
        let model = std::env::var("IRIS_GENERATE_CLASS_MODEL").ok();
        if model.is_none() {
            return err_json("LLM_UNAVAILABLE", "Set IRIS_GENERATE_CLASS_MODEL env var to enable class generation");
        }
        // TODO: implement LLM call + compile-retry loop
        err_json("NOT_IMPLEMENTED", "LLM class generation pending implementation")
    }

    #[tool(description = "Generate a %UnitTest.TestCase class for an existing ObjectScript class. Introspects the class first. Requires IRIS_GENERATE_CLASS_MODEL.")]
    async fn iris_generate_test(&self, Parameters(p): Parameters<GenerateTestParams>) -> Result<CallToolResult, McpError> {
        let model = std::env::var("IRIS_GENERATE_CLASS_MODEL").ok();
        if model.is_none() {
            return err_json("LLM_UNAVAILABLE", "Set IRIS_GENERATE_CLASS_MODEL env var to enable test generation");
        }
        err_json("NOT_IMPLEMENTED", "LLM test generation pending implementation")
    }

    // ── Skill tools ─────────────────────────────────────────────────────

    #[tool(description = "List all synthesized skills in the registry.")]
    async fn skill_list(&self, _: Parameters<serde_json::Value>) -> Result<CallToolResult, McpError> {
        ok_json(serde_json::json!({"skills": [], "count": 0, "note": "skill registry pending IRIS persistence"}))
    }

    #[tool(description = "Describe a skill by name — returns its full definition and steps.")]
    async fn skill_describe(&self, Parameters(p): Parameters<SkillNameParams>) -> Result<CallToolResult, McpError> {
        ok_json(serde_json::json!({"name": p.name, "error": "skill not found"}))
    }

    #[tool(description = "Semantic search over synthesized skills.")]
    async fn skill_search(&self, Parameters(p): Parameters<SkillSearchParams>) -> Result<CallToolResult, McpError> {
        ok_json(serde_json::json!({"query": p.query, "results": [], "count": 0}))
    }

    #[tool(description = "Remove a skill from the registry by name.")]
    async fn skill_forget(&self, Parameters(p): Parameters<SkillNameParams>) -> Result<CallToolResult, McpError> {
        ok_json(serde_json::json!({"success": false, "name": p.name, "error": "skill registry pending"}))
    }

    #[tool(description = "Trigger the pattern miner to synthesize new skills from recorded tool calls.")]
    async fn skill_propose(&self, _: Parameters<serde_json::Value>) -> Result<CallToolResult, McpError> {
        ok_json(serde_json::json!({"triggered": true, "note": "skill synthesis pending IRIS persistence"}))
    }

    #[tool(description = "Optimize a skill using DSPy. Requires OBJECTSCRIPT_DSPY=true.")]
    async fn skill_optimize(&self, Parameters(p): Parameters<SkillNameParams>) -> Result<CallToolResult, McpError> {
        err_json("NOT_AVAILABLE", "DSPy optimization requires OBJECTSCRIPT_DSPY=true")
    }

    #[tool(description = "Share a skill to the community by opening a GitHub PR.")]
    async fn skill_share(&self, Parameters(p): Parameters<SkillNameParams>) -> Result<CallToolResult, McpError> {
        err_json("NOT_IMPLEMENTED", "Skill sharing pending GitHub integration")
    }

    #[tool(description = "List community-contributed skills available on the community GitHub repo.")]
    async fn skill_community_list(&self, _: Parameters<serde_json::Value>) -> Result<CallToolResult, McpError> {
        // TODO: fetch from GitHub API
        ok_json(serde_json::json!({"skills": [], "note": "community skill listing pending"}))
    }

    #[tool(description = "Install a community skill from the GitHub community repo.")]
    async fn skill_community_install(&self, Parameters(p): Parameters<SkillNameParams>) -> Result<CallToolResult, McpError> {
        err_json("NOT_IMPLEMENTED", "Community skill installation pending")
    }

    // ── KB tools ─────────────────────────────────────────────────────────

    #[tool(description = "Index markdown files into the IRIS knowledge base for semantic search.")]
    async fn kb_index(&self, Parameters(p): Parameters<KbIndexParams>) -> Result<CallToolResult, McpError> {
        let ws = p.workspace_path.as_deref().unwrap_or(".");
        ok_json(serde_json::json!({"indexed": 0, "workspace": ws, "note": "KB indexing pending IRIS vector store"}))
    }

    #[tool(description = "Hybrid BM25 + semantic search over the indexed knowledge base.")]
    async fn kb_recall(&self, Parameters(p): Parameters<KbRecallParams>) -> Result<CallToolResult, McpError> {
        ok_json(serde_json::json!({"query": p.query, "results": [], "count": 0, "note": "KB search pending IRIS vector store"}))
    }

    // ── Agent tools ───────────────────────────────────────────────────────

    #[tool(description = "Return recent tool call history for this session.")]
    async fn agent_history(&self, Parameters(p): Parameters<AgentHistoryParams>) -> Result<CallToolResult, McpError> {
        ok_json(serde_json::json!({"calls": [], "limit": p.limit, "note": "history recording pending"}))
    }

    #[tool(description = "Return learning agent status: call count, skill count, pattern count, KB size.")]
    async fn agent_stats(&self, _: Parameters<serde_json::Value>) -> Result<CallToolResult, McpError> {
        ok_json(serde_json::json!({
            "status": "ok",
            "skill_count": null,
            "pattern_count": null,
            "learning_enabled": false,
            "note": "stats pending IRIS persistence layer"
        }))
    }
}

#[tool_handler]
impl ServerHandler for IrisTools {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(
            ServerCapabilities::builder().enable_tools().build(),
        )
        .with_server_info(Implementation::from_build_env())
        .with_instructions("iris-dev MCP server: 23 tools for ObjectScript and IRIS development.".to_string())
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────

fn parse_iris_error_string(s: &str) -> Option<(String, i64)> {
    let re = regex::Regex::new(r"<[A-Z]+>\s*[^+\s]+\+(\d+)\^([\w.%]+)").ok()?;
    let caps = re.captures(s)?;
    let offset: i64 = caps[1].parse().ok()?;
    let routine = caps[2].to_string();
    Some((routine, offset))
}

fn parse_source_line(raw: &str) -> (Option<String>, Option<i64>) {
    // Format: "ClassName.cls:LineNumber"
    if raw.is_empty() { return (None, None); }
    if let Some((cls_part, line_part)) = raw.split_once(':') {
        let cls = cls_part.trim_end_matches(".cls").to_string();
        let line = line_part.trim().parse::<i64>().ok();
        return (Some(cls), line);
    }
    (None, None)
}
