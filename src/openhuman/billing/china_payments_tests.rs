// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OpenHuman-ZN Contributors

#[cfg(test)]
mod tests {
    use super::super::china_payments::*;
    use crate::openhuman::config::Config;

    // ── CnPlanTier tests ───────────────────────────────────────────────

    #[test]
    fn trial_is_free() {
        assert_eq!(CnPlanTier::Trial.price_cny(), 0);
    }

    #[test]
    fn personal_plan_price() {
        assert_eq!(CnPlanTier::Personal.price_cny(), 199);
    }

    #[test]
    fn team_plan_price() {
        assert_eq!(CnPlanTier::Team.price_cny(), 499);
    }

    #[test]
    fn enterprise_plan_is_negotiable_price() {
        assert_eq!(CnPlanTier::Enterprise.price_cny(), 0);
    }

    #[test]
    fn all_tiers_have_chinese_names() {
        let tiers = [
            CnPlanTier::Trial,
            CnPlanTier::Personal,
            CnPlanTier::Team,
            CnPlanTier::Enterprise,
        ];
        for tier in &tiers {
            assert!(!tier.display_name().is_empty(), "tier has no display name");
        }
    }

    // ── Payment input validation (no network calls) ────────────────────

    fn cfg() -> Config {
        Config::default()
    }

    #[tokio::test]
    async fn wechat_payment_rejects_empty_plan() {
        let err = create_wechat_payment(&cfg(), "").await.unwrap_err();
        assert!(err.contains("plan is required"), "got: {err}");
    }

    #[tokio::test]
    async fn wechat_payment_rejects_whitespace_plan() {
        let err = create_wechat_payment(&cfg(), "   ").await.unwrap_err();
        assert!(err.contains("plan is required"), "got: {err}");
    }

    #[tokio::test]
    async fn alipay_payment_rejects_empty_plan() {
        let err = create_alipay_payment(&cfg(), "").await.unwrap_err();
        assert!(err.contains("plan is required"), "got: {err}");
    }

    #[tokio::test]
    async fn query_status_rejects_empty_order_id() {
        let err = query_payment_status(&cfg(), "").await.unwrap_err();
        assert!(err.contains("order_id is required"), "got: {err}");
    }

    #[tokio::test]
    async fn query_status_rejects_whitespace_order_id() {
        let err = query_payment_status(&cfg(), "   ").await.unwrap_err();
        assert!(err.contains("order_id is required"), "got: {err}");
    }
}
