//! Per-workspace IRIS connection config via `.iris-dev.toml`.
//!
//! Priority order: CLI flags > .iris-dev.toml > env vars > auto-discovery.

use crate::iris::connection::{DiscoverySource, IrisConnection};
use serde::Deserialize;
use std::path::PathBuf;

/// Parsed contents of `.iris-dev.toml`. All fields are optional.
#[derive(Debug, Deserialize, Default, Clone)]
pub struct WorkspaceConfig {
    pub container: Option<String>,
    pub namespace: Option<String>,
    pub host: Option<String>,
    pub web_port: Option<u16>,
    /// URL path prefix for the IRIS web gateway, e.g. "irisaicore" when the
    /// Atelier API is served at http://host:port/irisaicore/api/atelier/...
    /// Corresponds to intersystems.servers[x].webServer.pathPrefix in VS Code settings.
    pub web_prefix: Option<String>,
    /// URL scheme: "http" or "https". Defaults to "http".
    /// Set to "https" for TLS-protected IRIS web gateways.
    pub scheme: Option<String>,
    pub username: Option<String>,
    pub password: Option<String>,
}

/// Resolve the workspace root path.
/// Priority: OBJECTSCRIPT_WORKSPACE env var > workspace_path arg > walk up from cwd.
///
/// When no explicit path is given, walks up from current_dir() looking for .iris-dev.toml
/// (git-style discovery). This ensures the config is found even when the MCP server is
/// launched from a parent directory (e.g. by an IDE that sets cwd to the home directory).
pub fn workspace_root(workspace_path: Option<&str>) -> PathBuf {
    if let Ok(ws) = std::env::var("OBJECTSCRIPT_WORKSPACE") {
        if !ws.is_empty() {
            return PathBuf::from(ws);
        }
    }
    if let Some(p) = workspace_path {
        if !p.is_empty() && p != "." {
            return PathBuf::from(p);
        }
    }
    // Walk up from current directory looking for .iris-dev.toml
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let mut dir = cwd.as_path();
    loop {
        if dir.join(".iris-dev.toml").exists() {
            return dir.to_path_buf();
        }
        match dir.parent() {
            Some(parent) => dir = parent,
            None => break,
        }
    }
    cwd
}

/// Load `.iris-dev.toml` from the resolved workspace root.
/// Returns `None` if the file does not exist (not an error).
/// Logs a warning and returns `None` on parse errors — never panics.
pub fn load_workspace_config(workspace_path: Option<&str>) -> Option<WorkspaceConfig> {
    let root = workspace_root(workspace_path);
    let config_path = root.join(".iris-dev.toml");

    if !config_path.exists() {
        return None;
    }

    match std::fs::read_to_string(&config_path) {
        Err(e) => {
            tracing::warn!(
                "Could not read .iris-dev.toml at {}: {}",
                config_path.display(),
                e
            );
            None
        }
        Ok(contents) => match toml::from_str::<WorkspaceConfig>(&contents) {
            Ok(cfg) => {
                tracing::debug!("Loaded .iris-dev.toml from {}", config_path.display());
                Some(cfg)
            }
            Err(e) => {
                tracing::warn!(
                    "Could not parse .iris-dev.toml at {}: {}",
                    config_path.display(),
                    e
                );
                None
            }
        },
    }
}

/// Apply workspace config to set up the connection environment.
///
/// If `host` is specified: returns `Some(IrisConnection)` that will be passed directly
/// to `discover_iris()` as the explicit override.
///
/// If `container` is specified (but not host): sets `IRIS_CONTAINER` (and optionally
/// `IRIS_NAMESPACE`, `IRIS_USERNAME`, `IRIS_PASSWORD`) so the standard discovery cascade
/// picks up the container. Returns `None` to let discovery proceed normally.
///
/// If neither is specified: returns `None` — no connection info in the config.
pub fn workspace_config_to_connection(
    cfg: &WorkspaceConfig,
    namespace_default: &str,
) -> Option<IrisConnection> {
    // host + web_port → explicit HTTP/HTTPS connection (highest priority, no docker needed)
    if let Some(ref host) = cfg.host {
        let port = cfg.web_port.unwrap_or(52773);
        let scheme = cfg
            .scheme
            .clone()
            .or_else(|| std::env::var("IRIS_SCHEME").ok())
            .map(|s| s.trim_matches('/').to_string())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "http".to_string());
        let prefix = cfg
            .web_prefix
            .clone()
            .or_else(|| std::env::var("IRIS_WEB_PREFIX").ok())
            .map(|p| p.trim_matches('/').to_string())
            .filter(|p| !p.is_empty());
        let base_url = match prefix {
            Some(p) => format!("{}://{}:{}/{}", scheme, host, port, p),
            None => format!("{}://{}:{}", scheme, host, port),
        };
        let namespace = cfg
            .namespace
            .clone()
            .or_else(|| std::env::var("IRIS_NAMESPACE").ok())
            .unwrap_or_else(|| namespace_default.to_string());
        let username = cfg
            .username
            .clone()
            .or_else(|| std::env::var("IRIS_USERNAME").ok())
            .unwrap_or_else(|| "_SYSTEM".to_string());
        let password = cfg
            .password
            .clone()
            .or_else(|| std::env::var("IRIS_PASSWORD").ok())
            .unwrap_or_else(|| "SYS".to_string());
        return Some(IrisConnection::new(
            base_url,
            namespace,
            username,
            password,
            DiscoverySource::EnvVar,
        ));
    }

    // container → inject into env so discover_iris() docker step picks it up
    if let Some(ref container) = cfg.container {
        std::env::set_var("IRIS_CONTAINER", container);
        if let Some(ref ns) = cfg.namespace {
            std::env::set_var("IRIS_NAMESPACE", ns);
        }
        if let Some(ref user) = cfg.username {
            std::env::set_var("IRIS_USERNAME", user);
        }
        if let Some(ref pass) = cfg.password {
            std::env::set_var("IRIS_PASSWORD", pass);
        }
        return None; // discover_iris() will find the container via IRIS_CONTAINER
    }

    None
}

/// Apply workspace config to an existing explicit connection override.
///
/// If `explicit` is already set (from CLI flags), returns it unchanged.
/// Otherwise loads `.iris-dev.toml` from `workspace_path` and applies it:
/// - `host` config → returns `Some(IrisConnection)`
/// - `container` config → sets `IRIS_CONTAINER` env var, returns `None`
/// - no config / no relevant fields → returns `None`
pub fn apply_workspace_config(
    explicit: Option<IrisConnection>,
    workspace_path: Option<&str>,
    namespace: &str,
) -> Option<IrisConnection> {
    if explicit.is_some() {
        return explicit;
    }
    let cfg = load_workspace_config(workspace_path)?;
    explicit.or_else(|| workspace_config_to_connection(&cfg, namespace))
}

/// Generate starter `.iris-dev.toml` content with inline comments.
/// Used by `iris-dev init`.
pub fn generate_toml_content(container: &str, namespace: &str) -> String {
    format!(
        r#"# iris-dev workspace configuration
# Commit this file to share connection settings with your team.

# Docker container name (for local development)
container = "{container}"

# Default IRIS namespace
namespace = "{namespace}"

# Alternative: direct host connection (for remote or CI IRIS)
# host = "iris.example.com"
# web_port = 52773
# web_prefix = ""  # URL path prefix, e.g. "irisaicore" when Atelier is at /irisaicore/api/atelier/
# scheme = "http"  # Use "https" for TLS-protected IRIS web gateways

# Credentials (optional)
# Use IRIS_USERNAME / IRIS_PASSWORD env vars instead of committing credentials.
# username = "_SYSTEM"
# password = "..."  # not recommended in committed files
"#
    )
}
