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
    assert_eq!(stripped.trim(), "42", "expected only code output, got: {:?}", stripped);
}

#[test]
fn test_banner_strip_preserves_multiline_output() {
    // Multiline output: banner, bare prompt, two output lines, bare prompt.
    let raw = "Copyright (c) 2024 InterSystems Corporation\nUSER>\nline1\nline2\nUSER>\n";
    let stripped = iris_dev_core::iris::connection::strip_iris_banner(raw);
    let trimmed = stripped.trim();
    assert!(trimmed.contains("line1"), "should contain line1, got: {:?}", trimmed);
    assert!(trimmed.contains("line2"), "should contain line2, got: {:?}", trimmed);
    assert!(!trimmed.contains("Copyright"), "should not contain Copyright, got: {:?}", trimmed);
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
    assert!(result.is_ok(), "http_client should succeed in normal environment");
}
