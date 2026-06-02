// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OpenHuman-ZN Contributors

//! GATE闸机 — startup self-check system for OpenHuman-ZN.
//!
//! Inspired by the reviewreply-cn-v2 GATE pattern. Five gate levels
//! validate the runtime environment at startup before the core proceeds
//! to serve RPC requests:
//!
//! | Gate   | Check                     | Severity | Error Code            |
//! |--------|---------------------------|----------|-----------------------|
//! | GATE-1 | Binary file integrity     | Critical | `GATE_001_FILE_HASH`  |
//! | GATE-2 | API key validation        | Critical | `GATE_002_NO_API_KEY` |
//! | GATE-3 | Model endpoint connectivity| Warning  | `GATE_003_CONNECTIVITY` |
//! | GATE-4 | Payment webhook reachable | Warning  | `GATE_004_WEBHOOK`    |
//! | GATE-5 | Disk space quota          | Warning  | `GATE_005_DISK_QUOTA` |
//!
//! # Safe Mode
//!
//! When any **Critical** gate fails, [`SafeModeConfig`] is returned alongside
//! the [`GateReport`]. Safe mode disables all cloud-dependent features
//! (model inference, webhook delivery, external sync) and restricts the
//! system to local-only operation.
//!
//! # Usage
//!
//! ```no_run
//! use openhuman_core::doctor::gate::run_gate_checks;
//!
//! let status = run_gate_checks().await;
//! if !status.all_passed {
//!     eprintln!("{}", status.summary());
//! }
//! ```

use chrono::{DateTime, Utc};
use log::{info, warn};
use serde::{Deserialize, Serialize};
use thiserror::Error;

// ---------------------------------------------------------------------------
// Sub-modules — one file per gate check
// ---------------------------------------------------------------------------

mod integrity;
pub use integrity::check_binary_integrity;

mod attestation;
pub use attestation::check_api_keys;

mod connectivity;
pub use connectivity::{check_model_connectivity, check_payment_webhook};

mod quota;
pub use quota::check_disk_quota;

#[cfg(test)]
mod tests;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Minimum acceptable free disk space for the SQLite database and Obsidian
/// vault, in bytes (100 MiB).
const MIN_DISK_SPACE_BYTES: u64 = 100 * 1024 * 1024;

/// Minimum length for a valid API key. LLM provider keys (DeepSeek, Doubao)
/// are typically 32+ characters; 12 is a generous lower bound.
const MIN_API_KEY_LENGTH: usize = 12;

/// Environment variable for the expected SHA-256 hash of the core binary.
const ENV_EXPECTED_HASH: &str = "OPENHUMAN_CORE_EXPECTED_HASH";

/// Environment variable override for the data directory (disk space check).
const ENV_DATA_DIR: &str = "OPENHUMAN_DATA_DIR";

/// Environment variable for the primary model API key (DeepSeek).
const ENV_DEEPSEEK_KEY: &str = "DEEPSEEK_API_KEY";

/// Environment variable for the fallback model API key (Doubao).
const ENV_DOUBAO_KEY: &str = "DOUBAO_API_KEY";

/// Feature name constants used in [`SafeModeConfig`].
const FEATURE_MODEL_INFERENCE: &str = "model_inference";
const FEATURE_WEBHOOK_DELIVERY: &str = "webhook_delivery";
const FEATURE_CREDENTIAL_REFRESH: &str = "credential_refresh";
const FEATURE_TELEMETRY_EXPORT: &str = "telemetry_export";

// ---------------------------------------------------------------------------
// Error Codes
// ---------------------------------------------------------------------------

/// Structured error codes that identify which gate check failed.
///
/// Each variant maps to exactly one gate check and carries a machine-readable
/// string via [`Display`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Error)]
pub enum GateErrorCode {
    /// Binary file SHA-256 hash does not match the expected value.
    #[error("GATE_001_FILE_HASH")]
    Gate001FileHash,

    /// No configured API key meets the minimum length requirement.
    #[error("GATE_002_NO_API_KEY")]
    Gate002NoApiKey,

    /// Model provider endpoint connectivity could not be verified.
    #[error("GATE_003_CONNECTIVITY")]
    Gate003Connectivity,

    /// Payment webhook callback URL is not reachable.
    #[error("GATE_004_WEBHOOK")]
    Gate004Webhook,

    /// Available disk space is below the minimum threshold (100 MiB).
    #[error("GATE_005_DISK_QUOTA")]
    Gate005DiskQuota,
}

// ---------------------------------------------------------------------------
// Gate Identifiers
// ---------------------------------------------------------------------------

/// Identifies which gate a [`GateCheckResult`] belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GateCode {
    /// GATE-1: Binary file SHA-256 integrity check.
    Gate1FileIntegrity,

    /// GATE-2: API key environment variable validation.
    Gate2ApiKey,

    /// GATE-3: Model provider endpoint connectivity (deferred).
    Gate3ModelConnectivity,

    /// GATE-4: Payment webhook callback reachability (deferred).
    Gate4PaymentWebhook,

    /// GATE-5: Available disk space for SQLite and vault storage.
    Gate5DiskQuota,
}

impl GateCode {
    /// Returns the human-readable GATE label (e.g. `"GATE-1"`).
    pub const fn label(&self) -> &'static str {
        match self {
            Self::Gate1FileIntegrity => "GATE-1",
            Self::Gate2ApiKey => "GATE-2",
            Self::Gate3ModelConnectivity => "GATE-3",
            Self::Gate4PaymentWebhook => "GATE-4",
            Self::Gate5DiskQuota => "GATE-5",
        }
    }

    /// Returns the machine-readable error code for this gate.
    pub const fn error_code(&self) -> GateErrorCode {
        match self {
            Self::Gate1FileIntegrity => GateErrorCode::Gate001FileHash,
            Self::Gate2ApiKey => GateErrorCode::Gate002NoApiKey,
            Self::Gate3ModelConnectivity => GateErrorCode::Gate003Connectivity,
            Self::Gate4PaymentWebhook => GateErrorCode::Gate004Webhook,
            Self::Gate5DiskQuota => GateErrorCode::Gate005DiskQuota,
        }
    }
}

// ---------------------------------------------------------------------------
// Severity
// ---------------------------------------------------------------------------

/// Severity level for a gate check result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GateSeverity {
    /// Failure blocks startup. The system enters safe mode.
    Critical,

    /// Failure is logged but does not block or degrade startup.
    Warning,

    /// Informational observation — neither pass nor fail.
    Info,
}

// ---------------------------------------------------------------------------
// Check Result
// ---------------------------------------------------------------------------

/// The result of a single gate check.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GateCheckResult {
    /// Human-readable name of the check (e.g. `"GATE-1 文件完整性"`).
    pub name: String,

    /// Whether the check passed (`true`) or failed (`false`).
    pub passed: bool,

    /// Human-readable message describing the result or error detail.
    pub message: String,

    /// Severity level of this check.
    pub severity: GateSeverity,

    /// Machine-readable error code. Present when `passed` is `false`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<GateErrorCode>,

    /// The gate identifier for this check.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gate: Option<GateCode>,
}

// ---------------------------------------------------------------------------
// Aggregate Status
// ---------------------------------------------------------------------------

/// Aggregate status of all gate checks.
///
/// Returned by [`run_gate_checks`]. For a richer result that includes safe
/// mode information and suggested fixes, see [`GateReport`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GateStatus {
    /// `true` when no **Critical** gate has failed. Non-critical failures
    /// (Warning, Info) do not flip this to `false`.
    pub all_passed: bool,

    /// Individual check results in execution order.
    pub checks: Vec<GateCheckResult>,

    /// Number of checks with `passed == true`.
    pub passed_count: usize,

    /// Total number of checks run.
    pub total_count: usize,
}

impl GateStatus {
    /// Constructs a `GateStatus` from a list of check results.
    ///
    /// `all_passed` is `true` only when every check with
    /// [`GateSeverity::Critical`] has `passed == true`. Non-critical
    /// failures are tolerated.
    pub fn new(checks: Vec<GateCheckResult>) -> Self {
        let passed = checks.iter().filter(|c| c.passed).count();
        let total = checks.len();
        let critical_failed = checks
            .iter()
            .any(|c| !c.passed && c.severity == GateSeverity::Critical);
        Self {
            all_passed: !critical_failed,
            checks,
            passed_count: passed,
            total_count: total,
        }
    }

    /// Returns a one-line summary of the gate check results.
    pub fn summary(&self) -> String {
        let kind = if self.all_passed { "通过" } else { "失败" };
        format!(
            "闸机{kind}: {}/{} 项检查{kind}",
            self.passed_count, self.total_count
        )
    }
}

// ---------------------------------------------------------------------------
// Safe Mode
// ---------------------------------------------------------------------------

/// Configuration that disables cloud-dependent features when critical gates
/// fail.
///
/// In safe mode the system runs in a restricted local-only state:
///
/// * No outbound model inference (DeepSeek / Doubao).
/// * No webhook delivery to payment callbacks.
/// * No external credential refresh.
/// * No telemetry or metrics export.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SafeModeConfig {
    /// Whether safe mode is currently active.
    pub safe_mode: bool,

    /// Human-readable list of feature names that are disabled.
    pub disabled_features: Vec<String>,

    /// Explanation of why safe mode was entered.
    pub reason: String,
}

impl SafeModeConfig {
    /// Creates an active `SafeModeConfig` that disables all cloud features.
    pub fn new(reason: impl Into<String>) -> Self {
        Self {
            safe_mode: true,
            disabled_features: vec![
                FEATURE_MODEL_INFERENCE.into(),
                FEATURE_WEBHOOK_DELIVERY.into(),
                FEATURE_CREDENTIAL_REFRESH.into(),
                FEATURE_TELEMETRY_EXPORT.into(),
            ],
            reason: reason.into(),
        }
    }

    /// Returns an inactive safe mode config with all features enabled.
    pub fn inactive() -> Self {
        Self {
            safe_mode: false,
            disabled_features: vec![],
            reason: String::new(),
        }
    }
}

// ---------------------------------------------------------------------------
// Gate Report
// ---------------------------------------------------------------------------

/// Comprehensive report from the full gate check suite.
///
/// Includes the aggregate [`GateStatus`], optional safe mode configuration,
/// a list of human-readable suggested fixes, and a UTC timestamp.
///
/// Use [`run_gate_checks`] for routine startup logging (returns
/// [`GateStatus`]). Use [`run_gate_checks_detailed`] when you need the full
/// report.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GateReport {
    /// Aggregate pass/fail status over all gate checks.
    pub status: GateStatus,

    /// Safe mode configuration. Present when a Critical gate has failed.
    pub safe_mode: Option<SafeModeConfig>,

    /// Ordered list of human-readable suggested fixes for every failed
    /// check. Empty when all checks pass.
    pub suggested_fixes: Vec<String>,

    /// UTC timestamp capturing when the checks ran.
    pub checked_at: DateTime<Utc>,
}

impl GateReport {
    /// Constructs a `GateReport` from raw check results.
    ///
    /// Automatically determines `all_passed`, `safe_mode`, and
    /// `suggested_fixes`.
    fn new(checks: Vec<GateCheckResult>) -> Self {
        let status = GateStatus::new(checks);

        let safe_mode = if !status.all_passed {
            let reasons: Vec<&str> = status
                .checks
                .iter()
                .filter(|c| !c.passed && c.severity == GateSeverity::Critical)
                .map(|c| c.message.as_str())
                .collect();
            Some(SafeModeConfig::new(reasons.join("; ")))
        } else {
            None
        };

        let suggested_fixes = Self::build_suggestions(&status.checks);

        Self {
            status,
            safe_mode,
            suggested_fixes,
            checked_at: Utc::now(),
        }
    }

    /// Generates a human-readable fix suggestion for each failed check.
    fn build_suggestions(checks: &[GateCheckResult]) -> Vec<String> {
        checks
            .iter()
            .filter(|c| !c.passed)
            .filter_map(|c| match c.code {
                Some(GateErrorCode::Gate001FileHash) => Some(
                    "Reinstall the openhuman-core binary or set \
                          OPENHUMAN_CORE_EXPECTED_HASH to the correct \
                          SHA-256 hash."
                        .into(),
                ),
                Some(GateErrorCode::Gate002NoApiKey) => Some(
                    "Set DEEPSEEK_API_KEY or DOUBAO_API_KEY to a \
                          valid key (minimum 12 characters)."
                        .into(),
                ),
                Some(GateErrorCode::Gate003Connectivity) => Some(
                    "Verify network connectivity to model provider \
                          endpoints. Retried automatically on the first \
                          inference call."
                        .into(),
                ),
                Some(GateErrorCode::Gate004Webhook) => Some(
                    "Check the payment callback URL configuration \
                          and ensure the endpoint is reachable."
                        .into(),
                ),
                Some(GateErrorCode::Gate005DiskQuota) => Some(
                    "Free up disk space. The system requires at \
                          least 100 MiB for the SQLite database and \
                          Obsidian vault."
                        .into(),
                ),
                None => None,
            })
            .collect()
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Builds a [`GateCheckResult`] from its components.
fn make_result(
    name: &str,
    passed: bool,
    message: &str,
    severity: GateSeverity,
    code: Option<GateErrorCode>,
    gate: Option<GateCode>,
) -> GateCheckResult {
    GateCheckResult {
        name: name.to_string(),
        passed,
        message: message.to_string(),
        severity,
        code,
        gate,
    }
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Runs all five GATE checks and returns an aggregate [`GateStatus`].
///
/// This is the primary entry point for startup self-checks. Results are
/// logged at `info` / `warn` levels via the `log` crate.
///
/// `china_models` is an optional reference to the `[china_models]` config
/// section. When provided, GATE-2 also validates API keys stored there.
///
/// # Example
///
/// ```no_run
/// use openhuman_core::doctor::gate::run_gate_checks;
///
/// let status = run_gate_checks(None).await;
/// if !status.all_passed {
///     eprintln!("{}", status.summary());
/// }
/// ```
pub async fn run_gate_checks(
    china_models: Option<&crate::openhuman::config::china_models::ChinaModelsConfig>,
) -> GateStatus {
    let checks = vec![
        check_binary_integrity(),
        check_api_keys(china_models),
        check_model_connectivity(),
        check_payment_webhook(),
        check_disk_quota(),
    ];

    let report = GateReport::new(checks);
    let status = report.status;

    // Log each check result.
    for check in &status.checks {
        if check.passed {
            info!("[闸机] {} 通过: {}", check.name, check.message);
        } else {
            match check.severity {
                GateSeverity::Critical => {
                    warn!(
                        "[闸机] {} 失败: {} ({:?})",
                        check.name, check.message, check.code
                    );
                }
                GateSeverity::Warning => {
                    warn!("[闸机] {} 警告: {}", check.name, check.message);
                }
                GateSeverity::Info => {
                    info!("[闸机] {}: {}", check.name, check.message);
                }
            }
        }
    }

    if let Some(ref sm) = report.safe_mode {
        warn!("[闸机] 进入安全模式: {}", sm.reason);
    }

    status
}

/// Runs all five GATE checks and returns a comprehensive [`GateReport`].
///
/// Unlike [`run_gate_checks`], this function returns safe mode configuration
/// per-gate suggested fixes, and a timestamp alongside the aggregate status.
///
/// Use this when you need to display or react to detailed gate results
/// (e.g. in a diagnostics UI or a startup report).
pub async fn run_gate_checks_detailed(
    china_models: Option<&crate::openhuman::config::china_models::ChinaModelsConfig>,
) -> GateReport {
    let checks = vec![
        check_binary_integrity(),
        check_api_keys(china_models),
        check_model_connectivity(),
        check_payment_webhook(),
        check_disk_quota(),
    ];
    GateReport::new(checks)
}
