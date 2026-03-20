use anyhow::Result;
use clap::Args;
use rmcp::{ServiceExt, transport::stdio};
use iris_dev_core::{iris::discovery::discover_iris, tools::IrisTools};
use std::sync::Arc;
use tokio::sync::watch;

#[derive(Args)]
pub struct McpCommand {
    #[arg(long, default_value = "stdio")]
    pub transport: String,
    #[arg(long, default_value = "8080")]
    pub port: u16,
    #[arg(long, env = "IRIS_HOST")]
    pub host: Option<String>,
    #[arg(long, env = "IRIS_WEB_PORT")]
    pub web_port: Option<u16>,
    #[arg(long, env = "IRIS_USERNAME")]
    pub username: Option<String>,
    #[arg(long, env = "IRIS_PASSWORD")]
    pub password: Option<String>,
    #[arg(long, env = "IRIS_NAMESPACE", default_value = "USER")]
    pub namespace: String,
    #[arg(long)]
    pub server: Option<String>,
    #[arg(long)]
    pub config: Option<String>,
    #[arg(long = "subscribe")]
    pub subscribe: Vec<String>,
    #[arg(long, default_value = ".")]
    pub workspace: String,
}

impl McpCommand {
    pub async fn run(self) -> Result<()> {
        tracing::info!("iris-dev mcp starting");

        let explicit = if let Some(host) = self.host.clone() {
            use iris_dev_core::iris::connection::{IrisConnection, DiscoverySource};
            let port = self.web_port.unwrap_or(52773);
            let username = self.username.as_deref().unwrap_or("_SYSTEM");
            let password = self.password.as_deref().unwrap_or("SYS");
            let base_url = format!("http://{}:{}", host, port);
            Some(IrisConnection::new(base_url, &self.namespace, username, password, DiscoverySource::ExplicitFlag))
        } else {
            None
        };

        // Start IRIS discovery in background; start MCP server immediately
        // so rmcp can begin buffering stdin (initialize message) right away.
        let (iris_tx, iris_rx) = watch::channel::<Option<iris_dev_core::iris::connection::IrisConnection>>(None);
        
        tokio::spawn(async move {
            let conn = match discover_iris(explicit).await {
                Ok(c) => c,
                Err(e) => { tracing::warn!("IRIS discovery error: {}", e); None }
            };
            if let Some(ref c) = conn {
                tracing::info!("IRIS connected: {} v{}", c.base_url, c.version.as_deref().unwrap_or("?"));
            } else {
                tracing::warn!("No IRIS connection — tools return IRIS_UNREACHABLE");
            }
            let _ = iris_tx.send(conn);
        });

        // Wait briefly for fast discoveries (env var, already-running localhost)
        // before starting the MCP server, so tools are connected on first call.
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
        let iris = iris_rx.borrow().clone();

        let service = IrisTools::new(iris).serve(stdio()).await
            .inspect_err(|e| tracing::error!("MCP server error: {:?}", e))?;
        service.waiting().await?;
        Ok(())
    }
}
