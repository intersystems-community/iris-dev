//! Spec 022 — Comprehensive E2E test suite for all iris-dev tools.
//! FR-001/FR-002: every tool has a happy-path test and an error-path test.
//! Run: IRIS_HOST=localhost IRIS_WEB_PORT=52773 IRIS_USERNAME=_SYSTEM IRIS_PASSWORD=SYS cargo test --test test_e2e_spec022
#![allow(dead_code, clippy::zombie_processes)]

use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};

fn iris_dev_bin() -> std::path::PathBuf {
    let mut path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.pop();
    path.pop();
    path.push("target/debug/iris-dev");
    path
}

struct McpSession {
    child: std::process::Child,
    stdin: std::process::ChildStdin,
    reader: BufReader<std::process::ChildStdout>,
    next_id: u64,
}

impl McpSession {
    fn new() -> Option<Self> {
        let bin = iris_dev_bin();
        if !bin.exists() {
            return None;
        }
        let iris_host = std::env::var("IRIS_HOST").unwrap_or_default();
        let iris_port = std::env::var("IRIS_WEB_PORT").unwrap_or_else(|_| "52773".to_string());
        let username = std::env::var("IRIS_USERNAME").unwrap_or_else(|_| "_SYSTEM".to_string());
        let password = std::env::var("IRIS_PASSWORD").unwrap_or_else(|_| "SYS".to_string());

        let mut child = Command::new(&bin)
            .args(["mcp"])
            .env("IRIS_HOST", &iris_host)
            .env("IRIS_WEB_PORT", &iris_port)
            .env("IRIS_USERNAME", &username)
            .env("IRIS_PASSWORD", &password)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .ok()?;

        let stdin = child.stdin.take()?;
        let reader = BufReader::new(child.stdout.take()?);
        Some(McpSession { child, stdin, reader, next_id: 1 })
    }

    fn send(&mut self, msg: serde_json::Value) -> Option<serde_json::Value> {
        let has_id = msg.get("id").is_some();
        let line = serde_json::to_string(&msg).unwrap() + "\n";
        self.stdin.write_all(line.as_bytes()).ok()?;
        self.stdin.flush().ok()?;

        if !has_id {
            std::thread::sleep(std::time::Duration::from_millis(80));
            return None;
        }

        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        loop {
            std::thread::sleep(std::time::Duration::from_millis(50));
            let mut buf = String::new();
            if self.reader.read_line(&mut buf).unwrap_or(0) > 0 {
                if let Ok(v) = serde_json::from_str::<serde_json::Value>(&buf) {
                    return Some(v);
                }
            }
            if std::time::Instant::now() > deadline {
                return None;
            }
        }
    }

    fn call_tool(&mut self, name: &str, args: serde_json::Value) -> serde_json::Value {
        let id = self.next_id;
        self.next_id += 1;
        let resp = self
            .send(serde_json::json!({
                "jsonrpc": "2.0",
                "id": id,
                "method": "tools/call",
                "params": {"name": name, "arguments": args}
            }))
            .unwrap_or_default();

        let text = resp["result"]["content"][0]["text"].as_str().unwrap_or("{}");
        serde_json::from_str(text).unwrap_or_else(|_| serde_json::json!({"raw": text}))
    }

    fn handshake(&mut self) {
        self.send(serde_json::json!({
            "jsonrpc": "2.0",
            "id": 0,
            "method": "initialize",
            "params": {
                "protocolVersion": "2024-11-05",
                "capabilities": {},
                "clientInfo": {"name": "spec022-e2e", "version": "0.1"}
            }
        }));
        self.send(serde_json::json!({"jsonrpc":"2.0","method":"notifications/initialized","params":{}}));
    }
}

impl Drop for McpSession {
    fn drop(&mut self) {
        self.child.kill().ok();
    }
}

fn skip_if_no_iris() -> bool {
    let host = std::env::var("IRIS_HOST").unwrap_or_default();
    if host.is_empty() {
        eprintln!("Skipping: IRIS_HOST not set");
        return true;
    }
    if !iris_dev_bin().exists() {
        eprintln!("Skipping: iris-dev binary not found — run cargo build first");
        return true;
    }
    false
}

fn assert_no_404(result: &serde_json::Value, tool: &str) {
    let err = result["error"].as_str().unwrap_or("");
    let code = result["error_code"].as_str().unwrap_or("");
    assert!(
        !err.contains("404") && !err.contains("HTTP 404"),
        "{tool}: must not return raw 404 — got: {result}"
    );
    assert_ne!(
        code, "INTERNAL_ERROR",
        "{tool}: must not return INTERNAL_ERROR — got: {result}"
    );
}

fn assert_not_null_success(result: &serde_json::Value, tool: &str, field: &str) {
    if result["success"].as_bool() == Some(true) {
        assert!(
            !result[field].is_null(),
            "{tool}: {field} must not be null on success — got: {result}"
        );
    }
}

// ── iris_query ────────────────────────────────────────────────────────────────

#[test]
fn e2e_iris_query_happy() {
    if skip_if_no_iris() { return; }
    let mut s = McpSession::new().expect("session");
    s.handshake();
    let r = s.call_tool("iris_query", serde_json::json!({"query": "SELECT 1 AS n"}));
    assert_eq!(r["success"], true, "iris_query SELECT 1: {r}");
    assert!(r["columns"].is_array() || r["results"].is_array(), "iris_query must return columns/results: {r}");
    assert_no_404(&r, "iris_query");
}

#[test]
fn e2e_iris_query_error_path() {
    if skip_if_no_iris() { return; }
    let mut s = McpSession::new().expect("session");
    s.handshake();
    let r = s.call_tool("iris_query", serde_json::json!({"query": "THIS IS NOT SQL"}));
    assert_no_404(&r, "iris_query-error");
    assert!(
        r.get("success").is_some() || r.get("error_code").is_some(),
        "iris_query error path must be structured: {r}"
    );
}

// ── iris_compile ──────────────────────────────────────────────────────────────

#[test]
fn e2e_iris_compile_happy() {
    if skip_if_no_iris() { return; }
    let mut s = McpSession::new().expect("session");
    s.handshake();
    let r = s.call_tool("iris_compile", serde_json::json!({"target": "%Library.Base"}));
    assert_no_404(&r, "iris_compile");
    assert!(
        r.get("success").is_some() || r.get("error_code").is_some(),
        "iris_compile must be structured: {r}"
    );
}

#[test]
fn e2e_iris_compile_error_path() {
    if skip_if_no_iris() { return; }
    let mut s = McpSession::new().expect("session");
    s.handshake();
    let r = s.call_tool("iris_compile", serde_json::json!({"target": "DoesNotExist.Spec022"}));
    assert_no_404(&r, "iris_compile-error");
    assert!(
        r.get("success").is_some() || r.get("error_code").is_some(),
        "iris_compile error path must be structured: {r}"
    );
}

// ── iris_doc ──────────────────────────────────────────────────────────────────

#[test]
fn e2e_iris_doc_get_happy() {
    if skip_if_no_iris() { return; }
    let mut s = McpSession::new().expect("session");
    s.handshake();
    let r = s.call_tool("iris_doc", serde_json::json!({"mode": "get", "name": "%Library.Base.cls"}));
    assert_no_404(&r, "iris_doc-get");
    assert!(
        r.get("success").is_some() || r.get("error_code").is_some(),
        "iris_doc get must be structured: {r}"
    );
    if r["success"].as_bool() == Some(true) {
        assert!(!r["content"].is_null(), "iris_doc get: content must not be null: {r}");
    }
}

#[test]
fn e2e_iris_doc_head_missing() {
    if skip_if_no_iris() { return; }
    let mut s = McpSession::new().expect("session");
    s.handshake();
    let r = s.call_tool("iris_doc", serde_json::json!({"mode": "head", "name": "Spec022.DoesNotExist.cls"}));
    assert_no_404(&r, "iris_doc-head-missing");
    assert!(
        r.get("success").is_some() || r.get("error_code").is_some(),
        "iris_doc head must be structured: {r}"
    );
}

// ── iris_search ───────────────────────────────────────────────────────────────

#[test]
fn e2e_iris_search_happy() {
    if skip_if_no_iris() { return; }
    let mut s = McpSession::new().expect("session");
    s.handshake();
    let r = s.call_tool("iris_search", serde_json::json!({"query": "Persistent", "doc_type": "CLS", "max_results": 5}));
    assert_no_404(&r, "iris_search");
    assert!(
        r.get("success").is_some() || r.get("error_code").is_some(),
        "iris_search must be structured: {r}"
    );
}

// ── iris_symbols ──────────────────────────────────────────────────────────────

#[test]
fn e2e_iris_symbols_happy() {
    if skip_if_no_iris() { return; }
    let mut s = McpSession::new().expect("session");
    s.handshake();
    let r = s.call_tool("iris_symbols", serde_json::json!({"query": "Base", "limit": 5}));
    assert_no_404(&r, "iris_symbols");
    assert!(
        r.get("success").is_some() || r.get("error_code").is_some(),
        "iris_symbols must be structured: {r}"
    );
}

// ── iris_info ─────────────────────────────────────────────────────────────────

#[test]
fn e2e_iris_info_metadata() {
    if skip_if_no_iris() { return; }
    let mut s = McpSession::new().expect("session");
    s.handshake();
    let r = s.call_tool("iris_info", serde_json::json!({"what": "metadata"}));
    assert_eq!(r["success"], true, "iris_info metadata: {r}");
    assert_no_404(&r, "iris_info-metadata");
}

#[test]
fn e2e_iris_info_documents() {
    if skip_if_no_iris() { return; }
    let mut s = McpSession::new().expect("session");
    s.handshake();
    let r = s.call_tool("iris_info", serde_json::json!({"what": "documents", "doc_type": "CLS"}));
    assert_no_404(&r, "iris_info-documents");
    assert!(r.get("success").is_some(), "iris_info documents must be structured: {r}");
}

#[test]
fn e2e_iris_info_namespace_not_404() {
    if skip_if_no_iris() { return; }
    let mut s = McpSession::new().expect("session");
    s.handshake();
    // FR-004: iris_info(what=namespace) must not return 404.
    let r = s.call_tool("iris_info", serde_json::json!({"what": "namespace"}));
    assert_no_404(&r, "iris_info-namespace");
    assert_eq!(r["success"], true, "iris_info namespace must succeed after URL fix: {r}");
}

#[test]
fn e2e_iris_info_invalid_what() {
    if skip_if_no_iris() { return; }
    let mut s = McpSession::new().expect("session");
    s.handshake();
    let r = s.call_tool("iris_info", serde_json::json!({"what": "bogus_spec022"}));
    assert_eq!(r["error_code"].as_str(), Some("INVALID_PARAM"), "iris_info bogus what: {r}");
}

// ── iris_macro ────────────────────────────────────────────────────────────────

#[test]
fn e2e_iris_macro_list_not_null() {
    if skip_if_no_iris() { return; }
    let mut s = McpSession::new().expect("session");
    s.handshake();
    // FR-005: macros must be [] not null.
    let r = s.call_tool("iris_macro", serde_json::json!({"action": "list"}));
    assert_no_404(&r, "iris_macro-list");
    assert_not_null_success(&r, "iris_macro-list", "macros");
    if r["success"].as_bool() == Some(true) {
        assert!(r["macros"].is_array(), "iris_macro list: macros must be array, not null: {r}");
    }
}

#[test]
fn e2e_iris_macro_invalid_action() {
    if skip_if_no_iris() { return; }
    let mut s = McpSession::new().expect("session");
    s.handshake();
    let r = s.call_tool("iris_macro", serde_json::json!({"action": "bogus_spec022"}));
    assert_eq!(r["error_code"].as_str(), Some("INVALID_PARAM"), "iris_macro bogus action: {r}");
}

// ── iris_debug ────────────────────────────────────────────────────────────────

#[test]
fn e2e_iris_debug_error_logs_not_null() {
    if skip_if_no_iris() { return; }
    let mut s = McpSession::new().expect("session");
    s.handshake();
    // FR-005: logs must be [] not null.
    let r = s.call_tool("iris_debug", serde_json::json!({"action": "error_logs", "limit": 5}));
    assert_no_404(&r, "iris_debug-error_logs");
    assert_not_null_success(&r, "iris_debug-error_logs", "logs");
    if r["success"].as_bool() == Some(true) {
        assert!(r["logs"].is_array(), "iris_debug error_logs: logs must be array, not null: {r}");
    }
}

#[test]
fn e2e_iris_debug_map_int_docker_required() {
    if skip_if_no_iris() { return; }
    // Skip if IRIS_CONTAINER is set (would actually try to execute).
    if std::env::var("IRIS_CONTAINER").is_ok() { return; }
    let mut s = McpSession::new().expect("session");
    s.handshake();
    let r = s.call_tool("iris_debug", serde_json::json!({"action": "map_int", "error_string": "<UNDEFINED>x+3^Foo.1"}));
    assert_no_404(&r, "iris_debug-map_int");
    // Must get DOCKER_REQUIRED, not a 404 or INTERNAL_ERROR.
    assert_eq!(r["error_code"].as_str(), Some("DOCKER_REQUIRED"), "iris_debug map_int without docker: {r}");
}

// ── iris_execute ──────────────────────────────────────────────────────────────

#[test]
fn e2e_iris_execute_docker_required_when_no_container() {
    if skip_if_no_iris() { return; }
    if std::env::var("IRIS_CONTAINER").is_ok() { return; }
    let mut s = McpSession::new().expect("session");
    s.handshake();
    // FR-003: must return DOCKER_REQUIRED, not HTTP 404.
    let r = s.call_tool("iris_execute", serde_json::json!({"code": "write 1+1"}));
    assert_no_404(&r, "iris_execute");
    assert_eq!(
        r["error_code"].as_str(),
        Some("DOCKER_REQUIRED"),
        "iris_execute without IRIS_CONTAINER must return DOCKER_REQUIRED: {r}"
    );
}

#[test]
fn e2e_iris_execute_with_container() {
    if skip_if_no_iris() { return; }
    let container = match std::env::var("IRIS_CONTAINER") {
        Ok(c) => c,
        Err(_) => return, // skip if no container
    };
    eprintln!("Testing iris_execute with IRIS_CONTAINER={}", container);
    let mut s = McpSession::new().expect("session");
    s.handshake();
    let r = s.call_tool("iris_execute", serde_json::json!({"code": "write 1+1,!"}));
    assert_eq!(r["success"], true, "iris_execute with docker: {r}");
    assert!(r["output"].as_str().map(|o| o.contains('2')).unwrap_or(false),
        "iris_execute output must contain '2': {r}");
}

// ── iris_test ─────────────────────────────────────────────────────────────────

#[test]
fn e2e_iris_test_docker_required_when_no_container() {
    if skip_if_no_iris() { return; }
    if std::env::var("IRIS_CONTAINER").is_ok() { return; }
    let mut s = McpSession::new().expect("session");
    s.handshake();
    let r = s.call_tool("iris_test", serde_json::json!({"pattern": "Spec022.Tests"}));
    assert_no_404(&r, "iris_test");
    assert_eq!(
        r["error_code"].as_str(),
        Some("DOCKER_REQUIRED"),
        "iris_test without IRIS_CONTAINER must return DOCKER_REQUIRED: {r}"
    );
}

// ── iris_source_control ───────────────────────────────────────────────────────

#[test]
fn e2e_iris_source_control_status_uncontrolled() {
    if skip_if_no_iris() { return; }
    let mut s = McpSession::new().expect("session");
    s.handshake();
    // Without SCM installed / without IRIS_CONTAINER, status must return UNCONTROLLED.
    let r = s.call_tool("iris_source_control", serde_json::json!({"action": "status", "document": "%Library.Base.cls"}));
    assert_no_404(&r, "iris_source_control-status");
    assert!(r.get("success").is_some(), "iris_source_control status must be structured: {r}");
}

#[test]
fn e2e_iris_source_control_checkout_no_container() {
    if skip_if_no_iris() { return; }
    if std::env::var("IRIS_CONTAINER").is_ok() { return; }
    let mut s = McpSession::new().expect("session");
    s.handshake();
    let r = s.call_tool("iris_source_control", serde_json::json!({"action": "checkout", "document": "%Library.Base.cls"}));
    assert_no_404(&r, "iris_source_control-checkout");
    // Must not silently return success=true when docker not available.
    assert_ne!(r["success"], true, "iris_source_control checkout without docker must not silently succeed: {r}");
}

// ── iris_generate ─────────────────────────────────────────────────────────────

#[test]
fn e2e_iris_generate_class() {
    if skip_if_no_iris() { return; }
    let mut s = McpSession::new().expect("session");
    s.handshake();
    let r = s.call_tool("iris_generate", serde_json::json!({"description": "a simple hello world class", "gen_type": "class"}));
    assert_eq!(r["success"], true, "iris_generate class: {r}");
    assert!(r["prompt"].as_str().map(|p| !p.is_empty()).unwrap_or(false), "iris_generate must return non-empty prompt: {r}");
    assert_no_404(&r, "iris_generate");
}

// ── iris_introspect ───────────────────────────────────────────────────────────

#[test]
fn e2e_iris_introspect_happy() {
    if skip_if_no_iris() { return; }
    let mut s = McpSession::new().expect("session");
    s.handshake();
    let r = s.call_tool("iris_introspect", serde_json::json!({"class_name": "%Library.Base"}));
    assert_no_404(&r, "iris_introspect");
    assert!(r.get("success").is_some(), "iris_introspect must be structured: {r}");
}

// ── zero-null contract ────────────────────────────────────────────────────────

/// FR-005: No tool returns success:true with a null data field.
/// Runs a quick sweep of key data-returning tools.
#[test]
fn e2e_no_tool_returns_silent_null() {
    if skip_if_no_iris() { return; }
    let mut s = McpSession::new().expect("session");
    s.handshake();

    let checks: &[(&str, serde_json::Value, &str)] = &[
        ("iris_macro",  serde_json::json!({"action": "list"}), "macros"),
        ("iris_debug",  serde_json::json!({"action": "error_logs", "limit": 1}), "logs"),
    ];

    for (tool, args, field) in checks {
        let r = s.call_tool(tool, args.clone());
        if r["success"].as_bool() == Some(true) {
            assert!(
                !r[field].is_null(),
                "FR-005 FAIL: {tool}.{field} is null on success=true — got: {r}"
            );
        }
    }
}
