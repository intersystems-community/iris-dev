use anyhow::Result;
use clap::Args;

#[derive(Args)]
pub struct CompileCommand {
    /// Class name or path to .cls file (omit to compile all .cls in workspace)
    pub target: Option<String>,
    #[arg(long, env = "IRIS_HOST")]
    pub host: Option<String>,
    #[arg(long, env = "IRIS_WEB_PORT", default_value = "52773")]
    pub web_port: u16,
    #[arg(long, env = "IRIS_NAMESPACE", default_value = "USER")]
    pub namespace: String,
    #[arg(long, env = "IRIS_USERNAME")]
    pub username: Option<String>,
    #[arg(long, env = "IRIS_PASSWORD")]
    pub password: Option<String>,
    #[arg(long, default_value = "cuk")]
    pub flags: String,
    #[arg(long)]
    pub force_writable: bool,
    #[arg(long, default_value = "text")]
    pub format: String,
}

impl CompileCommand {
    pub async fn run(self) -> Result<()> {
        // TODO: call iris_dev_core compile logic directly (not via MCP)
        anyhow::bail!("iris-dev compile: not yet implemented");
    }
}
