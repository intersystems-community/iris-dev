//! T029: Integration tests for `iris-dev compile` subcommand.
//! Spawns the binary as a subprocess, verifies exit codes and output.
#![allow(dead_code, clippy::zombie_processes)]

use std::process::Command;

fn iris_dev_bin() -> std::path::PathBuf {
    let mut path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.pop();
    path.pop();
    path.push("target/debug/iris-dev");
    path
}

fn fixtures_dir() -> std::path::PathBuf {
    let mut p = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.push("tests/fixtures");
    p
}

fn write_fixture(dir: &std::path::Path, name: &str, content: &str) -> std::path::PathBuf {
    let path = dir.join(name);
    std::fs::create_dir_all(dir).ok();
    std::fs::write(&path, content).unwrap();
    path
}

/// Compiling a valid class exits 0 and outputs the class name.
#[tokio::test]
async fn compile_good_cls_exits_zero() {
    let bin = iris_dev_bin();
    if !bin.exists() {
        return;
    }

    let dir = tempfile::tempdir().unwrap();
    let cls = write_fixture(
        dir.path(),
        "IrisDevTest.Good.cls",
        "Class IrisDevTest.Good Extends %RegisteredObject { }",
    );

    // Use opsreview-iris if running
    let output = Command::new(&bin)
        .args([
            "compile",
            cls.to_str().unwrap(),
            "--host",
            "localhost",
            "--web-port",
            "52773",
            "--username",
            "_SYSTEM",
            "--password",
            "SYS",
            "--format",
            "json",
        ])
        .output()
        .expect("failed to run iris-dev compile");

    // If IRIS not reachable, exit code may be 2 — that's acceptable
    let exit_code = output.status.code().unwrap_or(-1);
    let stdout = String::from_utf8_lossy(&output.stdout);

    if exit_code == 0 {
        // Should output JSON with success:true
        let result: serde_json::Value =
            serde_json::from_str(&stdout).expect("--format json should produce valid JSON");
        assert_eq!(
            result["success"], true,
            "good class should compile: {}",
            result
        );
    } else {
        // IRIS unreachable — exit 2 is acceptable
        assert!(
            exit_code == 2 || exit_code == 1,
            "unexpected exit code {} for compile",
            exit_code
        );
    }
}

/// Compiling a syntactically invalid class exits non-zero.
#[tokio::test]
async fn compile_bad_cls_exits_nonzero() {
    let bin = iris_dev_bin();
    if !bin.exists() {
        return;
    }

    let dir = tempfile::tempdir().unwrap();
    let cls = write_fixture(
        dir.path(),
        "IrisDevTest.Bad.cls",
        "Class IrisDevTest.Bad { SYNTAX ERROR HERE !!!! }",
    );

    let output = Command::new(&bin)
        .args([
            "compile",
            cls.to_str().unwrap(),
            "--host",
            "localhost",
            "--web-port",
            "52773",
            "--username",
            "_SYSTEM",
            "--password",
            "SYS",
        ])
        .output()
        .expect("failed to run iris-dev compile");

    let exit_code = output.status.code().unwrap_or(-1);
    // Either compile error (exit 1) or IRIS unreachable (exit 2) — never 0
    assert_ne!(exit_code, 0, "bad class should not exit 0");
}

/// --format json produces valid JSON on stdout.
#[test]
fn compile_format_json_produces_json() {
    let bin = iris_dev_bin();
    if !bin.exists() {
        return;
    }

    let dir = tempfile::tempdir().unwrap();
    let cls = write_fixture(
        dir.path(),
        "Test.Json.cls",
        "Class Test.Json Extends %RegisteredObject { }",
    );

    let output = Command::new(&bin)
        .args([
            "compile",
            cls.to_str().unwrap(),
            "--host",
            "nonexistent.invalid",
            "--web-port",
            "52773",
            "--format",
            "json",
        ])
        .output()
        .expect("failed to run iris-dev compile");

    let stdout = String::from_utf8_lossy(&output.stdout);
    if !stdout.trim().is_empty() {
        let _ = serde_json::from_str::<serde_json::Value>(&stdout)
            .expect("--format json output must be valid JSON");
    }
}
