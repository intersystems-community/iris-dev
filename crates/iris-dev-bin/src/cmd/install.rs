use anyhow::Result;
use clap::Args;

#[derive(Args)]
pub struct InstallCommand {
    /// Use exact versions from iris-dev.lock (no resolution)
    #[arg(long)]
    pub locked: bool,
    /// Show what would be installed without installing
    #[arg(long)]
    pub dry_run: bool,
}

impl InstallCommand {
    pub async fn run(self) -> Result<()> {
        // TODO: parse iris-dev.toml, run resolver, write iris-dev.lock, download packages
        anyhow::bail!("iris-dev install: not yet implemented");
    }
}
