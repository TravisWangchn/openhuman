mod core;
pub mod ops;
mod schemas;

pub mod audit;
pub mod bubblewrap;
pub mod detect;
pub mod docker;
pub mod firejail;
pub mod landlock;
pub mod pairing;
pub mod policy;
pub mod secrets;
pub mod traits;

#[allow(unused_imports)]
pub use audit::{AuditEvent, AuditEventType, AuditLogger};
pub use core::*;
#[allow(unused_imports)]
pub use detect::create_sandbox;
pub use ops as rpc;
pub use ops::*;
#[allow(unused_imports)]
pub use pairing::PairingGuard;
#[allow(unused_imports)]
pub use policy::AutonomyLevel;
pub use policy::SecurityPolicy;
pub use policy::ToolOperation;
#[allow(unused_imports)]
pub use secrets::SecretStore;
#[allow(unused_imports)]
pub use traits::{NoopSandbox, Sandbox};

pub use schemas::{
    all_controller_schemas as all_security_controller_schemas,
    all_registered_controllers as all_security_registered_controllers,
};

/// Validate that a resolved candidate path stays within the workspace root,
/// preventing path traversal via `..` components or symlink escapes.
///
/// Canonicalizes both paths and checks that `candidate` starts with `workspace_root`.
/// Returns the resolved path on success, or `None` when the candidate is outside
/// the workspace or cannot be resolved.
pub fn validate_workspace_path(
    workspace_root: &std::path::Path,
    candidate: &std::path::Path,
) -> Option<std::path::PathBuf> {
    let resolved_candidate = std::fs::canonicalize(candidate).ok()?;
    let resolved_root = std::fs::canonicalize(workspace_root).ok()?;
    if resolved_candidate.starts_with(&resolved_root) {
        Some(resolved_candidate)
    } else {
        log::warn!(
            "[security] path traversal blocked: {} is outside workspace {}",
            candidate.display(),
            workspace_root.display()
        );
        None
    }
}
