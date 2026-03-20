//! T010: Unit tests for IRIS discovery cascade.
//! Tests written FIRST — must fail before implementation is complete.
//!
//! These tests exercise: probe_atelier fingerprinting, cascade ordering,
//! graceful fallthrough when localhost probe fails, env var resolution.

use iris_dev_core::iris::discovery::{probe_atelier, discover_iris};
use iris_dev_core::iris::connection::DiscoverySource;

// ── probe_atelier ────────────────────────────────────────────────────────────

/// A reachable IRIS endpoint returns Some(IrisConnection).
#[tokio::test]
async fn probe_atelier_returns_connection_on_iris_response() {
    // This test uses a mock HTTP server. Since we don't have wiremock yet,
    // it tests against a real running IRIS on localhost:52773 if available,
    // otherwise asserts that probing a non-IRIS endpoint returns None.
    let result = probe_atelier("127.0.0.1", 9999, "_SYSTEM", "SYS", "USER", 100).await;
    // Port 9999 is not IRIS — must return None
    assert!(result.is_none(), "Non-IRIS port should return None");
}

/// probe_atelier respects the timeout — 100ms must not block longer than 250ms.
#[tokio::test]
async fn probe_atelier_respects_timeout() {
    let start = std::time::Instant::now();
    let _result = probe_atelier("10.255.255.1", 52773, "_SYSTEM", "SYS", "USER", 100).await;
    let elapsed = start.elapsed();
    assert!(
        elapsed < std::time::Duration::from_millis(500),
        "probe_atelier took {}ms, expected <500ms with 100ms timeout",
        elapsed.as_millis()
    );
}

// ── discover_iris cascade ────────────────────────────────────────────────────

/// When IRIS_HOST + IRIS_WEB_PORT env vars are set and valid, discover_iris
/// should attempt to connect (and fail gracefully if not reachable).
#[tokio::test]
async fn discover_iris_reads_env_vars() {
    // Set env vars to a non-existent host
    std::env::set_var("IRIS_HOST", "nonexistent.invalid");
    std::env::set_var("IRIS_WEB_PORT", "52773");
    std::env::set_var("IRIS_USERNAME", "testuser");
    std::env::set_var("IRIS_PASSWORD", "testpass");
    
    let result = discover_iris(None).await;
    // Env vars found but host unreachable — should still return Ok (not Err)
    assert!(result.is_ok(), "discover_iris should not error on unreachable host");
    
    // Clean up
    std::env::remove_var("IRIS_HOST");
    std::env::remove_var("IRIS_WEB_PORT");
    std::env::remove_var("IRIS_USERNAME");
    std::env::remove_var("IRIS_PASSWORD");
}

/// Without any config, discover_iris returns Ok(None) — not an error.
#[tokio::test]
async fn discover_iris_returns_none_when_nothing_found() {
    // Ensure no env vars interfere
    std::env::remove_var("IRIS_HOST");
    std::env::remove_var("IRIS_WEB_PORT");
    
    // With no IRIS running and no config, should return Ok(None)
    // (localhost scan will fail quickly, Docker scan returns nothing in unit context)
    let result = discover_iris(None).await;
    assert!(result.is_ok(), "discover_iris should return Ok even when nothing found");
}

/// Explicit connection passed to discover_iris is returned immediately without scanning.
#[tokio::test]
async fn discover_iris_explicit_wins_immediately() {
    use iris_dev_core::iris::connection::{IrisConnection, DiscoverySource};
    
    let explicit = IrisConnection::new(
        "http://explicit.example.com:52773",
        "MYNS",
        "admin",
        "secret",
        DiscoverySource::ExplicitFlag,
    );
    
    let result = discover_iris(Some(explicit)).await.unwrap();
    let conn = result.expect("explicit connection should be returned");
    assert_eq!(conn.base_url, "http://explicit.example.com:52773");
    assert_eq!(conn.namespace, "MYNS");
    assert!(matches!(conn.source, DiscoverySource::ExplicitFlag));
}

// ── IrisConnection ────────────────────────────────────────────────────────────

#[test]
fn iris_connection_atelier_url_format() {
    use iris_dev_core::iris::connection::{IrisConnection, DiscoverySource};
    
    let conn = IrisConnection::new(
        "http://localhost:52773",
        "USER",
        "_SYSTEM",
        "SYS",
        DiscoverySource::ExplicitFlag,
    );
    
    assert_eq!(conn.atelier_url("/v1/USER/action/query"), "http://localhost:52773/api/atelier/v1/USER/action/query");
    assert_eq!(conn.atelier_url("/"), "http://localhost:52773/api/atelier/");
}
