use anyhow::Result;
use clap::{Parser, Subcommand};
use tracing_subscriber::EnvFilter;

mod cmd;

#[derive(Parser)]
#[command(
    name = "iris-dev",
    version,
    about = "CLI and package manager for InterSystems IRIS developer ecosystem",
    long_about = None,
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    /// Enable debug logging
    #[arg(long, global = true)]
    verbose: bool,

    /// List discovered iris-dev-* plugin commands on PATH
    #[arg(long)]
    list_plugins: bool,
}

#[derive(Subcommand)]
enum Commands {
    /// Start the MCP server (stdio or HTTP transport)
    Mcp(cmd::mcp::McpCommand),
    /// Compile ObjectScript .cls files on IRIS
    Compile(cmd::compile::CompileCommand),
    /// Install packages from iris-dev.toml
    Install(cmd::install::InstallCommand),
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive(if cli.verbose {
            tracing::Level::DEBUG.into()
        } else {
            tracing::Level::WARN.into()
        }))
        .with_writer(std::io::stderr)
        .with_ansi(false)
        .init();

    if cli.list_plugins {
        cmd::plugin::list_plugins();
        return Ok(());
    }

    match cli.command {
        Some(Commands::Mcp(cmd)) => cmd.run().await,
        Some(Commands::Compile(cmd)) => cmd.run().await,
        Some(Commands::Install(cmd)) => cmd.run().await,
        None => {
            // Check for iris-dev-* plugin on PATH before giving up
            let args: Vec<String> = std::env::args().collect();
            if args.len() > 1 {
                cmd::plugin::try_dispatch_plugin(&args[1], &args[2..])?;
            }
            eprintln!("Run `iris-dev --help` for usage.");
            std::process::exit(1);
        }
    }
}
