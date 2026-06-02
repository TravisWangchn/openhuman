//! Network environment detection for OpenHuman-ZN.
//!
//! Detects whether China-hosted API endpoints are directly reachable
//! (no VPN/proxy required) at startup so the system can configure
//! routing appropriately instead of guessing.

mod probe;
pub use probe::{detect_environment, NetworkEnvironment, Reachability};
