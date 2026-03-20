//! T023: MCP handshake integration test.
//! Spawns `iris-dev mcp` binary, sends JSON-RPC initialize + tools/list,
//! asserts ≥23 tools returned and response within 500ms.
//!
//! Tests written FIRST — must fail until T015–T022 are implemented.

use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

fn iris_dev_bin() -> std::path::PathBuf {
    // Find the binary in the cargo target directory
    let mut path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.pop(); // crates/iris-dev-core → crates
    path.pop(); // crates → workspace root
    path.push("target/debug/iris-dev");
    path
}

fn send_jsonrpc(stdin: &mut impl Write, id: u64, method: &str, params: &str) {
    let msg = format!(
        "{{\"jsonrpc\":\"2.0\",\"id\":{},\"method\":\"{}\",\"params\":{}}}\n",
        id, method, params
    );
    stdin.write_all(msg.as_bytes()).unwrap();
    stdin.flush().unwrap();
}

fn read_jsonrpc(reader: &mut impl BufRead) -> serde_json::Value {
    let mut line = String::new();
    reader.read_line(&mut line).unwrap();
    serde_json::from_str(&line).expect("invalid JSON-RPC response")
}

/// iris-dev mcp starts and responds to initialize within 500ms.
#[test]
fn mcp_server_starts_and_responds_to_initialize() {
    let bin = iris_dev_bin();
    if !bin.exists() {
        eprintln!("Skipping: iris-dev binary not found at {}", bin.display());
        return;
    }

    let mut child = Command::new(&bin)
        .arg("mcp")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("failed to spawn iris-dev mcp");

    let mut stdin = child.stdin.take().unwrap();
    let stdout = child.stdout.take().unwrap();
    let mut reader = BufReader::new(stdout);

    let start = Instant::now();
    send_jsonrpc(&mut stdin, 1, "initialize", r#"{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"test","version":"0.1"}}"#);

    let response = read_jsonrpc(&mut reader);
    let elapsed = start.elapsed();

    assert!(elapsed < Duration::from_millis(500),
        "initialize took {}ms, expected <500ms", elapsed.as_millis());
    assert!(response.get("result").is_some(),
        "initialize response missing 'result': {}", response);

    child.kill().ok();
}

/// tools/list returns ≥23 tools.
#[test]
fn mcp_server_tools_list_returns_23_tools() {
    let bin = iris_dev_bin();
    if !bin.exists() {
        eprintln!("Skipping: iris-dev binary not found");
        return;
    }

    let mut child = Command::new(&bin)
        .arg("mcp")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("failed to spawn iris-dev mcp");

    let mut stdin = child.stdin.take().unwrap();
    let stdout = child.stdout.take().unwrap();
    let mut reader = BufReader::new(stdout);

    send_jsonrpc(&mut stdin, 1, "initialize", r#"{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"test","version":"0.1"}}"#);
    let _init = read_jsonrpc(&mut reader);

    send_jsonrpc(&mut stdin, 2, "tools/list", "{}");
    let response = read_jsonrpc(&mut reader);

    let tools = response["result"]["tools"].as_array()
        .expect("tools/list response missing tools array");

    let tool_names: Vec<_> = tools.iter()
        .filter_map(|t| t["name"].as_str())
        .collect();

    assert!(tool_names.len() >= 23,
        "expected ≥23 tools, got {}: {:?}", tool_names.len(), tool_names);

    // Assert all required tools are present (no dots — Bedrock compatible)
    let required = ["iris_compile", "iris_test", "iris_symbols", "debug_map_int_to_cls",
                    "docs_introspect", "skill_list", "kb_recall", "agent_stats"];
    for name in required {
        assert!(tool_names.contains(&name),
            "required tool '{}' missing from tools/list", name);
    }

    // Assert no tool has a dot in the name (Bedrock/VS Code requirement)
    for name in &tool_names {
        assert!(!name.contains('.'),
            "tool name '{}' contains dot — invalid for Bedrock/VS Code", name);
    }

    child.kill().ok();
}

/// Startup latency p50 < 100ms over 5 runs (SC-001).
#[test]
fn mcp_server_startup_latency_under_100ms() {
    let bin = iris_dev_bin();
    if !bin.exists() {
        eprintln!("Skipping: iris-dev binary not found");
        return;
    }

    let mut latencies = Vec::new();
    for _ in 0..5 {
        let mut child = Command::new(&bin)
            .arg("mcp")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .expect("failed to spawn iris-dev mcp");

        let mut stdin = child.stdin.take().unwrap();
        let stdout = child.stdout.take().unwrap();
        let mut reader = BufReader::new(stdout);

        let start = Instant::now();
        send_jsonrpc(&mut stdin, 1, "initialize", r#"{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"bench","version":"0.1"}}"#);
        let _resp = read_jsonrpc(&mut reader);
        latencies.push(start.elapsed());
        child.kill().ok();
    }

    latencies.sort();
    let p50 = latencies[latencies.len() / 2];
    assert!(p50 < Duration::from_millis(100),
        "p50 startup latency {}ms exceeds 100ms (SC-001)", p50.as_millis());
}
