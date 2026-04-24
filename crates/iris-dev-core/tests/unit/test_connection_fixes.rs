// Tests for connection.rs fixes: query() namespace, Debug redaction, container caching, banner stripping.

use iris_dev_core::iris::connection::{DiscoverySource, IrisConnection};

fn make_conn(password: &str) -> IrisConnection {
    IrisConnection::new(
        "http://localhost:52773",
        "USER",
        "_SYSTEM",
        password,
        DiscoverySource::ExplicitFlag,
    )
}

// ── T009: Debug redaction ────────────────────────────────────────────────────

#[test]
fn test_password_redacted_in_debug() {
    let conn = make_conn("supersecret123");
    let debug_output = format!("{:?}", conn);
    assert!(
        !debug_output.contains("supersecret123"),
        "password must not appear in Debug output, got: {}",
        debug_output
    );
    assert!(
        debug_output.contains("[redacted]"),
        "debug output should contain [redacted], got: {}",
        debug_output
    );
}

// ── T009: query() namespace ──────────────────────────────────────────────────
// We test the URL-building logic indirectly by verifying versioned_ns_url
// uses the passed namespace. Since query() delegates to versioned_ns_url,
// the test for the URL builder covers the contract.

#[test]
fn test_versioned_ns_url_uses_passed_namespace() {
    let conn = make_conn("SYS");
    let url = conn.versioned_ns_url("MYNS", "/action/query");
    assert!(
        url.contains("/MYNS/"),
        "URL should contain the passed namespace MYNS, got: {}",
        url
    );
    assert!(
        !url.contains("/USER/"),
        "URL should NOT contain the connection default USER, got: {}",
        url
    );
}

// ── T009: Banner stripping ───────────────────────────────────────────────────

#[test]
fn test_banner_stripped_from_output() {
    // Real IRIS session: banner, then bare prompt, then code output on its own line, then bare prompt.
    let raw = "Copyright (c) 2024 InterSystems Corporation\nAll rights reserved.\nIRIS for UNIX (Apple Mac OS X for x86-64) 2024.1\nUSER>\n42\nUSER>\n";
    let stripped = iris_dev_core::iris::connection::strip_iris_banner(raw);
    assert_eq!(
        stripped.trim(),
        "42",
        "expected only code output, got: {:?}",
        stripped
    );
}

#[test]
fn test_banner_strip_preserves_multiline_output() {
    // Multiline output: banner, bare prompt, two output lines, bare prompt.
    let raw = "Copyright (c) 2024 InterSystems Corporation\nUSER>\nline1\nline2\nUSER>\n";
    let stripped = iris_dev_core::iris::connection::strip_iris_banner(raw);
    let trimmed = stripped.trim();
    assert!(
        trimmed.contains("line1"),
        "should contain line1, got: {:?}",
        trimmed
    );
    assert!(
        trimmed.contains("line2"),
        "should contain line2, got: {:?}",
        trimmed
    );
    assert!(
        !trimmed.contains("Copyright"),
        "should not contain Copyright, got: {:?}",
        trimmed
    );
}

#[test]
fn test_banner_strip_noop_on_clean_output() {
    let raw = "hello world\n";
    let stripped = iris_dev_core::iris::connection::strip_iris_banner(raw);
    assert_eq!(stripped.trim(), "hello world");
}

// ── T021: http_client error handling ────────────────────────────────────────

#[test]
fn test_http_client_succeeds_normally() {
    // When TLS is not broken, http_client should succeed.
    let result = IrisConnection::http_client();
    assert!(
        result.is_ok(),
        "http_client should succeed in normal environment"
    );
}

// ── IDEV-3: sentinel Write ! ─────────────────────────────────────────────

#[test]
fn test_execute_captures_output_without_trailing_newline() {
    // build_exec_class must inject a sentinel Write ! after user code
    // so that Read line:0 always finds a line boundary.
    let lines = iris_dev_core::iris::connection::IrisConnection::build_exec_class_for_test(
        "TestClass",
        "/tmp/test.txt",
        "Write 42",
    );
    // Find the user code line
    let user_line_pos = lines
        .iter()
        .position(|l| l.contains("Write 42"))
        .expect("should contain user code");
    // The line immediately after user code must be the sentinel
    let sentinel_line = lines
        .get(user_line_pos + 1)
        .expect("should have line after user code");
    assert!(
        sentinel_line.contains("Write !"),
        "sentinel 'Write !' must follow user code, got: {:?}",
        sentinel_line
    );
}

#[test]
fn test_build_exec_class_sentinel_not_duplicated() {
    let lines = iris_dev_core::iris::connection::IrisConnection::build_exec_class_for_test(
        "TestClass",
        "/tmp/test.txt",
        "Write 42,!",
    );
    // Count sentinel occurrences — must be exactly one "Write !" line
    let sentinel_count = lines.iter().filter(|l| l.trim() == "Write !").count();
    assert_eq!(
        sentinel_count, 1,
        "exactly one sentinel Write ! should be present, got {}",
        sentinel_count
    );
}

#[test]
fn test_execute_captures_multiline_without_trailing_newline() {
    // FR-007: multi-line output where last line has no trailing ! must be fully captured.
    let lines = iris_dev_core::iris::connection::IrisConnection::build_exec_class_for_test(
        "TestClass",
        "/tmp/test.txt",
        "Write \"line1\",!\nWrite \"line2\"",
    );
    // Sentinel must appear after all user code lines
    let sentinel_pos = lines
        .iter()
        .rposition(|l| l.trim() == "Write !")
        .expect("sentinel must exist");
    let last_user_pos = lines
        .iter()
        .rposition(|l| l.contains("Write \"line2\""))
        .expect("user code must exist");
    assert!(
        sentinel_pos > last_user_pos,
        "sentinel must come after last user code line"
    );
}

// ── FR-009–FR-011: HTTP path handles long code ───────────────────────────

#[test]
fn test_build_exec_class_handles_long_code() {
    // The HTTP path (build_exec_class) must handle code strings of any length.
    // Generate a 200-char string literal and verify it appears intact in the generated class.
    let long_string: String = "A".repeat(200);
    let code = format!("Write \"{}\"", long_string);
    let lines = iris_dev_core::iris::connection::IrisConnection::build_exec_class_for_test(
        "TestClass",
        "/tmp/test.txt",
        &code,
    );
    // The full 200-char string must appear in the generated lines without truncation
    let found = lines.iter().any(|l| l.contains(&long_string));
    assert!(
        found,
        "200-char string must appear intact in generated class lines"
    );
}
