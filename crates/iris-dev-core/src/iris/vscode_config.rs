//! Parse VS Code settings.json for IRIS connection configuration.
//! Supports both direct host/port connections and named server references.

use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;
use crate::iris::connection::{IrisConnection, DiscoverySource};

#[derive(Debug, Deserialize, Default)]
pub struct VsCodeSettings {
    #[serde(rename = "objectscript.conn")]
    pub objectscript_conn: Option<ObjectScriptConn>,
    #[serde(rename = "intersystems.servers")]
    pub intersystems_servers: Option<HashMap<String, IntersystemsServer>>,
}

#[derive(Debug, Deserialize)]
pub struct ObjectScriptConn {
    pub active: Option<bool>,
    pub host: Option<String>,
    pub port: Option<u16>,
    pub username: Option<String>,
    pub password: Option<String>,
    pub ns: Option<String>,
    /// Named server reference (key into intersystems.servers)
    pub server: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct IntersystemsServer {
    #[serde(rename = "webServer")]
    pub web_server: WebServerSpec,
    #[serde(rename = "superServer")]
    pub super_server: Option<SuperServerSpec>,
    pub username: Option<String>,
    pub password: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct WebServerSpec {
    pub host: Option<String>,
    pub port: Option<u16>,
    pub scheme: Option<String>,
    #[serde(rename = "pathPrefix")]
    pub path_prefix: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct SuperServerSpec {
    pub host: Option<String>,
    pub port: Option<u16>,
}

impl IntersystemsServer {
    /// Returns the native SuperServer port if configured.
    pub fn super_server_port(&self) -> Option<u16> {
        self.super_server.as_ref().and_then(|ss| ss.port)
    }
}

/// Parse a VS Code settings.json file.
pub fn parse_vscode_settings(path: impl AsRef<Path>) -> anyhow::Result<VsCodeSettings> {
    let content = std::fs::read_to_string(path.as_ref())?;
    // VS Code settings.json may have trailing commas or comments — use serde_json's lenient parser
    let settings: VsCodeSettings = serde_json::from_str(&content)
        .unwrap_or_else(|_| {
            // Try stripping JS-style comments (basic)
            let cleaned: String = content.lines()
                .filter(|l| !l.trim_start().starts_with("//"))
                .collect::<Vec<_>>()
                .join("\n");
            serde_json::from_str(&cleaned).unwrap_or_default()
        });
    Ok(settings)
}

impl VsCodeSettings {
    /// Convert parsed settings to an IrisConnection, resolving named servers.
    pub async fn to_iris_connection(&self) -> Option<IrisConnection> {
        let conn = self.objectscript_conn.as_ref()?;
        if conn.active == Some(false) { return None; }

        // Named server path
        if let Some(server_name) = &conn.server {
            let servers = self.intersystems_servers.as_ref()?;
            let server = servers.get(server_name)?;
            let host = server.web_server.host.as_deref().unwrap_or("localhost");
            let web_port = server.web_server.port.unwrap_or(52773);
            let scheme = server.web_server.scheme.as_deref().unwrap_or("http");
            let base_url = format!("{}://{}:{}", scheme, host, web_port);
            let username = server.username.as_deref().unwrap_or("_SYSTEM");
            let password = server.password.as_deref().unwrap_or("SYS");
            let ns = conn.ns.as_deref().unwrap_or("USER");

            let iris_conn = IrisConnection::new(base_url, ns, username, password, DiscoverySource::VsCodeSettings);
            // Note: super_server_port is available if needed for native connections
            return Some(iris_conn);
        }

        // Direct host/port path
        let host = conn.host.as_deref().unwrap_or("localhost");
        let port = conn.port.unwrap_or(52773);
        let username = conn.username.as_deref().unwrap_or("_SYSTEM");
        let password = conn.password.as_deref().unwrap_or("SYS");
        let ns = conn.ns.as_deref().unwrap_or("USER");
        let base_url = format!("http://{}:{}", host, port);

        Some(IrisConnection::new(base_url, ns, username, password, DiscoverySource::VsCodeSettings))
    }
}
