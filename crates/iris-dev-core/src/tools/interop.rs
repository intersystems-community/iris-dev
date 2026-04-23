use crate::iris::connection::IrisConnection;
use rmcp::{model::*, ErrorData as McpError};
use schemars::JsonSchema;
use serde::Deserialize;

fn ok_json(v: serde_json::Value) -> Result<CallToolResult, McpError> {
    Ok(CallToolResult::success(vec![Content::text(v.to_string())]))
}
fn err_json(code: &str, msg: &str) -> Result<CallToolResult, McpError> {
    ok_json(serde_json::json!({"success": false, "error_code": code, "error": msg}))
}
fn iris_unreachable() -> McpError {
    McpError::invalid_request("IRIS_UNREACHABLE", None)
}
fn is_network_error(msg: &str) -> bool {
    msg.contains("error sending") || msg.contains("connection") || msg.contains("dns")
}

fn default_ns() -> String {
    "USER".to_string()
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ProductionStatusParams {
    #[serde(default = "default_ns")]
    pub namespace: String,
    #[serde(default)]
    pub full_status: bool,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ProductionNameParams {
    pub production: Option<String>,
    #[serde(default = "default_ns")]
    pub namespace: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ProductionStopParams {
    pub production: Option<String>,
    #[serde(default = "default_ns")]
    pub namespace: String,
    #[serde(default = "default_timeout")]
    pub timeout: u32,
    #[serde(default)]
    pub force: bool,
}
fn default_timeout() -> u32 {
    30
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ProductionUpdateParams {
    #[serde(default = "default_timeout")]
    pub timeout: u32,
    #[serde(default)]
    pub force: bool,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct LogsParams {
    pub item_name: Option<String>,
    #[serde(default = "default_limit")]
    pub limit: u32,
    #[serde(default = "default_log_type")]
    pub log_type: String,
}
fn default_limit() -> u32 {
    10
}
fn default_log_type() -> String {
    "error,warning".to_string()
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct QueuesParams {}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct MessageSearchParams {
    pub source: Option<String>,
    pub target: Option<String>,
    pub class_name: Option<String>,
    #[serde(default = "default_msg_limit")]
    pub limit: u32,
}
fn default_msg_limit() -> u32 {
    20
}

fn state_string(code: i64) -> &'static str {
    match code {
        1 => "Running",
        2 => "Stopped",
        3 => "Suspended",
        4 => "Troubled",
        5 => "NetworkStopped",
        _ => "Unknown",
    }
}

pub fn parse_status_response(raw: &str) -> Result<(String, i64, String), String> {
    if raw.is_empty() || raw == ":" {
        return Err("NO_PRODUCTION".to_string());
    }
    if raw.starts_with("ERROR") {
        return Err(format!("INTEROP_ERROR:{}", raw));
    }
    let parts: Vec<&str> = raw.splitn(2, ':').collect();
    if parts.len() < 2 || parts[0].is_empty() {
        return Err("NO_PRODUCTION".to_string());
    }
    let name = parts[0].to_string();
    let code: i64 = parts[1].trim().parse().unwrap_or(0);
    let state = state_string(code).to_string();
    Ok((name, code, state))
}

fn docker_required_interop() -> Result<CallToolResult, McpError> {
    err_json("DOCKER_REQUIRED", "Interoperability operations require docker exec. Set IRIS_CONTAINER=<container_name>.")
}

pub async fn interop_production_status_impl(
    iris: Option<&IrisConnection>,
    _params: ProductionStatusParams,
) -> Result<CallToolResult, McpError> {
    let iris = match iris {
        Some(i) => i,
        None => return err_json("IRIS_UNREACHABLE", "No IRIS connection"),
    };
    let code = r#"Set sc=##class(Ens.Director).GetProductionStatus(.n,.s) If $$$ISERR(sc) { Write "ERROR:"_$System.Status.GetErrorText(sc) } Else { Write n_":"_s }"#;
    match iris.execute(code, &iris.namespace).await {
        Ok(output) => {
            let raw = output.trim().to_string();
            match parse_status_response(&raw) {
                Ok((name, code, state)) => ok_json(
                    serde_json::json!({"success": true, "production": name, "state": state, "state_code": code}),
                ),
                Err(e) if e.starts_with("INTEROP_ERROR") => err_json("INTEROP_ERROR", &e[14..]),
                Err(_) => err_json("NO_PRODUCTION", "No production is running"),
            }
        }
        Err(e) if e.to_string() == "DOCKER_REQUIRED" => docker_required_interop(),
        Err(e) => err_json(
            if is_network_error(&e.to_string()) { "IRIS_UNREACHABLE" } else { "INTEROP_ERROR" },
            &e.to_string(),
        ),
    }
}

pub async fn interop_production_start_impl(
    iris: Option<&IrisConnection>,
    params: ProductionNameParams,
) -> Result<CallToolResult, McpError> {
    let iris = match iris {
        Some(i) => i,
        None => return err_json("IRIS_UNREACHABLE", "No IRIS connection"),
    };
    let prod = params.production.as_deref().unwrap_or("");
    let code = format!(
        r#"Set sc=##class(Ens.Director).StartProduction("{}") If $$$ISERR(sc) {{ Write "ERROR:"_$System.Status.GetErrorText(sc) }} Else {{ Write "OK" }}"#,
        prod
    );
    match iris.execute(&code, &iris.namespace).await {
        Ok(output) => {
            let raw = output.trim();
            if raw == "OK" { ok_json(serde_json::json!({"success": true, "state": "Running"})) }
            else { err_json("INTEROP_ERROR", raw) }
        }
        Err(e) if e.to_string() == "DOCKER_REQUIRED" => docker_required_interop(),
        Err(e) => err_json(
            if is_network_error(&e.to_string()) { "IRIS_UNREACHABLE" } else { "INTEROP_ERROR" },
            &e.to_string(),
        ),
    }
}

pub async fn interop_production_stop_impl(
    iris: Option<&IrisConnection>,
    params: ProductionStopParams,
) -> Result<CallToolResult, McpError> {
    let iris = match iris {
        Some(i) => i,
        None => return err_json("IRIS_UNREACHABLE", "No IRIS connection"),
    };
    let code = format!(
        r#"Set sc=##class(Ens.Director).StopProduction({},{}) If $$$ISERR(sc) {{ Write "ERROR:"_$System.Status.GetErrorText(sc) }} Else {{ Write "OK" }}"#,
        params.timeout,
        if params.force { 1 } else { 0 }
    );
    match iris.execute(&code, &iris.namespace).await {
        Ok(output) => {
            let raw = output.trim();
            if raw == "OK" { ok_json(serde_json::json!({"success": true, "state": "Stopped"})) }
            else { err_json("INTEROP_ERROR", raw) }
        }
        Err(e) if e.to_string() == "DOCKER_REQUIRED" => docker_required_interop(),
        Err(e) => err_json(
            if is_network_error(&e.to_string()) { "IRIS_UNREACHABLE" } else { "INTEROP_ERROR" },
            &e.to_string(),
        ),
    }
}

pub async fn interop_production_update_impl(
    iris: Option<&IrisConnection>,
    params: ProductionUpdateParams,
) -> Result<CallToolResult, McpError> {
    let iris = match iris {
        Some(i) => i,
        None => return err_json("IRIS_UNREACHABLE", "No IRIS connection"),
    };
    let code = format!(
        r#"Set sc=##class(Ens.Director).UpdateProduction({},{}) If $$$ISERR(sc) {{ Write "ERROR:"_$System.Status.GetErrorText(sc) }} Else {{ Write "OK" }}"#,
        params.timeout,
        if params.force { 1 } else { 0 }
    );
    match iris.execute(&code, &iris.namespace).await {
        Ok(output) => {
            let raw = output.trim();
            if raw == "OK" { ok_json(serde_json::json!({"success": true, "message": "Production updated"})) }
            else { err_json("INTEROP_ERROR", raw) }
        }
        Err(e) if e.to_string() == "DOCKER_REQUIRED" => docker_required_interop(),
        Err(e) => err_json(
            if is_network_error(&e.to_string()) { "IRIS_UNREACHABLE" } else { "INTEROP_ERROR" },
            &e.to_string(),
        ),
    }
}

pub async fn interop_production_needs_update_impl(
    iris: Option<&IrisConnection>,
) -> Result<CallToolResult, McpError> {
    let iris = match iris {
        Some(i) => i,
        None => return err_json("IRIS_UNREACHABLE", "No IRIS connection"),
    };
    let code = r#"Write ##class(Ens.Director).ProductionNeedsUpdate()"#;
    match iris.execute(code, &iris.namespace).await {
        Ok(output) => ok_json(serde_json::json!({"success": true, "needs_update": output.trim() == "1"})),
        Err(e) if e.to_string() == "DOCKER_REQUIRED" => docker_required_interop(),
        Err(e) => err_json(
            if is_network_error(&e.to_string()) { "IRIS_UNREACHABLE" } else { "INTEROP_ERROR" },
            &e.to_string(),
        ),
    }
}

pub async fn interop_production_recover_impl(
    iris: Option<&IrisConnection>,
) -> Result<CallToolResult, McpError> {
    let iris = match iris {
        Some(i) => i,
        None => return err_json("IRIS_UNREACHABLE", "No IRIS connection"),
    };
    let code = r#"Set sc=##class(Ens.Director).RecoverProduction() If $$$ISERR(sc) { Write "ERROR:"_$System.Status.GetErrorText(sc) } Else { Write "OK" }"#;
    match iris.execute(code, &iris.namespace).await {
        Ok(output) => {
            let raw = output.trim();
            if raw == "OK" { ok_json(serde_json::json!({"success": true, "state": "Running"})) }
            else { err_json("INTEROP_ERROR", raw) }
        }
        Err(e) if e.to_string() == "DOCKER_REQUIRED" => docker_required_interop(),
        Err(e) => err_json(
            if is_network_error(&e.to_string()) { "IRIS_UNREACHABLE" } else { "INTEROP_ERROR" },
            &e.to_string(),
        ),
    }
}

pub async fn interop_logs_impl(
    iris: Option<&IrisConnection>,
    params: LogsParams,
) -> Result<CallToolResult, McpError> {
    let iris = match iris {
        Some(i) => i,
        None => return err_json("IRIS_UNREACHABLE", "No IRIS connection"),
    };
    let client = IrisConnection::http_client().map_err(|_| iris_unreachable())?;
    let mut conditions = vec![];
    for lt in params.log_type.split(',') {
        match lt.trim().to_lowercase().as_str() {
            "error" => conditions.push("Type = 3"),
            "warning" => conditions.push("Type = 2"),
            "info" => conditions.push("Type = 1"),
            "alert" => conditions.push("Type = 4"),
            _ => {}
        }
    }
    let type_filter = if conditions.is_empty() {
        String::new()
    } else {
        format!("AND ({})", conditions.join(" OR "))
    };
    let item_filter = params
        .item_name
        .as_ref()
        .map(|n| format!("AND ConfigName = '{}'", n.replace('\'', "''")))
        .unwrap_or_default();
    let sql = format!("SELECT TOP {} ID, TimeLogged, Type, ConfigName, Text FROM Ens_Util.Log WHERE 1=1 {} {} ORDER BY ID DESC", params.limit, type_filter, item_filter);
    match iris.query(&sql, vec![], &client).await {
        Ok(resp) => ok_json(
            serde_json::json!({"success": true, "logs": resp["result"]["content"], "count": resp["result"]["content"].as_array().map(|a| a.len()).unwrap_or(0)}),
        ),
        Err(e) => err_json(
            if is_network_error(&e.to_string()) {
                "IRIS_UNREACHABLE"
            } else {
                "INTEROP_ERROR"
            },
            &e.to_string(),
        ),
    }
}

pub async fn interop_queues_impl(
    iris: Option<&IrisConnection>,
) -> Result<CallToolResult, McpError> {
    let iris = match iris {
        Some(i) => i,
        None => return err_json("IRIS_UNREACHABLE", "No IRIS connection"),
    };
    let client = IrisConnection::http_client().map_err(|_| iris_unreachable())?;
    match iris
        .query("SELECT * FROM Ens.Queue_Enumerate()", vec![], &client)
        .await
    {
        Ok(resp) => {
            ok_json(serde_json::json!({"success": true, "queues": resp["result"]["content"]}))
        }
        Err(e) => err_json(
            if is_network_error(&e.to_string()) {
                "IRIS_UNREACHABLE"
            } else {
                "INTEROP_ERROR"
            },
            &e.to_string(),
        ),
    }
}

pub async fn interop_message_search_impl(
    iris: Option<&IrisConnection>,
    params: MessageSearchParams,
) -> Result<CallToolResult, McpError> {
    let iris = match iris {
        Some(i) => i,
        None => return err_json("IRIS_UNREACHABLE", "No IRIS connection"),
    };
    let client = IrisConnection::http_client().map_err(|_| iris_unreachable())?;
    let mut filters = vec![];
    if let Some(src) = &params.source {
        filters.push(format!("SourceConfigName = '{}'", src.replace('\'', "''")));
    }
    if let Some(tgt) = &params.target {
        filters.push(format!("TargetConfigName = '{}'", tgt.replace('\'', "''")));
    }
    if let Some(cls) = &params.class_name {
        filters.push(format!(
            "MessageBodyClassName = '{}'",
            cls.replace('\'', "''")
        ));
    }
    let where_clause = if filters.is_empty() {
        String::new()
    } else {
        format!("WHERE {}", filters.join(" AND "))
    };
    let sql = format!("SELECT TOP {} ID, TimeCreated, SourceConfigName, TargetConfigName, MessageBodyClassName, Status FROM Ens.MessageHeader {} ORDER BY ID DESC", params.limit, where_clause);
    match iris.query(&sql, vec![], &client).await {
        Ok(resp) => ok_json(
            serde_json::json!({"success": true, "messages": resp["result"]["content"], "count": resp["result"]["content"].as_array().map(|a| a.len()).unwrap_or(0)}),
        ),
        Err(e) => err_json(
            if is_network_error(&e.to_string()) {
                "IRIS_UNREACHABLE"
            } else {
                "INTEROP_ERROR"
            },
            &e.to_string(),
        ),
    }
}
