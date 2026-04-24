//! iris_info — namespace/document discovery via Atelier REST.
//! iris_macro — macro introspection.
//! iris_debug — debug tools via Atelier xecute + SQL.
//! iris_generate — LLM-based class/test generation.

use crate::iris::connection::IrisConnection;
use schemars::JsonSchema;
use serde::Deserialize;

fn ok_json(v: serde_json::Value) -> Result<rmcp::model::CallToolResult, rmcp::ErrorData> {
    Ok(rmcp::model::CallToolResult::success(vec![
        rmcp::model::Content::text(v.to_string()),
    ]))
}
fn err_json(code: &str, msg: &str) -> Result<rmcp::model::CallToolResult, rmcp::ErrorData> {
    ok_json(serde_json::json!({"success": false, "error_code": code, "error": msg}))
}
fn default_namespace() -> String {
    "USER".to_string()
}
fn default_limit() -> usize {
    20
}

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
            iris.versioned_ns_url(ns, &format!("/docs?category={}", cat))
        }
        "modified" => iris.versioned_ns_url(ns, "/docs/modified"),
        "namespace" => iris.versioned_ns_url(ns, "/"),
        "metadata" => iris.versioned_ns_url(ns, "/metadata"),
        "jobs" => iris.versioned_ns_url(ns, "/jobs"),
        "csp_apps" => iris.versioned_ns_url(ns, "/cspapps"),
        "csp_debug" => iris.versioned_ns_url(ns, "/cspdebugid"),
        "sa_schema" => {
            let name = p.name.as_deref().unwrap_or("");
            iris.versioned_ns_url(ns, &format!("/saschema/{}", urlencoding::encode(name)))
        }
        other => return err_json("INVALID_PARAM", &format!("Unknown what='{}'. Use: documents, modified, namespace, metadata, jobs, csp_apps, csp_debug, sa_schema", other)),
    };

    let resp = client
        .get(&url)
        .basic_auth(&iris.username, Some(&iris.password))
        .send()
        .await
        .map_err(|e| rmcp::ErrorData::internal_error(format!("HTTP error: {e}"), None))?;

    if !resp.status().is_success() {
        return err_json(
            "IRIS_UNREACHABLE",
            &format!("HTTP {} for {}", resp.status(), url),
        );
    }

    let body: serde_json::Value = resp.json().await.unwrap_or_default();
    ok_json(
        serde_json::json!({"success": true, "what": p.what, "namespace": p.namespace, "result": body["result"]}),
    )
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
            let url = iris.versioned_ns_url(&p.namespace, "/macros");
            let resp = client
                .get(&url)
                .basic_auth(&iris.username, Some(&iris.password))
                .send()
                .await
                .map_err(|e| rmcp::ErrorData::internal_error(format!("HTTP error: {e}"), None))?;
            let body: serde_json::Value = resp.json().await.unwrap_or_default();
            ok_json(serde_json::json!({"success": true, "macros": body["result"]["content"]}))
        }
        action @ ("signature" | "location" | "definition" | "expand") => {
            let name = p.name.as_deref().unwrap_or("");
            let url = iris.versioned_ns_url(&p.namespace, "/action/getmacro");
            let arg_count = p.args.len();
            let resp = client
                .post(&url)
                .basic_auth(&iris.username, Some(&iris.password))
                .json(&serde_json::json!({
                    "macros": [{"name": name, "arguments": arg_count}],
                    "action": action,
                    "args": p.args,
                }))
                .send()
                .await
                .map_err(|e| rmcp::ErrorData::internal_error(format!("HTTP error: {e}"), None))?;
            let body: serde_json::Value = resp.json().await.unwrap_or_default();
            ok_json(
                serde_json::json!({"success": true, "name": name, "action": action, "result": body["result"]}),
            )
        }
        other => err_json(
            "INVALID_PARAM",
            &format!(
                "Unknown action='{}'. Use: list, signature, location, definition, expand",
                other
            ),
        ),
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
    let query_url = iris.versioned_ns_url(&p.namespace, "/action/query");

    match p.action.as_str() {
        "map_int" => {
            let err = p.error_string.as_deref().unwrap_or("");
            let code = format!(
                "set err=\"{}\" set routine=$piece($piece(err,\"^\",2),\".\",1) set offset=$piece(err,\"+\",2) set offset=$piece(offset,\"^\",1) write ##class(%Studio.Debugger).SourceLine(routine,+offset)",
                err.replace('"', "\\\"")
            );
            match iris.execute(&code, &p.namespace).await {
                Ok(output) => ok_json(serde_json::json!({"success": true, "error_string": err, "source_location": output.trim()})),
                Err(e) if e.to_string() == "DOCKER_REQUIRED" => ok_json(serde_json::json!({
                    "success": false, "error_code": "DOCKER_REQUIRED",
                    "error": "iris_debug map_int requires docker exec. Set IRIS_CONTAINER=<container_name>.",
                })),
                Err(e) => err_json("EXECUTION_FAILED", &e.to_string()),
            }
        }
        "error_logs" => {
            let sql = format!(
                "SELECT TOP {} ID, Name, Location, Date, Time FROM %SYSTEM.Error ORDER BY ID DESC",
                p.limit
            );
            let resp = client
                .post(&query_url)
                .basic_auth(&iris.username, Some(&iris.password))
                .json(&serde_json::json!({"query": sql}))
                .send()
                .await
                .map_err(|e| rmcp::ErrorData::internal_error(format!("HTTP error: {e}"), None))?;
            let body: serde_json::Value = resp.json().await.unwrap_or_default();
            ok_json(serde_json::json!({"success": true, "logs": body["result"]["content"]}))
        }
        "capture" => {
            let code = "set err=$ZERROR write \"error:\"_err,! set loc=$ZPOSITION write \"position:\"_loc,!";
            match iris.execute(code, &p.namespace).await {
                Ok(output) => ok_json(serde_json::json!({"success": true, "capture": output.trim()})),
                Err(e) if e.to_string() == "DOCKER_REQUIRED" => ok_json(serde_json::json!({
                    "success": false, "error_code": "DOCKER_REQUIRED",
                    "error": "iris_debug capture requires docker exec. Set IRIS_CONTAINER=<container_name>.",
                })),
                Err(e) => err_json("EXECUTION_FAILED", &e.to_string()),
            }
        }
        "source_map" => {
            let cls = p.class_name.as_deref().unwrap_or("");
            let code = format!(
                "set map=\"\" set line=1 do {{set int=##class(%Studio.Debugger).MapToINT(\"{cls}\",line,.intline) if int=\"\" quit set map=map_line_\"->\"_intline_\",\" set line=line+1 }} while 1 write map",
                cls = cls.replace('"', "\\\"")
            );
            match iris.execute(&code, &p.namespace).await {
                Ok(output) => ok_json(serde_json::json!({"success": true, "class": cls, "mapping": output.trim()})),
                Err(e) if e.to_string() == "DOCKER_REQUIRED" => ok_json(serde_json::json!({
                    "success": false, "error_code": "DOCKER_REQUIRED",
                    "error": "iris_debug source_map requires docker exec. Set IRIS_CONTAINER=<container_name>.",
                })),
                Err(e) => err_json("EXECUTION_FAILED", &e.to_string()),
            }
        }
        other => err_json(
            "INVALID_PARAM",
            &format!(
                "Unknown action='{}'. Use: map_int, error_logs, capture, source_map",
                other
            ),
        ),
    }
}

// ── iris_generate ─────────────────────────────────────────────────────────────
//
// Context-provider design: returns everything the calling AI agent needs to
// write the class itself. No API key, no server-side LLM call, works with
// Copilot, Claude Code, or any MCP client.

#[derive(Debug, Deserialize, JsonSchema)]
pub struct GenerateParams {
    /// What to generate — natural language description, e.g. "a Patient class with Name and DOB properties"
    pub description: String,
    /// Type: "class" (default) or "test"
    #[serde(default = "default_type")]
    pub gen_type: String,
    /// Existing class name to generate tests for (gen_type=test only)
    pub class_name: Option<String>,
    #[serde(default = "default_namespace")]
    pub namespace: String,
}

fn default_type() -> String {
    "class".to_string()
}

pub async fn handle_iris_generate(
    iris: &IrisConnection,
    client: &reqwest::Client,
    p: GenerateParams,
) -> Result<rmcp::model::CallToolResult, rmcp::ErrorData> {
    let ns = &p.namespace;
    let query_url = iris.versioned_ns_url(ns, "/action/query");

    match p.gen_type.as_str() {
        "test" => {
            let cls = p.class_name.as_deref().unwrap_or("");

            // Fetch the class's methods and properties as generation context
            let sql = format!(
                "SELECT Name, FormalSpec, ReturnType, Description \
                 FROM %Dictionary.CompiledMethod WHERE parent = '{}' ORDER BY Name",
                cls.replace('\'', "''")
            );
            let resp = client
                .post(&query_url)
                .basic_auth(&iris.username, Some(&iris.password))
                .json(&serde_json::json!({"query": sql}))
                .send()
                .await
                .map_err(|e| rmcp::ErrorData::internal_error(format!("HTTP error: {e}"), None))?;
            let body: serde_json::Value = resp.json().await.unwrap_or_default();
            let methods = body["result"]["content"].clone();

            let prompt = format!(
                "Write an InterSystems IRIS %UnitTest.TestCase subclass to test '{}'. \
                 Requirements: {}. \
                 The class has these methods: {}. \
                 Rules: extend %UnitTest.TestCase, prefix test methods with 'Test', \
                 use $$$AssertEquals/$$$AssertTrue macros, include ##class({}).%New() in setup. \
                 Write only valid ObjectScript — no explanations, no markdown fences.",
                cls,
                p.description,
                serde_json::to_string(&methods).unwrap_or_default(),
                cls
            );

            ok_json(serde_json::json!({
                "success": true,
                "gen_type": "test",
                "target_class": cls,
                "namespace": ns,
                "prompt": prompt,
                "context": {
                    "methods": methods,
                    "suggested_class_name": format!("{}.Test", cls),
                },
                "instructions": "Use the prompt above to write the class, then call iris_doc(mode=put) to save it and iris_compile to compile it."
            }))
        }

        _ => {
            // Fetch existing classes in the namespace as naming/style context
            let sql = "SELECT TOP 10 Name FROM %Dictionary.ClassDefinition \
                       WHERE Name NOT LIKE '%\\%%' ESCAPE '\\' ORDER BY Name";
            let resp = client
                .post(&query_url)
                .basic_auth(&iris.username, Some(&iris.password))
                .json(&serde_json::json!({"query": sql}))
                .send()
                .await
                .map_err(|e| rmcp::ErrorData::internal_error(format!("HTTP error: {e}"), None))?;
            let body: serde_json::Value = resp.json().await.unwrap_or_default();
            let existing: Vec<String> = body["result"]["content"]
                .as_array()
                .unwrap_or(&vec![])
                .iter()
                .filter_map(|r| r["Name"].as_str().map(|s| s.to_string()))
                .collect();

            // Detect likely package prefix from existing classes
            let package = existing
                .first()
                .and_then(|n| n.split('.').next())
                .unwrap_or("MyApp")
                .to_string();

            let prompt = format!(
                "Write an InterSystems IRIS ObjectScript class. \
                 Requirements: {}. \
                 Use package prefix '{}' to match existing classes in this namespace. \
                 Rules: valid ObjectScript syntax, extend %Persistent or %RegisteredObject \
                 as appropriate, include property definitions with types, add basic accessor \
                 methods if needed. Write only the class code — no explanations, no markdown fences.",
                p.description, package
            );

            ok_json(serde_json::json!({
                "success": true,
                "gen_type": "class",
                "namespace": ns,
                "prompt": prompt,
                "context": {
                    "existing_classes": existing,
                    "suggested_package": package,
                    "iris_version": iris.version.as_deref().unwrap_or("unknown"),
                },
                "instructions": "Use the prompt above to write the class, then call iris_doc(mode=put) to save it and iris_compile to compile it."
            }))
        }
    }
}
