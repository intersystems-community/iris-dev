//! T035: Unit tests for iris-dev plugin discovery and dispatch.

use std::os::unix::fs::PermissionsExt;
use std::process::Command;

fn iris_dev_bin() -> std::path::PathBuf {
    let mut path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.pop();
    path.push("target/debug/iris-dev");
    path
}

/// --list-plugins runs without error.
#[test]
fn list_plugins_exits_zero() {
    let bin = iris_dev_bin();
    if !bin.exists() { return; }

    let output = Command::new(&bin)
        .arg("--list-plugins")
        .output()
        .expect("failed to run --list-plugins");

    assert!(output.status.success(),
        "iris-dev --list-plugins should exit 0, got: {}", output.status);
}

/// Unknown command exits non-zero.
#[test]
fn unknown_command_exits_nonzero() {
    let bin = iris_dev_bin();
    if !bin.exists() { return; }

    let output = Command::new(&bin)
        .arg("totally-unknown-command-xyzzy")
        .output()
        .expect("failed to run iris-dev");

    assert!(!output.status.success(),
        "unknown command should exit non-zero");
}

/// iris-dev-* plugin on PATH is discovered and dispatched.
#[test]
#[cfg(unix)]
fn plugin_on_path_is_dispatched() {
    let bin = iris_dev_bin();
    if !bin.exists() { return; }

    let dir = tempfile::tempdir().unwrap();
    let plugin = dir.path().join("iris-dev-testplugin");

    // Write a simple shell script that exits 0 and prints a marker
    std::fs::write(&plugin, "#!/bin/sh\necho 'TESTPLUGIN_OK'\nexit 0\n").unwrap();
    let mut perms = std::fs::metadata(&plugin).unwrap().permissions();
    perms.set_mode(0o755);
    std::fs::set_permissions(&plugin, perms).unwrap();

    // Prepend our dir to PATH
    let original_path = std::env::var("PATH").unwrap_or_default();
    let new_path = format!("{}:{}", dir.path().display(), original_path);

    let output = Command::new(&bin)
        .arg("testplugin")
        .env("PATH", &new_path)
        .output()
        .expect("failed to dispatch plugin");

    assert!(output.status.success(),
        "plugin dispatch should exit 0, got: {}", output.status);

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("TESTPLUGIN_OK"),
        "plugin output should contain marker, got: {}", stdout);
}

/// Plugin passes remaining args correctly.
#[test]
#[cfg(unix)]
fn plugin_receives_args() {
    let bin = iris_dev_bin();
    if !bin.exists() { return; }

    let dir = tempfile::tempdir().unwrap();
    let plugin = dir.path().join("iris-dev-argtest");
    std::fs::write(&plugin, "#!/bin/sh\necho \"ARGS: $@\"\nexit 0\n").unwrap();
    let mut perms = std::fs::metadata(&plugin).unwrap().permissions();
    perms.set_mode(0o755);
    std::fs::set_permissions(&plugin, perms).unwrap();

    let original_path = std::env::var("PATH").unwrap_or_default();
    let new_path = format!("{}:{}", dir.path().display(), original_path);

    let output = Command::new(&bin)
        .args(["argtest", "--foo", "bar"])
        .env("PATH", &new_path)
        .output()
        .expect("failed to dispatch plugin");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("--foo") && stdout.contains("bar"),
        "plugin should receive all args, got: {}", stdout);
}
