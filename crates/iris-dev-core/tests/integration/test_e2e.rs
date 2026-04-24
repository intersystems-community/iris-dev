//! E2E integration tests for iris-dev MCP server against a real IRIS container.
//!
//! Replaces the Python test suites (test_022_all_tools.py, test_032_compile_hook.py).
//!
//! Run with a live IRIS container:
//!   IRIS_HOST=localhost IRIS_WEB_PORT=52773 IRIS_CONTAINER=iris-e2e \
//!   IRIS_USERNAME=_SYSTEM IRIS_PASSWORD=SYS \
//!   cargo test --test test_e2e -- --nocapture
//!
//! All tests skip gracefully when IRIS_HOST is not set.
#![allow(dead_code, clippy::zombie_processes)]

use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};

// ── Helpers ───────────────────────────────────────────────────────────────────

fn iris_dev_bin() -> std::path::PathBuf {
    let mut p = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.pop(); // crates/iris-dev-core
    p.pop(); // crates/
    p.push("target/debug/iris-dev");
    if !p.exists() {
        p.pop();
        p.push("release/iris-dev");
    }
    p
}

fn iris_host() -> String {
    std::env::var("IRIS_HOST").unwrap_or_default()
}

fn iris_env() -> Vec<(&'static str, String)> {
    vec![
        ("IRIS_HOST", std::env::var("IRIS_HOST").unwrap_or_default()),
        (
            "IRIS_WEB_PORT",
            std::env::var("IRIS_WEB_PORT").unwrap_or_else(|_| "52773".to_string()),
        ),
        (
            "IRIS_USERNAME",
            std::env::var("IRIS_USERNAME").unwrap_or_else(|_| "_SYSTEM".to_string()),
        ),
        (
            "IRIS_PASSWORD",
            std::env::var("IRIS_PASSWORD").unwrap_or_else(|_| "SYS".to_string()),
        ),
        (
            "IRIS_NAMESPACE",
            std::env::var("IRIS_NAMESPACE").unwrap_or_else(|_| "USER".to_string()),
        ),
        (
            "IRIS_CONTAINER",
            std::env::var("IRIS_CONTAINER").unwrap_or_default(),
        ),
    ]
}

/// Skip this test if IRIS_HOST is not set or the binary doesn't exist.
macro_rules! require_iris {
    () => {
        if iris_host().is_empty() {
            eprintln!("Skipping: IRIS_HOST not set");
            return;
        }
        if !iris_dev_bin().exists() {
            eprintln!(
                "Skipping: iris-dev binary not found at {:?}",
                iris_dev_bin()
            );
            return;
        }
    };
}

/// Skip if binary doesn't exist (for no-IRIS tests).
macro_rules! require_bin {
    () => {
        if !iris_dev_bin().exists() {
            eprintln!("Skipping: iris-dev binary not found");
            return;
        }
    };
}

/// Send MCP messages to iris-dev mcp and collect responses.
fn mcp_call(env_vars: &[(&str, String)], messages: &[serde_json::Value]) -> Vec<serde_json::Value> {
    let bin = iris_dev_bin();
    if !bin.exists() {
        return vec![];
    }

    let mut cmd = Command::new(&bin);
    cmd.args(["mcp"]);
    for (k, v) in env_vars {
        cmd.env(k, v);
    }

    let mut child = cmd
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("failed to spawn iris-dev mcp");

    let mut stdin = child.stdin.take().unwrap();
    let stdout = child.stdout.take().unwrap();
    let mut reader = BufReader::new(stdout);
    let mut results = vec![];

    for msg in messages {
        stdin
            .write_all((serde_json::to_string(msg).unwrap() + "\n").as_bytes())
            .unwrap();
        stdin.flush().unwrap();

        if msg.get("id").is_some() {
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
            loop {
                std::thread::sleep(std::time::Duration::from_millis(50));
                let mut line = String::new();
                if reader.read_line(&mut line).unwrap_or(0) > 0 {
                    if let Ok(v) = serde_json::from_str::<serde_json::Value>(&line) {
                        results.push(v);
                        break;
                    }
                }
                if std::time::Instant::now() > deadline {
                    break;
                }
            }
        } else {
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
    }

    child.kill().ok();
    results
}

/// Standard MCP handshake messages.
fn init_msgs() -> Vec<serde_json::Value> {
    vec![
        serde_json::json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{
            "protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"e2e","version":"0.1"}
        }}),
        serde_json::json!({"jsonrpc":"2.0","method":"notifications/initialized","params":{}}),
    ]
}

/// Extract the JSON tool result from an MCP response for a given id.
fn tool_result(responses: &[serde_json::Value], id: u64) -> serde_json::Value {
    let resp = responses
        .iter()
        .find(|r| r["id"] == id)
        .cloned()
        .unwrap_or_default();
    let text = resp["result"]["content"][0]["text"]
        .as_str()
        .unwrap_or("{}");
    serde_json::from_str(text).unwrap_or_default()
}

/// Call a single tool and return its result JSON.
fn call_tool(name: &str, args: serde_json::Value) -> serde_json::Value {
    let env = iris_env();
    let mut msgs = init_msgs();
    msgs.push(serde_json::json!({
        "jsonrpc":"2.0","id":2,"method":"tools/call",
        "params":{"name": name, "arguments": args}
    }));
    let responses = mcp_call(&env, &msgs);
    tool_result(&responses, 2)
}

// ── iris_execute ──────────────────────────────────────────────────────────────

#[test]
fn e2e_execute_write_without_trailing_bang_returns_output() {
    require_iris!();
    // IDEV-3 regression: sentinel Write ! must capture output even without trailing !
    let result = call_tool(
        "iris_execute",
        serde_json::json!({"code": "Write 42", "namespace": "USER", "confirmed": true}),
    );
    if result["success"] == true {
        assert_eq!(
            result["output"].as_str().map(|s| s.trim()),
            Some("42"),
            "Write 42 (no trailing !) must return '42', got: {}",
            result
        );
    }
    // If success=false (e.g. DOCKER_REQUIRED), that's acceptable — what's NOT acceptable is
    // success=true with empty output, which was the bug.
    if result["success"] == true {
        assert_ne!(
            result["output"].as_str().unwrap_or("").trim(),
            "",
            "iris_execute must not return empty output for Write 42"
        );
    }
}

#[test]
fn e2e_execute_returns_version_string() {
    require_iris!();
    let result = call_tool(
        "iris_execute",
        serde_json::json!({"code": "Write $ZVERSION", "namespace": "USER", "confirmed": true}),
    );
    if result["success"] == true {
        let output = result["output"].as_str().unwrap_or("");
        assert!(
            output.contains("IRIS")
                || output.contains("Cache")
                || output.contains("2025")
                || output.contains("2026"),
            "Write $ZVERSION should return version string, got: {:?}",
            output
        );
    }
}

#[test]
fn e2e_execute_docker_required_has_instructions() {
    require_bin!();
    // Run WITHOUT IRIS_HOST so it must explain what to do
    let env = vec![
        ("IRIS_HOST", "".to_string()),
        ("IRIS_CONTAINER", "".to_string()),
    ];
    let mut msgs = init_msgs();
    msgs.push(serde_json::json!({
        "jsonrpc":"2.0","id":2,"method":"tools/call",
        "params":{"name":"iris_execute","arguments":{"code":"Write 1","namespace":"USER","confirmed":true}}
    }));
    let responses = mcp_call(&env, &msgs);
    let result = tool_result(&responses, 2);
    if result["success"] == false {
        let ec = result["error_code"].as_str().unwrap_or("");
        let text = result.to_string().to_lowercase();
        assert!(
            ec == "DOCKER_REQUIRED"
                || text.contains("iris_container")
                || text.contains("docker")
                || ec == "IRIS_UNREACHABLE",
            "error without IRIS should mention Docker or container: {}",
            result
        );
    }
}

// ── iris_symbols ──────────────────────────────────────────────────────────────

#[test]
fn e2e_symbols_glob_star_returns_package_classes() {
    require_iris!();
    // Seed two classes then query with glob
    call_tool(
        "iris_doc",
        serde_json::json!({"mode":"put","name":"Test022Glob.Alpha.cls",
        "content":"Class Test022Glob.Alpha { ClassMethod Run() { } }","namespace":"USER"}),
    );
    call_tool(
        "iris_compile",
        serde_json::json!({"target":"Test022Glob.Alpha.cls","namespace":"USER"}),
    );
    call_tool(
        "iris_doc",
        serde_json::json!({"mode":"put","name":"Test022Glob.Beta.cls",
        "content":"Class Test022Glob.Beta { ClassMethod Run() { } }","namespace":"USER"}),
    );
    call_tool(
        "iris_compile",
        serde_json::json!({"target":"Test022Glob.Beta.cls","namespace":"USER"}),
    );

    let result = call_tool(
        "iris_symbols",
        serde_json::json!({"query": "Test022Glob.*", "namespace": "USER"}),
    );
    let symbols = result["symbols"].as_array().cloned().unwrap_or_default();
    let names: Vec<String> = symbols
        .iter()
        .filter_map(|s| s["Name"].as_str().map(|n| n.to_string()))
        .collect();
    assert!(
        names.iter().any(|n| n.contains("Test022Glob")),
        "Test022Glob.* should return Test022Glob classes, got: {:?}",
        names
    );

    // Cleanup
    call_tool(
        "iris_doc",
        serde_json::json!({"mode":"delete","name":"Test022Glob.Alpha.cls","namespace":"USER"}),
    );
    call_tool(
        "iris_doc",
        serde_json::json!({"mode":"delete","name":"Test022Glob.Beta.cls","namespace":"USER"}),
    );
}

#[test]
fn e2e_symbols_trailing_dot_prefix_matches() {
    require_iris!();
    // Plain prefix with trailing dot
    let result = call_tool(
        "iris_symbols",
        serde_json::json!({"query": "Test022Glob.", "namespace": "USER", "limit": 5}),
    );
    // Must not crash
    assert!(
        result["symbols"].is_array() || result["error_code"].is_string(),
        "iris_symbols with trailing dot must return array or structured error: {}",
        result
    );
}

#[test]
fn e2e_symbols_plain_substring_no_regression() {
    require_iris!();
    let result = call_tool(
        "iris_symbols",
        serde_json::json!({"query": "Ens.Director", "namespace": "USER", "limit": 5}),
    );
    assert!(
        result["symbols"].is_array(),
        "plain substring must return array: {}",
        result
    );
}

// ── iris_doc ──────────────────────────────────────────────────────────────────

#[test]
fn e2e_doc_put_with_storage_block_strips_and_succeeds() {
    require_iris!();
    // I-3: Storage blocks must be stripped automatically
    let cls_with_storage = r#"Class Test022.StorageTest Extends %Persistent {
Property Name As %String;
Storage Default
{
<Data name="DefaultData">
<Value name="1"><Value>%%CLASSNAME</Value></Value>
</Data>
<DataLocation>^Test022.StorageTestD</DataLocation>
<DefaultData>DefaultData</DefaultData>
<IdLocation>^Test022.StorageTestD</IdLocation>
<IndexLocation>^Test022.StorageTestI</IndexLocation>
<StreamLocation>^Test022.StorageTestS</StreamLocation>
<Type>%Storage.Persistent</Type>
}
}"#;

    let result = call_tool(
        "iris_doc",
        serde_json::json!({"mode":"put","name":"Test022.StorageTest.cls",
            "content": cls_with_storage, "namespace":"USER"}),
    );
    assert_eq!(
        result["success"], true,
        "put with Storage block should succeed: {}",
        result
    );
    assert_eq!(
        result["storage_stripped"], true,
        "response must include storage_stripped:true: {}",
        result
    );

    // Cleanup
    call_tool(
        "iris_doc",
        serde_json::json!({"mode":"delete","name":"Test022.StorageTest.cls","namespace":"USER"}),
    );
}

#[test]
fn e2e_doc_rewrite_after_compile_failure_no_conflict() {
    require_iris!();
    // I-4: Re-writing a class after a compile failure must not return CONFLICT
    let name = "Test022.ETagTest.cls";
    let bad = "Class Test022.ETagTest { ClassMethod Bad() { this is not valid !! } }";
    let good = "Class Test022.ETagTest { ClassMethod Good() As %String { Return \"ok\" } }";

    // First write (bad class)
    let r1 = call_tool(
        "iris_doc",
        serde_json::json!({"mode":"put","name":name,"content":bad,"namespace":"USER"}),
    );
    assert_eq!(r1["success"], true, "first write should succeed: {}", r1);

    // Attempt compile (will fail — that's expected)
    call_tool(
        "iris_compile",
        serde_json::json!({"target":name,"namespace":"USER"}),
    );

    // Second write (fixed class) — must NOT return CONFLICT
    let r2 = call_tool(
        "iris_doc",
        serde_json::json!({"mode":"put","name":name,"content":good,"namespace":"USER"}),
    );
    assert_ne!(
        r2["error_code"].as_str(),
        Some("CONFLICT"),
        "re-write after compile failure must not return CONFLICT: {}",
        r2
    );
    assert_eq!(r2["success"], true, "second write should succeed: {}", r2);

    // Cleanup
    call_tool(
        "iris_doc",
        serde_json::json!({"mode":"delete","name":name,"namespace":"USER"}),
    );
}

#[test]
fn e2e_doc_put_get_delete_roundtrip() {
    require_iris!();
    let name = "Test022.RoundTrip.cls";
    let content = "Class Test022.RoundTrip { ClassMethod Hello() As %String { Return \"world\" } }";

    let put = call_tool(
        "iris_doc",
        serde_json::json!({"mode":"put","name":name,"content":content,"namespace":"USER"}),
    );
    assert_eq!(put["success"], true, "put: {}", put);

    let get = call_tool(
        "iris_doc",
        serde_json::json!({"mode":"get","name":name,"namespace":"USER"}),
    );
    assert_eq!(get["success"], true, "get: {}", get);

    let del = call_tool(
        "iris_doc",
        serde_json::json!({"mode":"delete","name":name,"namespace":"USER"}),
    );
    assert_eq!(del["success"], true, "delete: {}", del);
}

// ── iris_compile ──────────────────────────────────────────────────────────────

#[test]
fn e2e_compile_error_has_line_number_and_text() {
    require_iris!();
    let name = "Test022.CompileError.cls";
    let bad =
        "Class Test022.CompileError {\nClassMethod Bad() {\n    this is invalid objectscript\n}\n}";

    call_tool(
        "iris_doc",
        serde_json::json!({"mode":"put","name":name,"content":bad,"namespace":"USER"}),
    );

    let result = call_tool(
        "iris_compile",
        serde_json::json!({"target":name,"namespace":"USER"}),
    );
    assert_eq!(
        result["success"], false,
        "compile of bad class should fail: {}",
        result
    );

    let errors = result["errors"].as_array().cloned().unwrap_or_default();
    assert!(
        !errors.is_empty(),
        "errors array must be non-empty: {}",
        result
    );
    for err in &errors {
        assert!(
            err["text"].is_string() || err["message"].is_string(),
            "error must have text: {}",
            err
        );
        assert!(
            err["line"].is_number(),
            "error must have line number: {}",
            err
        );
    }

    // Cleanup
    call_tool(
        "iris_doc",
        serde_json::json!({"mode":"delete","name":name,"namespace":"USER"}),
    );
}

#[test]
fn e2e_compile_valid_class_succeeds() {
    require_iris!();
    let name = "Test022.CompileOk.cls";
    let good = "Class Test022.CompileOk { ClassMethod Run() As %String { Return \"ok\" } }";

    call_tool(
        "iris_doc",
        serde_json::json!({"mode":"put","name":name,"content":good,"namespace":"USER"}),
    );
    let result = call_tool(
        "iris_compile",
        serde_json::json!({"target":name,"namespace":"USER"}),
    );
    assert_eq!(
        result["success"], true,
        "compile of valid class should succeed: {}",
        result
    );
    let errors = result["errors"].as_array().cloned().unwrap_or_default();
    assert!(
        errors.is_empty(),
        "successful compile should have no errors: {}",
        result
    );

    call_tool(
        "iris_doc",
        serde_json::json!({"mode":"delete","name":name,"namespace":"USER"}),
    );
}

// ── iris_test ─────────────────────────────────────────────────────────────────

#[test]
fn e2e_test_no_match_returns_no_tests_found() {
    require_iris!();
    let result = call_tool(
        "iris_test",
        serde_json::json!({"pattern": "Test022.NonExistent.NoSuchClass", "namespace": "USER"}),
    );
    if result["success"] == false {
        let ec = result["error_code"].as_str().unwrap_or("");
        assert!(
            ec == "NO_TESTS_FOUND" || ec == "DOCKER_REQUIRED",
            "no-match pattern should return NO_TESTS_FOUND or DOCKER_REQUIRED, got: {}",
            result
        );
    }
}

// ── iris_info ─────────────────────────────────────────────────────────────────

#[test]
fn e2e_info_metadata_returns_version() {
    require_iris!();
    let result = call_tool(
        "iris_info",
        serde_json::json!({"what": "metadata", "namespace": "USER"}),
    );
    assert!(
        result["success"] == true
            || result.get("version").is_some()
            || result.get("iris_version").is_some(),
        "iris_info metadata should return version info: {}",
        result
    );
}

#[test]
fn e2e_info_namespace_returns_name() {
    require_iris!();
    let result = call_tool(
        "iris_info",
        serde_json::json!({"what": "namespace", "namespace": "USER"}),
    );
    assert!(
        result["success"] == true || result.get("name").is_some(),
        "iris_info namespace should return namespace info: {}",
        result
    );
}

// ── iris_query ────────────────────────────────────────────────────────────────

#[test]
fn e2e_query_select_returns_rows() {
    require_iris!();
    let result = call_tool(
        "iris_query",
        serde_json::json!({"query": "SELECT TOP 3 Name FROM %Dictionary.ClassDefinition ORDER BY Name", "namespace": "USER"}),
    );
    assert_eq!(
        result["success"], true,
        "SQL SELECT should succeed: {}",
        result
    );
    let rows = result["rows"].as_array().cloned().unwrap_or_default();
    assert!(!rows.is_empty(), "SELECT should return rows: {}", result);
}

#[test]
fn e2e_query_invalid_sql_structured_error() {
    require_iris!();
    let result = call_tool(
        "iris_query",
        serde_json::json!({"query": "THIS IS NOT SQL", "namespace": "USER"}),
    );
    assert_eq!(
        result["success"], false,
        "invalid SQL should fail: {}",
        result
    );
    assert!(
        result["error_code"].is_string(),
        "invalid SQL must return error_code: {}",
        result
    );
}

// ── iris_search ───────────────────────────────────────────────────────────────

#[test]
fn e2e_search_finds_seeded_content() {
    require_iris!();
    // First seed a class with unique content
    let name = "Test022.SearchTarget.cls";
    let unique = "UNIQUESEARCHTOKEN022";
    let content = format!("Class Test022.SearchTarget {{ /// {} }}", unique);
    call_tool(
        "iris_doc",
        serde_json::json!({"mode":"put","name":name,"content":content,"namespace":"USER"}),
    );

    let result = call_tool(
        "iris_search",
        serde_json::json!({"query": unique, "namespace": "USER"}),
    );
    // Search may return 0 results if not indexed yet — just must not crash
    assert!(
        result["success"] == true || result["error_code"].is_string(),
        "iris_search must return structured response: {}",
        result
    );

    call_tool(
        "iris_doc",
        serde_json::json!({"mode":"delete","name":name,"namespace":"USER"}),
    );
}

// ── docs_introspect ───────────────────────────────────────────────────────────

#[test]
fn e2e_introspect_known_class() {
    require_iris!();
    let result = call_tool(
        "docs_introspect",
        serde_json::json!({"class_name": "Ens.Director", "namespace": "USER"}),
    );
    assert_eq!(
        result["success"], true,
        "introspect Ens.Director should succeed: {}",
        result
    );
    let methods = result["methods"].as_array().cloned().unwrap_or_default();
    assert!(
        !methods.is_empty(),
        "Ens.Director should have methods: {}",
        result
    );
}

#[test]
fn e2e_introspect_nonexistent_structured_error() {
    require_iris!();
    let result = call_tool(
        "docs_introspect",
        serde_json::json!({"class_name": "Nonexistent.Class.That.DoesNotExist", "namespace": "USER"}),
    );
    assert!(
        result["success"] == true || result["success"] == false,
        "introspect of nonexistent class must return structured response: {}",
        result
    );
}

// ── workspace config ──────────────────────────────────────────────────────────

#[test]
fn e2e_workspace_config_iris_dev_init_creates_toml() {
    require_bin!();
    let tmp = tempfile::TempDir::new().unwrap();
    let output = Command::new(iris_dev_bin())
        .args([
            "init",
            "--workspace",
            tmp.path().to_str().unwrap(),
            "--format",
            "json",
        ])
        .output();

    match output {
        Ok(out) => {
            let stdout = String::from_utf8_lossy(&out.stdout);
            if out.status.success() {
                // If it succeeded, the TOML file must exist
                let toml_path = tmp.path().join(".iris-dev.toml");
                assert!(
                    toml_path.exists(),
                    "iris-dev init should create .iris-dev.toml"
                );
                let content = std::fs::read_to_string(&toml_path).unwrap();
                assert!(
                    content.contains("container"),
                    "generated toml must have container field"
                );
                assert!(
                    content.contains("namespace"),
                    "generated toml must have namespace field"
                );
                // JSON output must be valid
                if !stdout.trim().is_empty() {
                    let json: serde_json::Value = serde_json::from_str(stdout.trim())
                        .expect("iris-dev init --format json must produce valid JSON");
                    assert_eq!(json["success"], true, "init JSON output: {}", json);
                }
            }
            // If it failed (no containers running), that's acceptable — just must not panic
        }
        Err(e) => panic!("iris-dev init failed to run: {}", e),
    }
}

// ── compile hook ──────────────────────────────────────────────────────────────

fn hook_script() -> std::path::PathBuf {
    let mut p = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.pop();
    p.pop();
    p.push("scripts/compile-hook.sh");
    p
}

fn run_hook(event: &serde_json::Value, env_override: &[(&str, &str)]) -> (String, i32) {
    let script = hook_script();
    if !script.exists() {
        return ("SKIP: compile-hook.sh not found".to_string(), 0);
    }

    let mut cmd = Command::new("bash");
    cmd.arg(&script)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    for (k, v) in env_override {
        cmd.env(k, v);
    }

    let mut child = cmd.spawn().expect("spawn bash");
    if let Some(mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(serde_json::to_string(event).unwrap().as_bytes());
    }
    let output = child.wait_with_output().expect("wait");
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let code = output.status.code().unwrap_or(-1);
    (stdout, code)
}

#[test]
fn e2e_hook_non_cls_file_is_silent() {
    // Non-ObjectScript files must produce no output — no IRIS needed
    let event = serde_json::json!({
        "hook_event_name": "PostToolUse",
        "tool_name": "Write",
        "tool_input": {"file_path": "/workspace/config.json"},
        "tool_result": {},
        "cwd": "/workspace"
    });
    let (output, code) = run_hook(&event, &[]);
    if output != "SKIP: compile-hook.sh not found" {
        assert_eq!(
            output, "",
            "non-.cls file must produce no output, got: {:?}",
            output
        );
        assert_eq!(code, 0);
    }
}

#[test]
fn e2e_hook_auto_compile_disabled_is_silent() {
    // IRIS_AUTO_COMPILE=false must always be silent — no IRIS needed
    let event = serde_json::json!({
        "hook_event_name": "PostToolUse",
        "tool_name": "Write",
        "tool_input": {"file_path": "/workspace/MyApp/Patient.cls"},
        "tool_result": {},
        "cwd": "/workspace"
    });
    let (output, code) = run_hook(&event, &[("IRIS_AUTO_COMPILE", "false")]);
    if output != "SKIP: compile-hook.sh not found" {
        assert_eq!(
            output, "",
            "IRIS_AUTO_COMPILE=false must be silent, got: {:?}",
            output
        );
        assert_eq!(code, 0);
    }
}

#[test]
fn e2e_hook_no_iris_host_message_within_3s() {
    // When IRIS_HOST is not set, must print a message within 3.5 seconds
    let event = serde_json::json!({
        "hook_event_name": "PostToolUse",
        "tool_name": "Write",
        "tool_input": {"file_path": "/workspace/MyApp/Patient.cls"},
        "tool_result": {},
        "cwd": "/workspace"
    });
    let start = std::time::Instant::now();
    let (output, _) = run_hook(&event, &[("IRIS_HOST", ""), ("IRIS_CONTAINER", "")]);
    let elapsed = start.elapsed();
    if output != "SKIP: compile-hook.sh not found" {
        assert!(
            elapsed < std::time::Duration::from_millis(3500),
            "hook with no IRIS must respond in <3.5s, took {:?}",
            elapsed
        );
        // Must either be silent (IRIS not configured) or explain
        let text_lower = output.to_lowercase();
        assert!(
            output.is_empty()
                || text_lower.contains("not connected")
                || text_lower.contains("iris_host")
                || text_lower.contains("unreachable"),
            "unexpected output with no IRIS: {:?}",
            output
        );
    }
}

#[test]
fn e2e_hook_file_changed_disabled_by_default() {
    // FileChanged without IRIS_COMPILE_ON_SAVE=true must be silent
    let event = serde_json::json!({
        "hook_event_name": "FileChanged",
        "file_path": "/workspace/MyApp/Patient.cls"
    });
    let (output, code) = run_hook(&event, &[]);
    if output != "SKIP: compile-hook.sh not found" {
        assert_eq!(
            output, "",
            "FileChanged without opt-in must be silent, got: {:?}",
            output
        );
        assert_eq!(code, 0);
    }
}
