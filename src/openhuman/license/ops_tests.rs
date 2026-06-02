// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OpenHuman-ZN Contributors

#[cfg(test)]
mod tests {
    use super::super::types::*;

    // ── LicenseKey validation ──────────────────────────────────────────

    #[test]
    fn valid_license_key_format() {
        assert!(LicenseKey::is_valid_format("ABCD-EFGH-IJKL-MNOP"));
        assert!(LicenseKey::is_valid_format("1234-5678-9ABC-DEFG"));
    }

    #[test]
    fn invalid_license_key_too_short() {
        assert!(!LicenseKey::is_valid_format("short"));
    }

    #[test]
    fn invalid_license_key_wrong_segments() {
        assert!(!LicenseKey::is_valid_format("ABCD-EFGH-IJKL"));
    }

    #[test]
    fn invalid_license_key_special_chars() {
        assert!(!LicenseKey::is_valid_format("ABCD-EFGH-IJKL-MNO!"));
    }

    #[test]
    fn invalid_license_key_empty() {
        assert!(!LicenseKey::is_valid_format(""));
    }

    // ── LicenseTier ────────────────────────────────────────────────────

    #[test]
    fn trial_tier_quota_and_price() {
        assert_eq!(LicenseTier::Trial.daily_quota(), Some(20));
        assert_eq!(LicenseTier::Trial.monthly_price_cny(), 0);
        assert_eq!(LicenseTier::Trial.max_devices(), 1);
    }

    #[test]
    fn personal_tier_unlimited_quota() {
        assert_eq!(LicenseTier::Personal.daily_quota(), None);
        assert_eq!(LicenseTier::Personal.monthly_price_cny(), 199);
        assert_eq!(LicenseTier::Personal.max_devices(), 2);
    }

    #[test]
    fn team_tier_pricing() {
        assert_eq!(LicenseTier::Team.monthly_price_cny(), 499);
        assert_eq!(LicenseTier::Team.max_devices(), 10);
    }

    #[test]
    fn enterprise_unlimited_devices() {
        assert_eq!(LicenseTier::Enterprise.max_devices(), u32::MAX);
        assert_eq!(LicenseTier::Enterprise.monthly_price_cny(), 0);
    }

    #[test]
    fn all_tiers_have_chinese_display_names() {
        assert!(!LicenseTier::Trial.display_name().is_empty());
        assert!(!LicenseTier::Personal.display_name().is_empty());
        assert!(!LicenseTier::Team.display_name().is_empty());
        assert!(!LicenseTier::Enterprise.display_name().is_empty());
    }

    // ── LicenseState ───────────────────────────────────────────────────

    #[test]
    fn unlicensed_state_not_active_no_tier() {
        assert!(!LicenseState::Unlicensed.is_active());
        assert_eq!(LicenseState::Unlicensed.tier(), None);
    }

    #[test]
    fn active_state_is_active_with_tier() {
        let state = LicenseState::Active {
            tier: LicenseTier::Personal,
            activated_at: chrono::Utc::now(),
        };
        assert!(state.is_active());
        assert_eq!(state.tier(), Some(LicenseTier::Personal));
    }

    #[test]
    fn trial_state_is_active() {
        let state = LicenseState::Trial {
            started_at: chrono::Utc::now(),
            days_remaining: 7,
        };
        assert!(state.is_active());
        assert_eq!(state.tier(), Some(LicenseTier::Trial));
    }

    #[test]
    fn expired_not_active() {
        let state = LicenseState::Expired {
            previous_tier: LicenseTier::Personal,
            expired_at: chrono::Utc::now(),
        };
        assert!(!state.is_active());
    }

    #[test]
    fn revoked_not_active() {
        let state = LicenseState::Revoked {
            reason: "test".into(),
            revoked_at: chrono::Utc::now(),
        };
        assert!(!state.is_active());
    }

    // ── JSON serialization ─────────────────────────────────────────────

    #[test]
    fn active_state_serializes() {
        let state = LicenseState::Active {
            tier: LicenseTier::Personal,
            activated_at: chrono::Utc::now(),
        };
        let json = serde_json::to_string(&state).unwrap();
        assert!(json.contains("active"));
    }

    #[test]
    fn daily_usage_defaults_zero() {
        let usage = DailyUsage::default();
        assert_eq!(usage.request_count, 0);
        assert_eq!(usage.token_count, 0);
    }
}
