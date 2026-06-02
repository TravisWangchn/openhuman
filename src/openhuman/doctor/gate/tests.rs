// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OpenHuman-ZN Contributors

use super::*;
use std::env;

// -- Scoped environment guard ------------------------------------------------

/// Temporarily removes or sets an environment variable for the duration
/// of a test. The previous value is restored on drop.
struct ScopedEnv {
    key: String,
    previous: Option<String>,
}

impl ScopedEnv {
    fn set(key: &str, value: &str) -> Self {
        let previous = env::var(key).ok();
        env::set_var(key, value);
        Self {
            key: key.to_string(),
            previous,
        }
    }

    fn clear(key: &str) -> Self {
        let previous = env::var(key).ok();
        env::remove_var(key);
        Self {
            key: key.to_string(),
            previous,
        }
    }
}

impl Drop for ScopedEnv {
    fn drop(&mut self) {
        match &self.previous {
            Some(val) => env::set_var(&self.key, val),
            None => env::remove_var(&self.key),
        }
    }
}

// -- GateStatus ----------------------------------------------------------------

#[test]
fn test_gate_status_all_pass() {
    let checks = vec![
        GateCheckResult {
            name: "critical-ok".into(),
            passed: true,
            message: "ok".into(),
            severity: GateSeverity::Critical,
            code: None,
            gate: Some(GateCode::Gate1FileIntegrity),
        },
        GateCheckResult {
            name: "warning-ok".into(),
            passed: true,
            message: "ok".into(),
            severity: GateSeverity::Warning,
            code: None,
            gate: Some(GateCode::Gate3ModelConnectivity),
        },
    ];
    let status = GateStatus::new(checks);
    assert!(status.all_passed);
    assert_eq!(status.passed_count, 2);
    assert_eq!(status.total_count, 2);
}

#[test]
fn test_gate_status_critical_failure() {
    let checks = vec![
        GateCheckResult {
            name: "critical-fail".into(),
            passed: false,
            message: "no key".into(),
            severity: GateSeverity::Critical,
            code: Some(GateErrorCode::Gate002NoApiKey),
            gate: Some(GateCode::Gate2ApiKey),
        },
        GateCheckResult {
            name: "warning-fail".into(),
            passed: false,
            message: "low space".into(),
            severity: GateSeverity::Warning,
            code: Some(GateErrorCode::Gate005DiskQuota),
            gate: Some(GateCode::Gate5DiskQuota),
        },
    ];
    let status = GateStatus::new(checks);
    assert!(!status.all_passed, "critical failure must flip all_passed");
    assert_eq!(status.passed_count, 0);
}

#[test]
fn test_gate_status_warning_only_does_not_fail() {
    let checks = vec![GateCheckResult {
        name: "warn-only".into(),
        passed: false,
        message: "disk low".into(),
        severity: GateSeverity::Warning,
        code: Some(GateErrorCode::Gate005DiskQuota),
        gate: Some(GateCode::Gate5DiskQuota),
    }];
    let status = GateStatus::new(checks);
    assert!(status.all_passed, "warning-only must not flip all_passed");
}

#[test]
fn test_summary_format() {
    let checks = vec![GateCheckResult {
        name: "ok".into(),
        passed: true,
        message: "ok".into(),
        severity: GateSeverity::Critical,
        code: None,
        gate: None,
    }];
    let status = GateStatus::new(checks);
    assert!(status.summary().contains("\u{901a}\u{8fc7}"));
}

// -- GateReport ----------------------------------------------------------------

#[test]
fn test_report_safe_mode_on_critical_fail() {
    let checks = vec![GateCheckResult {
        name: "critical".into(),
        passed: false,
        message: "key missing".into(),
        severity: GateSeverity::Critical,
        code: Some(GateErrorCode::Gate002NoApiKey),
        gate: Some(GateCode::Gate2ApiKey),
    }];
    let report = GateReport::new(checks);
    assert!(report.safe_mode.is_some());
    let sm = report.safe_mode.unwrap();
    assert!(sm.safe_mode);
    assert!(sm
        .disabled_features
        .contains(&FEATURE_MODEL_INFERENCE.to_string()));
}

#[test]
fn test_report_no_safe_mode_when_all_pass() {
    let checks = vec![GateCheckResult {
        name: "pass".into(),
        passed: true,
        message: "ok".into(),
        severity: GateSeverity::Critical,
        code: None,
        gate: Some(GateCode::Gate1FileIntegrity),
    }];
    let report = GateReport::new(checks);
    assert!(report.safe_mode.is_none());
}

#[test]
fn test_report_suggested_fixes() {
    let checks = vec![
        GateCheckResult {
            name: "api".into(),
            passed: false,
            message: "no key".into(),
            severity: GateSeverity::Critical,
            code: Some(GateErrorCode::Gate002NoApiKey),
            gate: Some(GateCode::Gate2ApiKey),
        },
        GateCheckResult {
            name: "disk".into(),
            passed: false,
            message: "low space".into(),
            severity: GateSeverity::Warning,
            code: Some(GateErrorCode::Gate005DiskQuota),
            gate: Some(GateCode::Gate5DiskQuota),
        },
    ];
    let report = GateReport::new(checks);
    assert_eq!(report.suggested_fixes.len(), 2);
    assert!(report.suggested_fixes[0].contains("key"));
    assert!(report.suggested_fixes[1].contains("disk"));
}

#[test]
fn test_report_timestamp() {
    let report = GateReport::new(vec![]);
    let elapsed = Utc::now() - report.checked_at;
    assert!(elapsed.num_seconds() < 5, "timestamp must be recent");
}

// -- GateCode / GateErrorCode --------------------------------------------------

#[test]
fn test_gate_code_labels() {
    assert_eq!(GateCode::Gate1FileIntegrity.label(), "GATE-1");
    assert_eq!(GateCode::Gate2ApiKey.label(), "GATE-2");
    assert_eq!(GateCode::Gate3ModelConnectivity.label(), "GATE-3");
    assert_eq!(GateCode::Gate4PaymentWebhook.label(), "GATE-4");
    assert_eq!(GateCode::Gate5DiskQuota.label(), "GATE-5");
}

#[test]
fn test_gate_code_error_codes_match_labels() {
    assert_eq!(
        GateCode::Gate1FileIntegrity.error_code().to_string(),
        "GATE_001_FILE_HASH"
    );
    assert_eq!(
        GateCode::Gate5DiskQuota.error_code().to_string(),
        "GATE_005_DISK_QUOTA"
    );
}

// -- Binary integrity ----------------------------------------------------------

#[test]
fn test_check_binary_integrity_no_env() {
    let _g = ScopedEnv::clear(ENV_EXPECTED_HASH);
    let result = check_binary_integrity();
    assert!(result.passed);
    assert_eq!(result.severity, GateSeverity::Info);
    assert!(result.message.contains("\u{672a}\u{914d}\u{7f6e}"));
}

#[test]
fn test_check_binary_integrity_env_set_binary_readable() {
    let _g = ScopedEnv::set(
        ENV_EXPECTED_HASH,
        "0000000000000000000000000000000000000000000000000000000000000000",
    );
    let result = check_binary_integrity();
    // The binary is readable; the hash simply won't match all-zeros.
    assert!(!result.passed);
    assert_eq!(result.severity, GateSeverity::Critical);
    assert_eq!(result.code, Some(GateErrorCode::Gate001FileHash));
}

// -- API keys ------------------------------------------------------------------

#[test]
fn test_check_api_keys_missing() {
    let _ds = ScopedEnv::clear(ENV_DEEPSEEK_KEY);
    let _db = ScopedEnv::clear(ENV_DOUBAO_KEY);
    let result = check_api_keys(None);
    assert!(!result.passed);
    assert_eq!(result.code, Some(GateErrorCode::Gate002NoApiKey));
    assert_eq!(result.severity, GateSeverity::Critical);
}

#[test]
fn test_check_api_keys_short_key_rejected() {
    let _ds = ScopedEnv::set(ENV_DEEPSEEK_KEY, "short");
    let _db = ScopedEnv::clear(ENV_DOUBAO_KEY);
    assert!(!check_api_keys(None).passed);
}

#[test]
fn test_check_api_keys_valid_deepseek() {
    let _ds = ScopedEnv::set(ENV_DEEPSEEK_KEY, "sk-valid-key-123456");
    let _db = ScopedEnv::clear(ENV_DOUBAO_KEY);
    assert!(check_api_keys(None).passed);
}

#[test]
fn test_check_api_keys_valid_doubao() {
    let _ds = ScopedEnv::clear(ENV_DEEPSEEK_KEY);
    let _db = ScopedEnv::set(ENV_DOUBAO_KEY, "db-valid-key-123456");
    assert!(check_api_keys(None).passed);
}

#[test]
fn test_check_api_keys_both_valid() {
    let _ds = ScopedEnv::set(ENV_DEEPSEEK_KEY, "sk-valid-key-123456");
    let _db = ScopedEnv::set(ENV_DOUBAO_KEY, "db-valid-key-123456");
    let result = check_api_keys(None);
    assert!(result.passed);
    assert!(result.message.contains("+"));
}

// -- Connectivity / webhook (deferred) -----------------------------------------

#[test]
fn test_check_model_connectivity_deferred() {
    let result = check_model_connectivity();
    assert!(result.passed);
    assert_eq!(result.severity, GateSeverity::Warning);
    assert!(result
        .message
        .contains("\u{9996}\u{6b21}\u{63a8}\u{7406}\u{8c03}\u{7528}"));
}

#[test]
fn test_check_payment_webhook_deferred() {
    let result = check_payment_webhook();
    assert!(result.passed);
    assert_eq!(result.severity, GateSeverity::Warning);
    assert!(result
        .message
        .contains("\u{9996}\u{6b21}\u{652f}\u{4ed8}\u{4e8b}\u{4ef6}"));
}

// -- Serialization round-trip --------------------------------------------------

#[test]
fn test_gate_status_round_trip() {
    let checks = vec![GateCheckResult {
        name: "test".into(),
        passed: true,
        message: "ok".into(),
        severity: GateSeverity::Critical,
        code: None,
        gate: Some(GateCode::Gate1FileIntegrity),
    }];
    let original = GateStatus::new(checks);
    let json = serde_json::to_string(&original).unwrap();
    let restored: GateStatus = serde_json::from_str(&json).unwrap();
    assert_eq!(restored.all_passed, original.all_passed);
    assert_eq!(restored.checks.len(), 1);
}

// -- SafeModeConfig ------------------------------------------------------------

#[test]
fn test_safe_mode_config_active() {
    let config = SafeModeConfig::new("test reason");
    assert!(config.safe_mode);
    assert!(config
        .disabled_features
        .contains(&FEATURE_MODEL_INFERENCE.to_string()));
}

#[test]
fn test_safe_mode_config_inactive() {
    let config = SafeModeConfig::inactive();
    assert!(!config.safe_mode);
    assert!(config.disabled_features.is_empty());
}
