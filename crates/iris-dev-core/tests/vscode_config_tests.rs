//! T027: Unit tests for VS Code settings.json parsing.
//! Tests the vscode_config module for named server resolution.

use iris_dev_core::iris::vscode_config::{parse_vscode_settings, ObjectScriptConn};

fn write_settings(dir: &std::path::Path, content: &str) -> std::path::PathBuf {
    let path = dir.join("settings.json");
    std::fs::write(&path, content).unwrap();
    path
}

/// Parse settings.json with a direct host/port connection.
#[test]
fn parse_direct_connection() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_settings(
        dir.path(),
        r#"{
        "objectscript.conn": {
            "active": true,
            "host": "localhost",
            "port": 52773,
            "username": "_SYSTEM",
            "password": "SYS",
            "ns": "USER"
        }
    }"#,
    );

    let settings = parse_vscode_settings(&path).expect("should parse direct connection");
    let conn = settings
        .objectscript_conn
        .expect("objectscript.conn should be present");

    assert_eq!(conn.host.as_deref(), Some("localhost"));
    assert_eq!(conn.port, Some(52773));
    assert_eq!(conn.username.as_deref(), Some("_SYSTEM"));
    assert_eq!(conn.ns.as_deref(), Some("USER"));
    assert!(conn.server.is_none());
}

/// Parse settings.json with a named server — resolves superServer.port.
#[test]
fn parse_named_server_with_super_server_port() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_settings(
        dir.path(),
        r#"{
        "objectscript.conn": {
            "active": true,
            "server": "opsreview-iris",
            "ns": "USER"
        },
        "intersystems.servers": {
            "opsreview-iris": {
                "webServer": {
                    "scheme": "http",
                    "host": "localhost",
                    "port": 52773
                },
                "superServer": { "port": 1972 },
                "username": "_SYSTEM"
            }
        }
    }"#,
    );

    let settings = parse_vscode_settings(&path).expect("should parse named server");
    let conn = settings
        .objectscript_conn
        .expect("objectscript.conn present");
    assert_eq!(conn.server.as_deref(), Some("opsreview-iris"));

    let servers = settings
        .intersystems_servers
        .expect("intersystems.servers present");
    let server = servers
        .get("opsreview-iris")
        .expect("opsreview-iris server present");
    assert_eq!(server.web_server.host.as_deref(), Some("localhost"));
    assert_eq!(server.web_server.port, Some(52773));
    assert_eq!(server.super_server_port(), Some(1972));
}

/// Named server without superServer.port returns None for super_server_port.
#[test]
fn named_server_without_super_server_port_returns_none() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_settings(
        dir.path(),
        r#"{
        "objectscript.conn": {"active": true, "server": "myserver"},
        "intersystems.servers": {
            "myserver": {
                "webServer": {"host": "myiris.example.com", "port": 52773},
                "username": "admin"
            }
        }
    }"#,
    );

    let settings = parse_vscode_settings(&path).unwrap();
    let servers = settings.intersystems_servers.unwrap();
    let server = servers.get("myserver").unwrap();
    assert!(
        server.super_server_port().is_none(),
        "superServer absent should return None for super_server_port"
    );
}

/// Active=false connection is respected.
#[test]
fn inactive_connection_is_parsed() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_settings(
        dir.path(),
        r#"{
        "objectscript.conn": {
            "active": false,
            "host": "localhost",
            "port": 52773
        }
    }"#,
    );

    let settings = parse_vscode_settings(&path).unwrap();
    let conn = settings.objectscript_conn.unwrap();
    assert_eq!(conn.active, Some(false));
}

/// Missing objectscript.conn is Ok with None.
#[test]
fn settings_without_objectscript_conn_is_ok() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_settings(dir.path(), r#"{"editor.fontSize": 14}"#);
    let settings = parse_vscode_settings(&path).unwrap();
    assert!(settings.objectscript_conn.is_none());
}
