use anyhow::Result;

/// List all iris-dev-* binaries discovered on PATH.
pub fn list_plugins() {
    let prefix = "iris-dev-";
    let paths = std::env::var("PATH").unwrap_or_default();
    let mut plugins = vec![];
    for dir in std::env::split_paths(&paths) {
        if let Ok(entries) = std::fs::read_dir(&dir) {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                if name.starts_with(prefix) {
                    let cmd = &name[prefix.len()..];
                    plugins.push((cmd.to_string(), entry.path()));
                }
            }
        }
    }
    plugins.sort();
    plugins.dedup_by_key(|(name, _)| name.clone());
    if plugins.is_empty() {
        println!("No iris-dev-* plugins found on PATH.");
    } else {
        println!("Discovered plugins:");
        for (name, path) in plugins {
            println!("  {} → {}", name, path.display());
        }
    }
}

/// If a binary named iris-dev-{cmd} exists on PATH, exec it with the remaining args.
/// Never returns on Unix (process is replaced). Returns Ok on Windows after child exits.
pub fn try_dispatch_plugin(cmd: &str, args: &[String]) -> Result<()> {
    let binary = format!("iris-dev-{}", cmd);
    match which::which(&binary) {
        Ok(path) => {
            #[cfg(unix)]
            {
                use std::os::unix::process::CommandExt;
                let err = std::process::Command::new(&path).args(args).exec();
                anyhow::bail!("failed to exec {}: {}", path.display(), err);
            }
            #[cfg(not(unix))]
            {
                let status = std::process::Command::new(&path).args(args).status()?;
                std::process::exit(status.code().unwrap_or(1));
            }
        }
        Err(_) => {
            eprintln!(
                "iris-dev: unknown command '{}'\nRun `iris-dev --help` for available commands.",
                cmd
            );
            std::process::exit(1);
        }
    }
}
