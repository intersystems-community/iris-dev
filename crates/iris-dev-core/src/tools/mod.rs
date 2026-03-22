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
use crate::iris::connection::IrisConnection;

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

fn default_flags() -> String { "cuk".to_string() }
fn default_namespace() -> String { "USER".to_string() }
fn default_limit() -> usize { 20 }
fn default_max_entries() -> usize { 50 }

fn iris_unreachable() -> McpError {
    McpError::invalid_request("IRIS_UNREACHABLE: no IRIS connection available", None)
}
fn ok_json(v: serde_json::Value) -> Result<CallToolResult, McpError> {
    Ok(CallToolResult::success(vec![Content::text(v.to_string())]))
}
fn err_json(code: &str, msg: &str) -> Result<CallToolResult, McpError> {
    ok_json(serde_json::json!({"success": false, "error_code": code, "error": msg}))
}
fn is_network_error(msg: &str) -> bool {
    msg.contains("error sending request") || msg.contains("connection") || msg.contains("dns")
}

#[derive(Clone)]
pub struct IrisTools {
    pub iris: Option<Arc<IrisConnection>>,
    pub registry: Arc<crate::skills::SkillRegistry>,
    tool_router: ToolRouter<IrisTools>,
}

#[tool_router]
impl IrisTools {
    pub fn new(iris: Option<IrisConnection>) -> Self {
        Self { iris: iris.map(Arc::new), registry: Arc::new(crate::skills::SkillRegistry::new()), tool_router: Self::tool_router() }
    }
    pub fn with_registry(iris: Option<IrisConnection>, registry: crate::skills::SkillRegistry) -> Self {
        Self { iris: iris.map(Arc::new), registry: Arc::new(registry), tool_router: Self::tool_router() }
    }
    fn get_iris(&self) -> Result<&IrisConnection, McpError> {
        self.iris.as_deref().ok_or_else(iris_unreachable)
    }
    fn http_client(&self) -> reqwest::Client {
        IrisConnection::http_client().unwrap_or_default()
    }

    #[tool(description = "Compile an ObjectScript class or .cls file on IRIS. Pass class name or file path. Returns compiler output with errors.")]
    async fn iris_compile(&self, Parameters(p): Parameters<CompileParams>) -> Result<CallToolResult, McpError> {
        let iris = self.get_iris()?;
        let client = self.http_client();
        let code = format!(
            "Set sc=$SYSTEM.OBJ.Compile(\"{}\",\"{}\")\nIf $System.Status.IsOK(sc) {{Write \"OK\"}} Else {{Write $System.Status.GetErrorText(sc)}}",
            p.target.replace('"', "\\\""), p.flags
        );
        match iris.xecute(&code, &client).await {
            Ok(resp) => ok_json(serde_json::json!({"success": true, "target": p.target, "namespace": p.namespace, "result": resp["result"]})),
            Err(e) => err_json(if is_network_error(&e.to_string()) { "IRIS_UNREACHABLE" } else { "IRIS_COMPILE_FAILED" }, &e.to_string()),
        }
    }

    #[tool(description = "Run %UnitTest tests matching a pattern on IRIS. Returns pass/fail counts and error details.")]
    async fn iris_test(&self, Parameters(p): Parameters<TestParams>) -> Result<CallToolResult, McpError> {
        let iris = self.get_iris()?;
        let client = self.http_client();
        let code = format!("Do ##class(%UnitTest.Manager).RunTest(\"{}\",\"/noload\")", p.pattern.replace('"', "\\\""));
        match iris.xecute(&code, &client).await {
            Ok(resp) => ok_json(serde_json::json!({"success": true, "pattern": p.pattern, "result": resp})),
            Err(e) => err_json("IRIS_TEST_FAILED", &e.to_string()),
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
    async fn skill_optimize(&self, Parameters(p): Parameters<SkillNameParams>) -> Result<CallToolResult, McpError> {
        err_json("NOT_AVAILABLE", "DSPy optimization requires OBJECTSCRIPT_DSPY=true")
    }

    #[tool(description = "Share a skill to the community via GitHub PR.")]
    async fn skill_share(&self, Parameters(p): Parameters<SkillNameParams>) -> Result<CallToolResult, McpError> {
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
    async fn skill_community_install(&self, Parameters(p): Parameters<CommunityPkgParams>) -> Result<CallToolResult, McpError> {
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
