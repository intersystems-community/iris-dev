use anyhow::Result;
use clap::Args;
use iris_dev_core::{iris::discovery::discover_iris, skills::SkillRegistry, tools::IrisTools};
use rmcp::{transport::stdio, ServiceExt};
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
    #[arg(long, env = "IRIS_WEB_PREFIX", default_value = "")]
    pub web_prefix: String,
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
            use iris_dev_core::iris::connection::{DiscoverySource, IrisConnection};
            let port = self.web_port.unwrap_or(52773);
            let prefix = self.web_prefix.trim_matches('/');
            let base_url = if prefix.is_empty() {
                format!("http://{}:{}", host, port)
            } else {
                format!("http://{}:{}/{}", host, port, prefix)
            };
            let username = self.username.as_deref().unwrap_or("_SYSTEM");
            let password = self.password.as_deref().unwrap_or("SYS");
            Some(IrisConnection::new(
                base_url,
                &self.namespace,
                username,
                password,
                DiscoverySource::ExplicitFlag,
            ))
        } else {
            None
        };

        let (iris_tx, iris_rx) =
            watch::channel::<Option<iris_dev_core::iris::connection::IrisConnection>>(None);

        tokio::spawn(async move {
            let conn = match discover_iris(explicit).await {
                Ok(c) => c,
                Err(e) => {
                    tracing::warn!("IRIS discovery error: {}", e);
                    None
                }
            };
            if let Some(ref c) = conn {
                tracing::info!(
                    "IRIS connected: {}/api/atelier/{} {}",
                    c.base_url,
                    c.atelier_version.version_str(),
                    c.version.as_deref().unwrap_or("?")
                );
            } else {
                tracing::warn!("No IRIS connection — tools return IRIS_UNREACHABLE");
            }
            let _ = iris_tx.send(conn);
        });

        let mut registry = SkillRegistry::new();
        for owner_repo in &self.subscribe {
            match registry.load_from_github(owner_repo).await {
                Ok(()) => tracing::info!("Subscribed to {}", owner_repo),
                Err(e) => tracing::warn!("Failed to subscribe to {}: {}", owner_repo, e),
            }
        }

        // Do NOT wait for discovery here — start serving immediately so MCP clients
        // (Claude Code, Copilot) get the initialize response within their timeout window.
        // Tools read iris_rx dynamically; if discovery hasn't completed yet they return
        // IRIS_UNREACHABLE, which is the correct behavior until IRIS is found.
        let iris = iris_rx.borrow().clone();

        // On Windows, stdout opens in text mode which translates \n → \r\n.
        // MCP clients expect bare \n-terminated JSON lines — set stdout/stdin to binary mode.
        #[cfg(windows)]
        unsafe {
            extern "C" {
                fn _setmode(fd: i32, mode: i32) -> i32;
            }
            const O_BINARY: i32 = 0x8000;
            _setmode(0, O_BINARY); // stdin
            _setmode(1, O_BINARY); // stdout
        }

        let service = IrisTools::with_registry(iris, registry)
            .serve(stdio())
            .await
            .inspect_err(|e| tracing::error!("MCP server error: {:?}", e))?;
        service.waiting().await?;
        Ok(())
    }
}
