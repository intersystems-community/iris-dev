//! T023: MCP handshake integration test.
//! Spawns `iris-dev mcp` binary, sends JSON-RPC initialize + tools/list,
//! asserts ≥23 tools returned and response within 500ms.
//!
//! Tests written FIRST — must fail until T015–T022 are implemented.
#![allow(dead_code, clippy::zombie_processes)]

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
    // Give any previous test's spawned processes time to fully exit
    std::thread::sleep(std::time::Duration::from_millis(500));
    let bin = iris_dev_bin();
    if !bin.exists() {
        eprintln!("Skipping: iris-dev binary not found at {}", bin.display());
        return;
    }

    let mut child = Command::new(&bin)
        .arg("mcp")
        // Disable IRIS discovery for handshake tests — we only test MCP protocol, not tools
        .env("IRIS_WEB_PORT", "9") // Port 9 (discard) — instant ECONNREFUSED, no DNS lookup
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("failed to spawn iris-dev mcp");

    let mut stdin = child.stdin.take().unwrap();
    let stdout = child.stdout.take().unwrap();
    let mut reader = BufReader::new(stdout);

    let start = Instant::now();
    send_jsonrpc(
        &mut stdin,
        1,
        "initialize",
        r#"{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"test","version":"0.1"}}"#,
    );

    let response = read_jsonrpc(&mut reader);
    let elapsed = start.elapsed();
    // Send required initialized notification
    let init_notif = concat!(
        r#"{"jsonrpc":"2.0","method":"notifications/initialized","params":{}}"#,
        "
"
    );
    stdin.write_all(init_notif.as_bytes()).unwrap();
    stdin.flush().unwrap();

    assert!(
        elapsed < Duration::from_millis(500),
        "initialize took {}ms, expected <500ms",
        elapsed.as_millis()
    );
    assert!(
        response.get("result").is_some(),
        "initialize response missing 'result': {}",
        response
    );

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
        // Disable IRIS discovery for handshake tests — we only test MCP protocol, not tools
        .env("IRIS_WEB_PORT", "9") // Port 9 (discard) — instant ECONNREFUSED, no DNS lookup
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("failed to spawn iris-dev mcp");

    let mut stdin = child.stdin.take().unwrap();
    let stdout = child.stdout.take().unwrap();
    let mut reader = BufReader::new(stdout);

    send_jsonrpc(
        &mut stdin,
        1,
        "initialize",
        r#"{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"test","version":"0.1"}}"#,
    );
    let _init = read_jsonrpc(&mut reader);
    let init_notif = concat!(
        r#"{"jsonrpc":"2.0","method":"notifications/initialized","params":{}}"#,
        "
"
    );
    stdin.write_all(init_notif.as_bytes()).unwrap();
    stdin.flush().unwrap();
    std::thread::sleep(std::time::Duration::from_millis(50));

    send_jsonrpc(&mut stdin, 2, "tools/list", "{}");
    let response = read_jsonrpc(&mut reader);

    let tools = response["result"]["tools"]
        .as_array()
        .expect("tools/list response missing tools array");

    let tool_names: Vec<_> = tools.iter().filter_map(|t| t["name"].as_str()).collect();

    assert!(
        tool_names.len() >= 23,
        "expected ≥23 tools, got {}: {:?}",
        tool_names.len(),
        tool_names
    );

    // Assert all required tools are present (no dots — Bedrock compatible)
    let required = [
        "iris_compile",
        "iris_test",
        "iris_symbols",
        "debug_map_int_to_cls",
        "docs_introspect",
        "skill_list",
        "kb_recall",
        "agent_stats",
    ];
    for name in required {
        assert!(
            tool_names.contains(&name),
            "required tool '{}' missing from tools/list",
            name
        );
    }

    // Assert no tool has a dot in the name (Bedrock/VS Code requirement)
    for name in &tool_names {
        assert!(
            !name.contains('.'),
            "tool name '{}' contains dot — invalid for Bedrock/VS Code",
            name
        );
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
        send_jsonrpc(
            &mut stdin,
            1,
            "initialize",
            r#"{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"bench","version":"0.1"}}"#,
        );
        let _resp = read_jsonrpc(&mut reader);
        latencies.push(start.elapsed());
        child.kill().ok();
    }

    latencies.sort();
    let p50 = latencies[latencies.len() / 2];
    assert!(
        p50 < Duration::from_millis(100),
        "p50 startup latency {}ms exceeds 100ms (SC-001)",
        p50.as_millis()
    );
}

/// T009: discovery waits for IRIS — server returns tool list within 5s even with no env vars.
/// Uses port 9 (discard) so discovery fails fast, but server still returns tool list.
#[test]
fn discovery_waits_for_iris() {
    let bin = iris_dev_bin();
    if !bin.exists() {
        eprintln!("Skipping: iris-dev binary not found");
        return;
    }

    let mut child = Command::new(&bin)
        .arg("mcp")
        .env("IRIS_WEB_PORT", "9") // instant fail — tests that server doesn't hang
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("failed to spawn iris-dev mcp");

    let mut stdin = child.stdin.take().unwrap();
    let stdout = child.stdout.take().unwrap();
    let mut reader = BufReader::new(stdout);

    let start = Instant::now();
    send_jsonrpc(
        &mut stdin,
        1,
        "initialize",
        r#"{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"test","version":"0.1"}}"#,
    );
    let init = read_jsonrpc(&mut reader);
    assert!(init.get("result").is_some(), "initialize failed: {}", init);

    let init_notif = concat!(
        r#"{"jsonrpc":"2.0","method":"notifications/initialized","params":{}}"#,
        "\n"
    );
    stdin.write_all(init_notif.as_bytes()).unwrap();
    stdin.flush().unwrap();

    send_jsonrpc(&mut stdin, 2, "tools/list", "{}");
    let resp = read_jsonrpc(&mut reader);
    let elapsed = start.elapsed();

    assert!(
        elapsed < Duration::from_secs(5),
        "tools/list took {}ms, expected <5000ms",
        elapsed.as_millis()
    );

    let tools = resp["result"]["tools"]
        .as_array()
        .expect("tools array missing");
    assert!(
        !tools.is_empty(),
        "expected tools to be listed even without IRIS connection"
    );

    child.kill().ok();
}

/// T010: web prefix is included in Atelier request URL.
/// Verifies that IRIS_WEB_PREFIX is correctly incorporated into the base URL.
#[test]
fn web_prefix_in_connection_url() {
    use iris_dev_core::iris::connection::{DiscoverySource, IrisConnection};

    // Construct a connection with a prefix in the base_url (as mcp.rs does)
    let base_url = "http://localhost:80/irisaicore".to_string();
    let conn = IrisConnection::new(
        base_url,
        "USER",
        "_SYSTEM",
        "SYS",
        DiscoverySource::ExplicitFlag,
    );

    let url = conn.atelier_url("/v8/USER/action/compile");
    assert!(
        url.contains("/irisaicore/api/atelier/"),
        "prefix missing from URL: {}",
        url
    );
    assert_eq!(
        url,
        "http://localhost:80/irisaicore/api/atelier/v8/USER/action/compile"
    );
}
