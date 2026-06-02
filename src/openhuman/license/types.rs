// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OpenHuman-ZN Contributors
//! License types for OpenHuman-ZN commercial activation.
//!
//! Supports trial, personal, team, and enterprise tiers with
//! expiration tracking, device binding, quota enforcement, and
//! a full state machine including offline fallback (FallbackGrace).

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// License tier determines feature access and usage limits.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LicenseTier {
    /// 7-day free trial — max 20 requests/day, all features.
    Trial,
    /// Personal — unlimited requests, up to 2 devices.
    Personal,
    /// Team — unlimited requests, multi-device, CSV import.
    Team,
    /// Enterprise — custom SLA, SSO, on-premise option.
    Enterprise,
}

impl LicenseTier {
    pub fn daily_quota(&self) -> Option<u64> {
        match self {
            LicenseTier::Trial => Some(20),
            _ => None,
        }
    }

    pub fn max_devices(&self) -> u32 {
        match self {
            LicenseTier::Trial => 1,
            LicenseTier::Personal => 2,
            LicenseTier::Team => 10,
            LicenseTier::Enterprise => u32::MAX,
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            LicenseTier::Trial => "\u{514d}\u{8d39}\u{8bd5}\u{7528}",
            LicenseTier::Personal => "\u{4e2a}\u{4eba}\u{7248}",
            LicenseTier::Team => "\u{56e2}\u{961f}\u{7248}",
            LicenseTier::Enterprise => "\u{4f01}\u{4e1a}\u{7248}",
        }
    }

    pub fn monthly_price_cny(&self) -> u32 {
        match self {
            LicenseTier::Trial => 0,
            LicenseTier::Personal => 199,
            LicenseTier::Team => 499,
            LicenseTier::Enterprise => 0,
        }
    }
}

/// The activation state of a license on this device.
///
/// # State machine
///
/// ```text
/// Unlicensed ──► Trial ──► Active ──► Expired
///      │                    │            │
///      └────────────────────┼────────────┘
///                           │
///                      FallbackGrace
///                           │
///                      Revoked
/// ```
///
/// `FallbackGrace` is entered automatically when the license server has been
/// unreachable for more than 72 consecutive hours. Local-only features remain
/// accessible; cloud features are suspended until the server becomes reachable
/// again.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LicenseState {
    /// No license present — feature-gated until activation.
    Unlicensed,
    /// 7-day trial period, counted from `started_at`.
    Trial {
        started_at: DateTime<Utc>,
        days_remaining: u32,
    },
    /// Paid license, fully active with the given tier.
    Active {
        tier: LicenseTier,
        activated_at: DateTime<Utc>,
    },
    /// License has expired; previously had `previous_tier`.
    Expired {
        previous_tier: LicenseTier,
        expired_at: DateTime<Utc>,
    },
    /// License revoked by the server (e.g. payment failure, abuse).
    Revoked {
        reason: String,
        revoked_at: DateTime<Utc>,
    },
    /// Grace period entered after the license server has been unreachable
    /// for more than 72 consecutive hours. Local-only features remain
    /// available; cloud features are suspended.
    FallbackGrace {
        original_tier: LicenseTier,
        entered_at: DateTime<Utc>,
        reason: String,
    },
}

impl LicenseState {
    /// Returns `true` if the state allows API access (including fallback grace).
    pub fn is_active(&self) -> bool {
        matches!(
            self,
            LicenseState::Trial { .. }
                | LicenseState::Active { .. }
                | LicenseState::FallbackGrace { .. }
        )
    }

    /// Returns the current (or most recent) license tier, if any.
    pub fn tier(&self) -> Option<LicenseTier> {
        match self {
            LicenseState::Active { tier, .. } => Some(tier.clone()),
            LicenseState::Trial { .. } => Some(LicenseTier::Trial),
            LicenseState::Expired { previous_tier, .. } => Some(previous_tier.clone()),
            LicenseState::FallbackGrace { original_tier, .. } => Some(original_tier.clone()),
            _ => None,
        }
    }
}

/// A license key (XXXX-XXXX-XXXX-XXXX format).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LicenseKey(String);

impl LicenseKey {
    pub fn new(key: impl Into<String>) -> Self {
        Self(key.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Validate the canonical XXXX-XXXX-XXXX-XXXX format.
    pub fn is_valid_format(key: &str) -> bool {
        let segments: Vec<&str> = key.split('-').collect();
        segments.len() == 4
            && segments
                .iter()
                .all(|s| s.len() == 4 && s.chars().all(|c| c.is_alphanumeric()))
    }
}

/// Device fingerprint for binding licenses to hardware.
///
/// Combines CPU identifier, motherboard serial/UUID, and hostname
/// into a SHA-256 hash that uniquely identifies this machine.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceFingerprint {
    pub hostname: String,
    pub hardware_hash: String,
}

/// Request to activate a license.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LicenseActivationRequest {
    pub license_key: LicenseKey,
    pub device: DeviceFingerprint,
    pub user_email: String,
}

/// Response from license activation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LicenseActivationResponse {
    pub state: LicenseState,
    pub message: String,
    pub server_signature: Option<String>,
}

/// Daily usage counter (reset at midnight UTC+8).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DailyUsage {
    pub date: String,
    pub request_count: u64,
    pub token_count: u64,
}
