// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OpenHuman-ZN Contributors
//
// Additional tests for doctor/gate — SafeMode edges, error codes, serialization.

#[cfg(test)]
mod extra_tests {
    use crate::openhuman::doctor::gate::{GateCheckResult, GateCode, GateSeverity, GateStatus};

    #[test]
    fn empty_checks_passes() {
        let s = GateStatus::new(vec![]);
        assert!(s.all_passed);
        assert_eq!(s.passed_count, 0);
    }

    #[test]
    fn critical_failure_blocks() {
        let checks = vec![GateCheckResult {
            name: "f1".into(),
            passed: false,
            message: "x".into(),
            severity: GateSeverity::Critical,
            code: None,
            gate: None,
        }];
        assert!(!GateStatus::new(checks).all_passed);
    }

    #[test]
    fn warning_failure_does_not_block() {
        let checks = vec![GateCheckResult {
            name: "w1".into(),
            passed: false,
            message: "x".into(),
            severity: GateSeverity::Warning,
            code: None,
            gate: None,
        }];
        assert!(GateStatus::new(checks).all_passed);
    }

    #[test]
    fn summary_shows_counts() {
        let checks = vec![GateCheckResult {
            name: "a".into(),
            passed: true,
            message: "x".into(),
            severity: GateSeverity::Info,
            code: None,
            gate: None,
        }];
        let s = GateStatus::new(checks);
        assert!(s.summary().contains("1/1"));
    }

    #[test]
    fn each_gate_code_has_label() {
        assert!(GateCode::Gate1FileIntegrity.label().contains("GATE-1"));
        assert!(GateCode::Gate2ApiKey.label().contains("GATE-2"));
        assert!(GateCode::Gate3ModelConnectivity.label().contains("GATE-3"));
        assert!(GateCode::Gate4PaymentWebhook.label().contains("GATE-4"));
        assert!(GateCode::Gate5DiskQuota.label().contains("GATE-5"));
    }

    #[test]
    fn gate_check_result_serializes() {
        let r = GateCheckResult {
            name: "t".into(),
            passed: true,
            message: "ok".into(),
            severity: GateSeverity::Info,
            code: None,
            gate: None,
        };
        let json = serde_json::to_string(&r).unwrap();
        assert!(json.contains("ok"));
    }

    #[test]
    fn mixed_severity_correct_count() {
        let checks = vec![
            GateCheckResult {
                name: "a".into(),
                passed: true,
                message: "x".into(),
                severity: GateSeverity::Critical,
                code: None,
                gate: None,
            },
            GateCheckResult {
                name: "b".into(),
                passed: true,
                message: "x".into(),
                severity: GateSeverity::Warning,
                code: None,
                gate: None,
            },
            GateCheckResult {
                name: "c".into(),
                passed: false,
                message: "x".into(),
                severity: GateSeverity::Info,
                code: None,
                gate: None,
            },
            GateCheckResult {
                name: "d".into(),
                passed: true,
                message: "x".into(),
                severity: GateSeverity::Info,
                code: None,
                gate: None,
            },
        ];
        let s = GateStatus::new(checks);
        assert!(s.all_passed);
        assert_eq!(s.passed_count, 3);
        assert_eq!(s.total_count, 4);
    }
}
