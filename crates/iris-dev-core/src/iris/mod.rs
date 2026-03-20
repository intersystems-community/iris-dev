pub mod connection;
pub mod discovery;
pub mod vscode_config;

pub use connection::{IrisConnection, DiscoverySource};
pub use discovery::{discover_iris, probe_atelier};
