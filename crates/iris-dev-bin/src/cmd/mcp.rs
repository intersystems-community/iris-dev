use anyhow::Result;
use clap::Args;
use rmcp::{ServiceExt, transport::stdio};
use iris_dev_core::{iris::discovery::discover_iris, tools::IrisTools};

#[derive(Args)]
pub struct McpCommand {
    /// Transport type
    #[arg(long, default_value = "stdio")]
    pub transport: String,
    /// HTTP port when --transport http
    #[arg(long, default_value = "8080")]
    pub port: u16,
    /// IRIS host (skips discovery cascade)
    #[arg(long, env = "IRIS_HOST")]
    pub host: Option<String>,
    /// IRIS web port
    #[arg(long, env = "IRIS_WEB_PORT")]
    pub web_port: Option<u16>,
    /// IRIS username
    #[arg(long, env = "IRIS_USERNAME")]
    pub username: Option<String>,
    /// IRIS password
    #[arg(long, env = "IRIS_PASSWORD")]
    pub password: Option<String>,
    /// IRIS namespace
    #[arg(long, env = "IRIS_NAMESPACE", default_value = "USER")]
    pub namespace: String,
    /// Named server from --config file
    #[arg(long)]
    pub server: Option<String>,
    /// Path to VS Code settings.json or iris-dev-config.json
    #[arg(long)]
    pub config: Option<String>,
    /// Subscribe to a KB skills GitHub repo (repeatable: --subscribe owner/repo)
    #[arg(long = "subscribe")]
    pub subscribe: Vec<String>,
    /// Workspace root for local file operations
    #[arg(long, default_value = ".")]
    pub workspace: String,
}

impl McpCommand {
    pub async fn run(self) -> Result<()> {
        tracing::info!("iris-dev mcp starting");

        // Discover IRIS connection
        let iris = discover_iris(None).await?;
        if let Some(ref conn) = iris {
            tracing::info!("IRIS connected: {} ({})", conn.base_url, conn.version.as_deref().unwrap_or("unknown version"));
        } else {
            tracing::warn!("No IRIS connection found — tools requiring IRIS will return IRIS_UNREACHABLE");
        }

        // TODO: load --subscribe packages into skill registry

        let service = IrisTools::new(iris).serve(stdio()).await
            .inspect_err(|e| tracing::error!("MCP server error: {:?}", e))?;

        service.waiting().await?;
        Ok(())
    }
}
