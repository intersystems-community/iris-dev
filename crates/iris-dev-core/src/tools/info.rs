//! iris_info — namespace/document discovery via Atelier REST.
//! iris_macro — macro introspection.
//! iris_debug — debug tools via Atelier xecute + SQL.
//! iris_generate — LLM-based class/test generation.

use schemars::JsonSchema;
use serde::Deserialize;
use crate::iris::connection::IrisConnection;

fn ok_json(v: serde_json::Value) -> Result<rmcp::model::CallToolResult, rmcp::ErrorData> {
    Ok(rmcp::model::CallToolResult::success(vec![rmcp::model::Content::text(v.to_string())]))
}
fn err_json(code: &str, msg: &str) -> Result<rmcp::model::CallToolResult, rmcp::ErrorData> {
    ok_json(serde_json::json!({"success": false, "error_code": code, "error": msg}))
}
fn default_namespace() -> String { "USER".to_string() }
fn default_limit() -> usize { 20 }

// ── iris_info ────────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize, JsonSchema)]
pub struct InfoParams {
    /// What to fetch: documents, modified, namespace, metadata, jobs, csp_apps, csp_debug, sa_schema
    pub what: String,
    /// Document type filter for what=documents: CLS, MAC, INT, INC, CSP, ALL
    pub doc_type: Option<String>,
    /// Schema/cube name for what=sa_schema
    pub name: Option<String>,
    #[serde(default = "default_namespace")]
    pub namespace: String,
}

pub async fn handle_iris_info(
    iris: &IrisConnection,
    client: &reqwest::Client,
    p: InfoParams,
) -> Result<rmcp::model::CallToolResult, rmcp::ErrorData> {
    let ns = &p.namespace;
    let url = match p.what.as_str() {
        "documents" => {
            let cat = p.doc_type.as_deref().unwrap_or("ALL");
            iris.atelier_url(&format!("/v8/{}/docs?category={}", ns, cat))
        }
        "modified" => iris.atelier_url(&format!("/v8/{}/docs/modified", ns)),
        "namespace" => iris.atelier_url(&format!("/v8/{}/", ns)),
        "metadata" => iris.atelier_url(&format!("/v8/{}/metadata", ns)),
        "jobs" => iris.atelier_url(&format!("/v8/{}/jobs", ns)),
        "csp_apps" => iris.atelier_url(&format!("/v8/{}/cspapps", ns)),
        "csp_debug" => iris.atelier_url(&format!("/v8/{}/cspdebugid", ns)),
        "sa_schema" => {
            let name = p.name.as_deref().unwrap_or("");
            iris.atelier_url(&format!("/v8/{}/saschema/{}", ns, urlencoding::encode(name)))
        }
        other => return err_json("INVALID_PARAM", &format!("Unknown what='{}'. Use: documents, modified, namespace, metadata, jobs, csp_apps, csp_debug, sa_schema", other)),
    };

    let resp = client.get(&url)
        .basic_auth(&iris.username, Some(&iris.password))
        .send().await
        .map_err(|e| rmcp::ErrorData::internal_error(format!("HTTP error: {e}"), None))?;

    if !resp.status().is_success() {
        return err_json("IRIS_UNREACHABLE", &format!("HTTP {} for {}", resp.status(), url));
    }

    let body: serde_json::Value = resp.json().await.unwrap_or_default();
    ok_json(serde_json::json!({"success": true, "what": p.what, "namespace": p.namespace, "result": body["result"]}))
}

// ── iris_macro ───────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize, JsonSchema)]
pub struct MacroParams {
    /// Action: list, signature, location, definition, expand
    pub action: String,
    pub name: Option<String>,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default = "default_namespace")]
    pub namespace: String,
}

pub async fn handle_iris_macro(
    iris: &IrisConnection,
    client: &reqwest::Client,
    p: MacroParams,
) -> Result<rmcp::model::CallToolResult, rmcp::ErrorData> {
    match p.action.as_str() {
        "list" => {
            let url = iris.atelier_url(&format!("/v8/{}/macros", p.namespace));
            let resp = client.get(&url).basic_auth(&iris.username, Some(&iris.password)).send().await
                .map_err(|e| rmcp::ErrorData::internal_error(format!("HTTP error: {e}"), None))?;
            let body: serde_json::Value = resp.json().await.unwrap_or_default();
            ok_json(serde_json::json!({"success": true, "macros": body["result"]["content"]}))
        }
        action @ ("signature" | "location" | "definition" | "expand") => {
            let name = p.name.as_deref().unwrap_or("");
            let url = iris.atelier_url(&format!("/v8/{}/action/getmacro", p.namespace));
            let arg_count = p.args.len();
            let resp = client.post(&url)
                .basic_auth(&iris.username, Some(&iris.password))
                .json(&serde_json::json!({
                    "macros": [{"name": name, "arguments": arg_count}],
                    "action": action,
                    "args": p.args,
                }))
                .send().await
                .map_err(|e| rmcp::ErrorData::internal_error(format!("HTTP error: {e}"), None))?;
            let body: serde_json::Value = resp.json().await.unwrap_or_default();
            ok_json(serde_json::json!({"success": true, "name": name, "action": action, "result": body["result"]}))
        }
        other => err_json("INVALID_PARAM", &format!("Unknown action='{}'. Use: list, signature, location, definition, expand", other)),
    }
}

// ── iris_debug ───────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize, JsonSchema)]
pub struct DebugParams {
    /// Action: map_int, error_logs, capture, source_map
    pub action: String,
    /// Error string for map_int e.g. "<UNDEFINED>x+3^MyApp.Foo.1"
    pub error_string: Option<String>,
    /// Class name for source_map
    pub class_name: Option<String>,
    #[serde(default = "default_limit")]
    pub limit: usize,
    #[serde(default = "default_namespace")]
    pub namespace: String,
}

pub async fn handle_iris_debug(
    iris: &IrisConnection,
    client: &reqwest::Client,
    p: DebugParams,
) -> Result<rmcp::model::CallToolResult, rmcp::ErrorData> {
    let xecute_url = iris.atelier_url(&format!("/v1/{}/action/xecute", p.namespace));
    let query_url = iris.atelier_url(&format!("/v1/{}/action/query", p.namespace));

    match p.action.as_str() {
        "map_int" => {
            let err = p.error_string.as_deref().unwrap_or("");
            // Parse routine and offset from "<TYPE>offset^routine.N"
            let code = format!(
                "set err=\"{}\" set routine=$piece($piece(err,\"^\",2),\".\",1) set offset=$piece(err,\"+\",2) set offset=$piece(offset,\"^\",1) write ##class(%Studio.Debugger).SourceLine(routine,+offset)",
                err.replace('"', "\\\"")
            );
            let resp = client.post(&xecute_url)
                .basic_auth(&iris.username, Some(&iris.password))
                .json(&serde_json::json!({"expression": code}))
                .send().await
                .map_err(|e| rmcp::ErrorData::internal_error(format!("HTTP error: {e}"), None))?;
            let body: serde_json::Value = resp.json().await.unwrap_or_default();
            let output = body["result"]["content"][0]["content"]
                .as_array().map(|a| a.iter().filter_map(|v| v.as_str()).collect::<Vec<_>>().join("\n"))
                .unwrap_or_default();
            ok_json(serde_json::json!({"success": true, "error_string": err, "source_location": output}))
        }
        "error_logs" => {
            let sql = format!("SELECT TOP {} ID, Name, Location, Date, Time FROM %SYSTEM.Error ORDER BY ID DESC", p.limit);
            let resp = client.post(&query_url)
                .basic_auth(&iris.username, Some(&iris.password))
                .json(&serde_json::json!({"query": sql}))
                .send().await
                .map_err(|e| rmcp::ErrorData::internal_error(format!("HTTP error: {e}"), None))?;
            let body: serde_json::Value = resp.json().await.unwrap_or_default();
            ok_json(serde_json::json!({"success": true, "logs": body["result"]["content"]}))
        }
        "capture" => {
            let code = "set err=$ZERROR write \"error:\"_err,! set loc=$ZPOSITION write \"position:\"_loc,!";
            let resp = client.post(&xecute_url)
                .basic_auth(&iris.username, Some(&iris.password))
                .json(&serde_json::json!({"expression": code}))
                .send().await
                .map_err(|e| rmcp::ErrorData::internal_error(format!("HTTP error: {e}"), None))?;
            let body: serde_json::Value = resp.json().await.unwrap_or_default();
            let output = body["result"]["content"][0]["content"]
                .as_array().map(|a| a.iter().filter_map(|v| v.as_str()).collect::<Vec<_>>().join("\n"))
                .unwrap_or_default();
            ok_json(serde_json::json!({"success": true, "capture": output}))
        }
        "source_map" => {
            let cls = p.class_name.as_deref().unwrap_or("");
            let code = format!(
                "set map=\"\" set line=1 do {{set int=##class(%Studio.Debugger).MapToINT(\"{cls}\",line,.intline) if int=\"\" quit set map=map_line_\"->\"_intline_\",\" set line=line+1 }} while 1 write map",
                cls = cls.replace('"', "\\\"")
            );
            let resp = client.post(&xecute_url)
                .basic_auth(&iris.username, Some(&iris.password))
                .json(&serde_json::json!({"expression": code}))
                .send().await
                .map_err(|e| rmcp::ErrorData::internal_error(format!("HTTP error: {e}"), None))?;
            let body: serde_json::Value = resp.json().await.unwrap_or_default();
            let output = body["result"]["content"][0]["content"]
                .as_array().map(|a| a.iter().filter_map(|v| v.as_str()).collect::<Vec<_>>().join("\n"))
                .unwrap_or_default();
            ok_json(serde_json::json!({"success": true, "class": cls, "mapping": output}))
        }
        other => err_json("INVALID_PARAM", &format!("Unknown action='{}'. Use: map_int, error_logs, capture, source_map", other)),
    }
}

// ── iris_generate ─────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize, JsonSchema)]
pub struct GenerateParams {
    /// Description of what to generate
    pub description: String,
    /// Type: class or test
    #[serde(default = "default_type")]
    pub gen_type: String,
    /// Class name to generate tests for (when gen_type=test)
    pub class_name: Option<String>,
    /// Compile after generating
    #[serde(default)]
    pub compile: bool,
    #[serde(default = "default_namespace")]
    pub namespace: String,
}

fn default_type() -> String { "class".to_string() }

pub async fn handle_iris_generate(
    iris: &IrisConnection,
    client: &reqwest::Client,
    p: GenerateParams,
) -> Result<rmcp::model::CallToolResult, rmcp::ErrorData> {
    let model = std::env::var("IRIS_GENERATE_CLASS_MODEL").unwrap_or_default();
    if model.is_empty() {
        return err_json("LLM_UNAVAILABLE", "IRIS_GENERATE_CLASS_MODEL env var not set");
    }

    // Build prompt
    let prompt = match p.gen_type.as_str() {
        "test" => format!(
            "Generate an InterSystems IRIS %UnitTest.TestCase subclass for '{}'. {}. Return only valid ObjectScript class code.",
            p.class_name.as_deref().unwrap_or("the class"),
            p.description
        ),
        _ => format!(
            "Generate an InterSystems IRIS ObjectScript class. {}. Return only valid ObjectScript class code.",
            p.description
        ),
    };

    // Call LLM via litellm-compatible HTTP endpoint
    let litellm_url = std::env::var("LITELLM_URL")
        .unwrap_or_else(|_| "https://api.anthropic.com/v1/messages".to_string());

    let api_key = std::env::var("ANTHROPIC_API_KEY")
        .or_else(|_| std::env::var("OPENAI_API_KEY"))
        .unwrap_or_default();

    let body = serde_json::json!({
        "model": model,
        "max_tokens": 2048,
        "messages": [{"role": "user", "content": prompt}]
    });

    let resp = client.post(&litellm_url)
        .header("x-api-key", &api_key)
        .header("anthropic-version", "2023-06-01")
        .json(&body)
        .send().await
        .map_err(|e| rmcp::ErrorData::internal_error(format!("LLM request failed: {e}"), None))?;

    let llm_body: serde_json::Value = resp.json().await.unwrap_or_default();
    let generated = llm_body["content"][0]["text"]
        .as_str()
        .unwrap_or("")
        .to_string();

    if generated.is_empty() {
        return err_json("LLM_ERROR", "LLM returned empty response");
    }

    let mut result = serde_json::json!({
        "success": true,
        "gen_type": p.gen_type,
        "generated": generated,
    });

    // Optional compile
    if p.compile && !generated.is_empty() {
        // Extract class name from generated code
        let class_name = generated.lines()
            .find(|l| l.trim_start().starts_with("Class "))
            .and_then(|l| l.trim_start().strip_prefix("Class "))
            .and_then(|l| l.split_whitespace().next())
            .map(|s| s.to_string())
            .unwrap_or_else(|| "Generated.Class".to_string());

        // First write the document
        let put_url = iris.atelier_url(&format!("/v8/{}/doc/{}.cls", p.namespace, urlencoding::encode(&class_name)));
        let lines: Vec<&str> = generated.lines().collect();
        let _ = client.put(&put_url)
            .basic_auth(&iris.username, Some(&iris.password))
            .json(&serde_json::json!({"enc": false, "content": lines}))
            .send().await;

        // Then compile
        let compile_url = iris.atelier_url(&format!("/v8/{}/action/compile", p.namespace));
        let compile_resp = client.post(&compile_url)
            .basic_auth(&iris.username, Some(&iris.password))
            .json(&serde_json::json!({"docs": [format!("{}.cls", class_name)], "flags": "cuk"}))
            .send().await;

        if let Ok(cr) = compile_resp {
            if cr.status().is_success() {
                if let Ok(cb) = cr.json::<serde_json::Value>().await {
                    result["compile_result"] = cb["result"].clone();
                }
            }
        }
    }

    Ok(rmcp::model::CallToolResult::success(vec![rmcp::model::Content::text(result.to_string())]))
}
