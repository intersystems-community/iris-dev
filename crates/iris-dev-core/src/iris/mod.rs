pub mod connection;
pub mod discovery;

pub use connection::{IrisConnection, DiscoverySource};
pub use discovery::discover_iris;
