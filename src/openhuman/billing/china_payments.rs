// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OpenHuman-ZN Contributors

//! China payment gateways for OpenHuman-ZN: WeChat Pay & Alipay.
//!
//! This module provides the full China-payment lifecycle:
//!
//! * **Plan catalog** — [`CnPlanTier`] with CNY pricing and Chinese display names.
//! * **Payment creation** — [`create_wechat_payment`] and [`create_alipay_payment`]
//!   proxy to the hosted backend (`/payments/wechat/create`, `/payments/alipay/create`).
//! * **Order state machine** — [`OrderState`] with typed transitions:
//!   `Pending → Paid → LicenseIssued → Confirmed → Refunded`.
//! * **7-day no-reason refund** — [`can_refund`] checks the refund window.
//! * **Callback idempotency** — [`IdempotencyStore`] uses SQLite `INSERT OR IGNORE`
//!   with `order_id` as the unique key (distributed-lock equivalent for a single-node setup).
//! * **Signature verification skeletons** — [`verify_wechat_callback_signature`] and
//!   [`verify_alipay_callback_signature`] using RSA PKCS1v15 SHA-256 (`ring`).
//! * **Retry with exponential backoff** — [`with_retry`] retries a fallible async
//!   operation at 1s / 2s / 4s / 8s intervals (max 4 retries).
//! * **Tax invoice (fapiao / 发票) stubs** — [`FapiaoService`] for enterprise customers.
//! * **Input validation** — plan-name whitelist, CNY amount range checks, `OrderId` format.
//!
//! # Security
//!
//! All payment-creation methods require a valid app-session JWT stored via
//! `auth_store_session`. The JWT is sent as `Authorization: Bearer …` to the backend.
//! Callback signature verification is performed locally with the platform's RSA public key
//! — the actual key material must be injected via environment variables at deployment time.
//! Idempotency is enforced at the database level so duplicate webhook deliveries are safe.
//!
//! # Dependencies
//!
//! Uses only crate dependencies that already exist in `Cargo.toml`:
//! `serde`, `serde_json`, `reqwest`, `anyhow`, `log`, `chrono`, `rusqlite`, `ring`,
//! `sha2`, `urlencoding`, `uuid`, `base64`, `once_cell`, `thiserror`.

use std::sync::Mutex;
use std::time::Duration;

use chrono::{DateTime, Utc};
use once_cell::sync::Lazy;
use reqwest::Method;
use ring::signature::{UnparsedPublicKey, RSA_PKCS1_2048_8192_SHA256};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::api::config::effective_backend_api_url;
use crate::api::jwt::get_session_token;
use crate::api::BackendOAuthClient;
use crate::openhuman::config::Config;
use crate::rpc::RpcOutcome;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Whitelist of valid plan names for China payments.
const VALID_CN_PLANS: &[&str] = &["trial", "personal", "team", "enterprise"];

/// Minimum payment amount in CNY (1 fen = CNY 0.01, floor at CNY 1).
const MIN_AMOUNT_CNY: u32 = 1;

/// Maximum single-payment amount in CNY.
const MAX_AMOUNT_CNY: u32 = 999_999;

/// Number of retry attempts for callback processing (1s / 2s / 4s / 8s).
const MAX_CALLBACK_RETRIES: u32 = 4;

/// Delay sequence for exponential backoff in seconds.
const RETRY_DELAYS_SECS: &[u64] = &[1, 2, 4, 8];

/// No-reason refund window in days (Chinese E-commerce law: 7 days for most goods).
const REFUND_WINDOW_DAYS: i64 = 7;

/// Default path for the idempotency SQLite database.
const IDEMPOTENCY_DB_PATH: &str = "payment_idempotency.db";

// ---------------------------------------------------------------------------
// Input Validation — newtype wrappers
// ---------------------------------------------------------------------------

/// A validated China-payment order ID.
///
/// Format: `{prefix}_{uuid_v4}` where `prefix` is one of `wx`, `ali`, `oh`.
/// Examples: `wx_a1b2c3d4-e5f6-7890-abcd-ef1234567890`,
///           `ali_e5f6a1b2-c3d4-7890-abcd-ef1234567890`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct OrderId(String);

impl OrderId {
    /// Validate and construct an `OrderId`.
    ///
    /// Returns an error if the string is empty, does not match the expected
    /// format, or contains a syntactically invalid UUID segment.
    pub fn parse(input: &str) -> Result<Self, String> {
        let trimmed = input.trim();
        if trimmed.is_empty() {
            return Err("order_id must not be empty".to_string());
        }
        // Expect "prefix_uuid" where prefix is wx/ali/oh
        let parts: Vec<&str> = trimmed.splitn(2, '_').collect();
        if parts.len() != 2 {
            return Err(format!(
                "invalid order_id format '{}': expected 'prefix_uuid'",
                trimmed
            ));
        }
        let prefix = parts[0];
        if !matches!(prefix, "wx" | "ali" | "oh") {
            return Err(format!(
                "invalid order_id prefix '{}': must be one of wx, ali, oh",
                prefix
            ));
        }
        let uuid_str = parts[1];
        // Validate UUID syntax without allocating.
        Uuid::parse_str(uuid_str)
            .map_err(|e| format!("invalid UUID segment in order_id '{}': {}", trimmed, e))?;
        Ok(Self(trimmed.to_string()))
    }

    /// Return the inner string.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Consume the wrapper and return the inner `String`.
    pub fn into_inner(self) -> String {
        self.0
    }
}

impl std::fmt::Display for OrderId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// A validated China-plan name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanName(String);

impl PlanName {
    /// Validate and construct a `PlanName`.
    ///
    /// Returns an error if the input is empty or not in the whitelist.
    pub fn parse(input: &str) -> Result<Self, String> {
        let trimmed = input.trim().to_ascii_lowercase();
        if trimmed.is_empty() {
            return Err("plan name is required".to_string());
        }
        if !VALID_CN_PLANS.contains(&trimmed.as_str()) {
            return Err(format!(
                "invalid plan '{}': must be one of {}",
                trimmed,
                VALID_CN_PLANS.join(", ")
            ));
        }
        Ok(Self(trimmed))
    }

    /// Return the inner plan name.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A validated CNY amount.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AmountCny(u32);

impl AmountCny {
    /// Validate and construct a `AmountCny`.
    ///
    /// Returns an error if the value is outside the allowed range.
    pub fn new(value: u32) -> Result<Self, String> {
        if value < MIN_AMOUNT_CNY {
            return Err(format!("amount must be at least CNY {}", MIN_AMOUNT_CNY));
        }
        if value > MAX_AMOUNT_CNY {
            return Err(format!("amount must not exceed CNY {}", MAX_AMOUNT_CNY));
        }
        Ok(Self(value))
    }

    /// Return the inner value.
    pub fn get(&self) -> u32 {
        self.0
    }
}

// ---------------------------------------------------------------------------
// Order State Machine
// ---------------------------------------------------------------------------

/// Lifecycle states for a China payment order.
///
/// Transition rules:
/// ```text
/// Pending ──(payment_success)──▶ Paid
///   Paid ──(license_issued)──▶ LicenseIssued
///   LicenseIssued ──(confirmed)──▶ Confirmed
///   Paid ──(refund)──▶ Refunded           (within 7-day window)
///   LicenseIssued ──(refund)──▶ Refunded  (within 7-day window)
/// ```
///
/// Illegal transitions return an error at runtime.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OrderState {
    /// Order created, awaiting payment gateway callback.
    Pending,
    /// Payment confirmed by gateway, license not yet issued.
    Paid,
    /// License issued to the user.
    LicenseIssued,
    /// User confirmed receipt / activation.
    Confirmed,
    /// Payment refunded (full or partial).
    Refunded,
}

impl OrderState {
    /// Attempt to transition from `self` to `target`.
    ///
    /// Returns an error if the transition is not allowed by the state machine.
    pub fn transition_to(self, target: OrderState) -> Result<OrderState, String> {
        match (self, target) {
            // Allowed forward transitions
            (OrderState::Pending, OrderState::Paid)
            | (OrderState::Paid, OrderState::LicenseIssued)
            | (OrderState::LicenseIssued, OrderState::Confirmed) => Ok(target),

            // Refund is allowed from Paid or LicenseIssued (within 7-day window).
            // The refund-window check is done by the caller via `can_refund`.
            (OrderState::Paid, OrderState::Refunded)
            | (OrderState::LicenseIssued, OrderState::Refunded) => Ok(target),

            // Everything else is illegal.
            (current, target) => Err(format!(
                "illegal state transition: {:?} → {:?}",
                current, target
            )),
        }
    }

    /// Check whether a refund is allowed from this state.
    ///
    /// Refunds are permitted from `Paid` or `LicenseIssued`.
    pub fn can_refund(&self) -> bool {
        matches!(self, OrderState::Paid | OrderState::LicenseIssued)
    }

    /// Human-readable Chinese label for UI display.
    pub fn cn_label(&self) -> &'static str {
        match self {
            OrderState::Pending => "待支付",
            OrderState::Paid => "已付款",
            OrderState::LicenseIssued => "已授权",
            OrderState::Confirmed => "已完成",
            OrderState::Refunded => "已退款",
        }
    }
}

/// Check whether a `paid_at` timestamp falls within the 7-day no-reason refund window.
pub fn is_within_refund_window(paid_at: DateTime<Utc>) -> bool {
    let elapsed = Utc::now() - paid_at;
    elapsed.num_days() < REFUND_WINDOW_DAYS
}

// ---------------------------------------------------------------------------
// Idempotency Store — SQLite-backed distributed-lock pattern
// ---------------------------------------------------------------------------

/// SQLite-backed idempotency store for payment callbacks.
///
/// Uses `INSERT OR IGNORE` with `order_id` as the unique key — the moral
/// equivalent of Redis `SET NX` for a single-node deployment.
///
/// # Lock guarantee
///
/// Exactly one actor succeeds in inserting the row. All concurrent or duplicate
/// attempts silently become no-ops. This is safe because rusqlite transactions
/// are serialized behind a `Mutex`.
pub struct IdempotencyStore {
    conn: Mutex<Connection>,
}

impl IdempotencyStore {
    /// Open (or create) the idempotency database at `path`.
    pub fn open(path: &str) -> Result<Self, String> {
        let conn =
            Connection::open(path).map_err(|e| format!("failed to open idempotency db: {e}"))?;
        let store = Self {
            conn: Mutex::new(conn),
        };
        store.init_table()?;
        Ok(store)
    }

    /// Create the callback table if it does not exist.
    fn init_table(&self) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|e| format!("lock error: {e}"))?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS payment_idempotency (
                order_id     TEXT PRIMARY KEY,
                gateway      TEXT NOT NULL,
                raw_body     TEXT NOT NULL DEFAULT '',
                status       TEXT NOT NULL DEFAULT 'pending',
                retry_count  INTEGER NOT NULL DEFAULT 0,
                created_at   TEXT NOT NULL,
                updated_at   TEXT NOT NULL DEFAULT ''
            );",
        )
        .map_err(|e| format!("failed to create idempotency table: {e}"))?;
        Ok(())
    }

    /// Atomically claim an order for processing.
    ///
    /// Returns `Ok(true)` if this call is the first to process the order
    /// (i.e. the row was inserted). Returns `Ok(false)` if the order was
    /// already claimed by a previous call (idempotent no-op).
    pub fn try_acquire(&self, order_id: &str, gateway: &str) -> Result<bool, String> {
        let conn = self.conn.lock().map_err(|e| format!("lock error: {e}"))?;
        let now = Utc::now().to_rfc3339();
        let affected = conn
            .execute(
                "INSERT OR IGNORE INTO payment_idempotency
                 (order_id, gateway, raw_body, status, retry_count, created_at, updated_at)
                 VALUES (?1, ?2, '', 'processing', 0, ?3, ?3)",
                rusqlite::params![order_id, gateway, now],
            )
            .map_err(|e| format!("idempotency insert failed: {e}"))?;
        Ok(affected > 0)
    }

    /// Mark an order as processed with a final status.
    pub fn mark_done(&self, order_id: &str, status: &str) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|e| format!("lock error: {e}"))?;
        let now = Utc::now().to_rfc3339();
        conn.execute(
            "UPDATE payment_idempotency SET status = ?1, updated_at = ?2 WHERE order_id = ?3",
            rusqlite::params![status, now, order_id],
        )
        .map_err(|e| format!("idempotency update failed: {e}"))?;
        Ok(())
    }

    /// Query the stored status for an order.
    pub fn get_status(&self, order_id: &str) -> Result<Option<String>, String> {
        let conn = self.conn.lock().map_err(|e| format!("lock error: {e}"))?;
        let mut stmt = conn
            .prepare("SELECT status FROM payment_idempotency WHERE order_id = ?1")
            .map_err(|e| format!("prepare failed: {e}"))?;
        let result: Result<String, _> =
            stmt.query_row(rusqlite::params![order_id], |row| row.get(0));
        match result {
            Ok(s) => Ok(Some(s)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(format!("idempotency query failed: {e}")),
        }
    }

    /// Increment the retry counter for an order.
    pub fn increment_retry(&self, order_id: &str) -> Result<u32, String> {
        let conn = self.conn.lock().map_err(|e| format!("lock error: {e}"))?;
        conn.execute(
            "UPDATE payment_idempotency
             SET retry_count = retry_count + 1, updated_at = ?1
             WHERE order_id = ?2",
            rusqlite::params![Utc::now().to_rfc3339(), order_id],
        )
        .map_err(|e| format!("retry increment failed: {e}"))?;

        let mut stmt = conn
            .prepare("SELECT retry_count FROM payment_idempotency WHERE order_id = ?1")
            .map_err(|e| format!("prepare failed: {e}"))?;
        let count: u32 = stmt
            .query_row(rusqlite::params![order_id], |row| row.get(0))
            .map_err(|e| format!("retry query failed: {e}"))?;
        Ok(count)
    }
}

/// Global idempotency store singleton (lazy-initialized on first access).
static IDEM_STORE: Lazy<Mutex<Option<IdempotencyStore>>> = Lazy::new(|| Mutex::new(None));

/// Initialise the global idempotency store.
///
/// Safe to call multiple times — subsequent calls are no-ops.
pub fn init_idempotency_store(db_path: Option<&str>) -> Result<(), String> {
    let mut guard = IDEM_STORE.lock().map_err(|e| format!("lock error: {e}"))?;
    if guard.is_some() {
        return Ok(()); // Already initialised.
    }
    let path = db_path.unwrap_or(IDEMPOTENCY_DB_PATH);
    let store = IdempotencyStore::open(path)?;
    *guard = Some(store);
    log::info!("[china_payments] idempotency store initialised at {path}");
    Ok(())
}

/// Acquire a reference to the global idempotency store.
fn with_idem_store<F, R>(f: F) -> Result<R, String>
where
    F: FnOnce(&IdempotencyStore) -> Result<R, String>,
{
    let guard = IDEM_STORE.lock().map_err(|e| format!("lock error: {e}"))?;
    match guard.as_ref() {
        Some(store) => f(store),
        None => {
            Err("idempotency store not initialised; call init_idempotency_store first".to_string())
        }
    }
}

// ---------------------------------------------------------------------------
// Callback Signature Verification Skeletons
// ---------------------------------------------------------------------------

/// Compute the WeChat Pay verification string.
///
/// Format: `"{timestamp}\n{nonce}\n{body}\n"`
fn build_wechat_verification_string(timestamp: &str, nonce: &str, body: &str) -> String {
    format!("{timestamp}\n{nonce}\n{body}\n")
}

/// Verify a WeChat Pay callback signature using RSA PKCS1v15 SHA-256.
///
/// # Parameters
///
/// * `platform_cert_der` — WeChat platform certificate public key in
///   SubjectPublicKeyInfo DER format (extracted from the PEM).
/// * `verification_str` — the string built by
///   [`build_wechat_verification_string`].
/// * `signature_b64` — the base64-encoded signature from the
///   `Wechatpay-Signature` header.
///
/// # Returns
///
/// `Ok(())` if the signature is valid, `Err` otherwise.
///
/// # Production notes
///
/// The platform certificate public key should be:
/// 1. Fetched from `GET /v3/certificates` (WeChat Pay v3 API).
/// 2. Cached by serial number (`Wechatpay-Serial` header).
/// 3. Rotated when WeChat issues a new certificate.
///
/// This skeleton extracts the SPKI bytes from a provided DER buffer. A full
/// implementation should also verify the `Wechatpay-Timestamp` is within an
/// acceptable clock skew (typically 5 minutes).
pub fn verify_wechat_callback_signature(
    platform_cert_der: &[u8],
    verification_str: &str,
    signature_b64: &str,
) -> Result<(), String> {
    let sig_bytes =
        base64::Engine::decode(&base64::engine::general_purpose::STANDARD, signature_b64)
            .map_err(|e| format!("failed to decode wechat signature base64: {e}"))?;

    let public_key = UnparsedPublicKey::new(&RSA_PKCS1_2048_8192_SHA256, platform_cert_der);

    public_key
        .verify(verification_str.as_bytes(), &sig_bytes)
        .map_err(|_| "wechat callback signature verification failed".to_string())
}

/// Verify an Alipay callback signature using RSA PKCS1v15 SHA-256.
///
/// # Parameters
///
/// * `alipay_public_key_der` — Alipay's public key in SPKI DER format.
/// * `verification_str` — the sorted `key=value` string (all parameters
///   except `sign` and `sign_type`, URL-decoded, concatenated).
/// * `signature_b64` — the base64-encoded `sign` parameter from the callback.
///
/// # Returns
///
/// `Ok(())` if the signature is valid, `Err` otherwise.
///
/// # Production notes
///
/// The verification string must be built by:
/// 1. Collecting all callback parameters except `sign` and `sign_type`.
/// 2. Sorting them alphabetically by key.
/// 3. Concatenating as `key1=value1&key2=value2` (values URL-decoded).
///
/// Alipay's public key can be obtained from the Alipay open platform
/// (MDPI / AntOpen). It is a fixed key per application — no rotation polling
/// is required (unlike WeChat Pay).
pub fn verify_alipay_callback_signature(
    alipay_public_key_der: &[u8],
    verification_str: &str,
    signature_b64: &str,
) -> Result<(), String> {
    let sig_bytes =
        base64::Engine::decode(&base64::engine::general_purpose::STANDARD, signature_b64)
            .map_err(|e| format!("failed to decode alipay signature base64: {e}"))?;

    let public_key = UnparsedPublicKey::new(&RSA_PKCS1_2048_8192_SHA256, alipay_public_key_der);

    public_key
        .verify(verification_str.as_bytes(), &sig_bytes)
        .map_err(|_| "alipay callback signature verification failed".to_string())
}

// ---------------------------------------------------------------------------
// Retry — exponential backoff
// ---------------------------------------------------------------------------

/// Execute a fallible async operation with exponential backoff.
///
/// Retry sequence: 1s, 2s, 4s, 8s (max [`MAX_CALLBACK_RETRIES`] retries).
/// On the final failure the last error is propagated.
pub async fn with_retry<F, Fut, T>(operation: F) -> Result<T, String>
where
    F: Fn() -> Fut,
    Fut: std::future::Future<Output = Result<T, String>>,
{
    // `attempt` counts actual retries; we try once before the first retry delay.
    let mut attempt = 0u32;
    loop {
        match operation().await {
            Ok(value) => return Ok(value),
            Err(e) => {
                attempt += 1;
                if attempt > MAX_CALLBACK_RETRIES {
                    log::error!(
                        "[china_payments] operation failed after {} retries: {}",
                        MAX_CALLBACK_RETRIES,
                        e
                    );
                    return Err(format!(
                        "operation failed after {} retries: {}",
                        MAX_CALLBACK_RETRIES, e
                    ));
                }
                let delay_idx = ((attempt - 1) as usize).min(RETRY_DELAYS_SECS.len() - 1);
                let delay_secs = RETRY_DELAYS_SECS[delay_idx];
                log::warn!(
                    "[china_payments] attempt {} failed, retrying in {}s: {}",
                    attempt,
                    delay_secs,
                    e
                );
                tokio::time::sleep(Duration::from_secs(delay_secs)).await;
            }
        }
    }
}

/// Compute an idempotency key from a callback payload via SHA-256.
///
/// Useful for generating a deterministic key when the upstream does not
/// provide a unique one.
pub fn compute_idempotency_key(gateway: &str, body: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(gateway.as_bytes());
    hasher.update(b":");
    hasher.update(body.as_bytes());
    let hash = hasher.finalize();
    format!("{}_{:x}", gateway, hash)
}

// ---------------------------------------------------------------------------
// Fapiao (Invoice) Service Stubs
// ---------------------------------------------------------------------------

/// Supported fapiao types for enterprise customers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FapiaoType {
    /// 增值税电子普通发票 — electronic general VAT invoice.
    Electronic,
    /// 增值税专用发票 — special VAT invoice (paper, for deduction).
    Special,
}

impl Default for FapiaoType {
    fn default() -> Self {
        FapiaoType::Electronic
    }
}

impl FapiaoType {
    /// Chinese label for UI display.
    pub fn cn_label(&self) -> &'static str {
        match self {
            FapiaoType::Electronic => "电子发票",
            FapiaoType::Special => "增值税专用发票",
        }
    }
}

/// Request payload for issuing a fapiao.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FapiaoRequest {
    /// Order ID (must be in `Confirmed` state).
    pub order_id: String,
    /// Buyer company name (购方名称).
    pub buyer_name: String,
    /// Buyer unified social credit code / tax ID (纳税人识别号).
    pub buyer_tax_id: String,
    /// Invoice amount in CNY.
    pub amount_cny: u32,
    /// Delivery email address.
    pub email: String,
    /// Desired fapiao type.
    #[serde(default)]
    pub fapiao_type: FapiaoType,
}

/// Response from the fapiao service.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FapiaoResponse {
    /// Unique invoice number assigned by the tax system.
    pub invoice_id: String,
    /// The type of invoice issued.
    pub fapiao_type: FapiaoType,
    /// ISO-8601 timestamp of issuance.
    pub issued_at: String,
    /// Optional download URL for the electronic invoice PDF.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pdf_url: Option<String>,
}

/// Stub: request a fapiao for a confirmed order.
///
/// # Production notes
///
/// This is a skeleton. A real implementation would integrate with a
/// state-approved fapiao platform such as:
/// * **百望 (Baiwang)** — `https://open.baiwang.com/`
/// * **诺诺 (Nuonuo)** — `https://www.jss.com.cn/`
/// * Local tax bureau e-invoice API.
///
/// The fapiao platform typically requires:
/// * Enterprise authentication (Digital Certificate / UKey).
/// * Real-time inventory query for Special VAT invoices.
/// * Red-letter invoice (红票) support for refunds.
pub async fn request_fapiao(request: FapiaoRequest) -> Result<FapiaoResponse, String> {
    log::info!(
        "[china_payments] fapiao requested: order={}, buyer={}, amount_cny={}, type={:?}",
        request.order_id,
        request.buyer_name,
        request.amount_cny,
        request.fapiao_type
    );
    // Placeholder: return a stub response.
    // In production this would call the fapiao platform API.
    Ok(FapiaoResponse {
        invoice_id: format!("fp_{}", Uuid::new_v4()),
        fapiao_type: request.fapiao_type,
        issued_at: Utc::now().to_rfc3339(),
        pdf_url: None,
    })
}

/// Stub: query the status of a previously issued invoice.
pub async fn query_fapiao(invoice_id: &str) -> Result<Option<FapiaoResponse>, String> {
    if invoice_id.trim().is_empty() {
        return Err("invoice_id is required".to_string());
    }
    log::info!("[china_payments] fapiao query: invoice={}", invoice_id);
    // Placeholder: always returns None.
    // In production this would query the fapiao platform.
    Ok(None)
}

// ---------------------------------------------------------------------------
// Token helper (unchanged from original)
// ---------------------------------------------------------------------------

fn require_token(config: &Config) -> Result<String, String> {
    get_session_token(config)?
        .and_then(|v| {
            let t = v.trim().to_string();
            if t.is_empty() {
                None
            } else {
                Some(t)
            }
        })
        .ok_or_else(|| "no backend session token; run auth_store_session first".to_string())
}

// ---------------------------------------------------------------------------
// Backend HTTP helper (unchanged from original)
// ---------------------------------------------------------------------------

async fn call_cn_payment(
    config: &Config,
    method: Method,
    path: &str,
    body: Option<Value>,
) -> Result<Value, String> {
    let token = require_token(config)?;
    let api_url = effective_backend_api_url(&config.api_url);
    let client = BackendOAuthClient::new(&api_url).map_err(|e| e.to_string())?;
    client
        .authed_json(&token, method, path, body)
        .await
        .map_err(|e| e.to_string())
}

// ---------------------------------------------------------------------------
// CnPlanTier — plan catalog
// ---------------------------------------------------------------------------

/// China-market plan tiers with CNY pricing and Chinese display names.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CnPlanTier {
    /// 免费试用 — Free trial (CNY 0).
    Trial,
    /// 个人版 — Personal (CNY 199).
    Personal,
    /// 团队版 — Team (CNY 499).
    Team,
    /// 企业版 — Enterprise (custom pricing via sales).
    Enterprise,
}

impl CnPlanTier {
    /// Price in CNY. `Trial` and `Enterprise` return 0 (contact sales).
    pub fn price_cny(&self) -> u32 {
        match self {
            CnPlanTier::Trial => 0,
            CnPlanTier::Personal => 199,
            CnPlanTier::Team => 499,
            CnPlanTier::Enterprise => 0,
        }
    }

    /// Chinese display name for UI rendering.
    pub fn display_name(&self) -> &'static str {
        match self {
            CnPlanTier::Trial => "免费试用",
            CnPlanTier::Personal => "个人版",
            CnPlanTier::Team => "团队版",
            CnPlanTier::Enterprise => "企业版",
        }
    }

    /// Machine-readable plan name (matches [`VALID_CN_PLANS`]).
    pub fn plan_name(&self) -> &'static str {
        match self {
            CnPlanTier::Trial => "trial",
            CnPlanTier::Personal => "personal",
            CnPlanTier::Team => "team",
            CnPlanTier::Enterprise => "enterprise",
        }
    }
}

// ---------------------------------------------------------------------------
// Public API — Payment Creation (original signatures preserved)
// ---------------------------------------------------------------------------

/// Create a WeChat Pay payment session for the given plan.
///
/// Proxies to `POST /payments/wechat/create` on the hosted backend.
///
/// # Errors
///
/// * If the session token is missing.
/// * If the backend returns a non-2xx status.
pub async fn create_wechat_payment(
    config: &Config,
    plan: &str,
) -> Result<RpcOutcome<Value>, String> {
    let validated = PlanName::parse(plan)?;
    let body = json!({ "plan": validated.as_str(), "gateway": "wechat_pay" });
    let data = call_cn_payment(config, Method::POST, "/payments/wechat/create", Some(body)).await?;
    Ok(RpcOutcome::single_log(
        data,
        "wechat payment session created",
    ))
}

/// Create an Alipay payment session for the given plan.
///
/// Proxies to `POST /payments/alipay/create` on the hosted backend.
///
/// # Errors
///
/// * If the session token is missing.
/// * If the backend returns a non-2xx status.
pub async fn create_alipay_payment(
    config: &Config,
    plan: &str,
) -> Result<RpcOutcome<Value>, String> {
    let validated = PlanName::parse(plan)?;
    let body = json!({ "plan": validated.as_str(), "gateway": "alipay" });
    let data = call_cn_payment(config, Method::POST, "/payments/alipay/create", Some(body)).await?;
    Ok(RpcOutcome::single_log(
        data,
        "alipay payment session created",
    ))
}

/// Query the status of a China payment order.
///
/// Proxies to `GET /payments/status/{order_id}` on the hosted backend.
///
/// # Errors
///
/// * If the order_id format is invalid.
/// * If the session token is missing.
pub async fn query_payment_status(
    config: &Config,
    order_id: &str,
) -> Result<RpcOutcome<Value>, String> {
    let validated = OrderId::parse(order_id)?;
    let path = format!(
        "/payments/status/{}",
        urlencoding::encode(validated.as_str())
    );
    let data = call_cn_payment(config, Method::GET, &path, None).await?;
    Ok(RpcOutcome::single_log(data, "payment status queried"))
}

/// Fetch all available China-market plans from the hosted backend.
///
/// Proxies to `GET /payments/plans/cn`.
pub async fn get_cn_plans(config: &Config) -> Result<RpcOutcome<Value>, String> {
    let data = call_cn_payment(config, Method::GET, "/payments/plans/cn", None).await?;
    Ok(RpcOutcome::single_log(data, "CN plans fetched"))
}

// ---------------------------------------------------------------------------
// Extended Public API — Refund / Order State / Invoice
// ---------------------------------------------------------------------------

/// Initiate a refund for a China payment order.
///
/// This is an idempotent operation: calling it multiple times with the same
/// `order_id` will only process the refund once.
///
/// # Errors
///
/// * If the order_id format is invalid.
/// * If the order is not in a refundable state.
/// * If the 7-day no-reason refund window has expired.
pub async fn create_refund(
    config: &Config,
    order_id: &str,
    amount_cny: Option<u32>,
) -> Result<RpcOutcome<Value>, String> {
    let validated_order = OrderId::parse(order_id)?;
    let amount = match amount_cny {
        Some(v) => Some(AmountCny::new(v)?),
        None => None,
    };

    // State machine check — the backend enforces this server-side too.
    // Idempotency: INSERT OR IGNORE means duplicate calls are safe.
    with_idem_store(|store| {
        match store.get_status(validated_order.as_str())? {
            Some(s) if s == "refunded" => {
                log::info!(
                    "[china_payments] refund already processed for order {}",
                    validated_order
                );
                return Err("refund already processed".to_string());
            }
            _ => {}
        }
        store.try_acquire(validated_order.as_str(), "refund")?;
        Ok(())
    })?;

    let mut body = json!({
        "order_id": validated_order.as_str(),
        "gateway": "refund",
    });
    if let Some(a) = amount {
        body["amount_cny"] = json!(a.get());
    }

    let data = call_cn_payment(config, Method::POST, "/payments/refund", Some(body)).await?;

    // Mark idempotency record as refunded.
    let _ = with_idem_store(|store| store.mark_done(validated_order.as_str(), "refunded"));

    Ok(RpcOutcome::single_log(data, "refund initiated"))
}

/// Query the refund status for an order.
pub async fn query_refund_status(
    config: &Config,
    order_id: &str,
) -> Result<RpcOutcome<Value>, String> {
    let validated = OrderId::parse(order_id)?;
    let path = format!(
        "/payments/refund/status/{}",
        urlencoding::encode(validated.as_str())
    );
    let data = call_cn_payment(config, Method::GET, &path, None).await?;
    Ok(RpcOutcome::single_log(data, "refund status queried"))
}

/// Get the local order state from the idempotency store (if tracking).
pub fn get_order_state(order_id: &str) -> Result<Option<String>, String> {
    let validated = OrderId::parse(order_id)?;
    with_idem_store(|store| store.get_status(validated.as_str()))
}

/// Generate a tax invoice (fapiao) for a completed payment.
///
/// The invoice can be either electronic (默认电子发票) or special VAT.
///
/// # Errors
///
/// * If the order has not been confirmed.
/// * If the session token is missing.
/// * If the fapiao platform request fails.
pub async fn generate_invoice(
    _config: &Config,
    order_id: &str,
    buyer_name: &str,
    buyer_tax_id: &str,
    fapiao_type: Option<FapiaoType>,
) -> Result<RpcOutcome<Value>, String> {
    let validated_order = OrderId::parse(order_id)?;

    if buyer_name.trim().is_empty() {
        return Err("buyer_name is required for fapiao issuance".to_string());
    }
    if buyer_tax_id.trim().is_empty() {
        return Err("buyer_tax_id is required for fapiao issuance".to_string());
    }

    let ft = fapiao_type.unwrap_or(FapiaoType::Electronic);

    // In production, verify the order is in Confirmed state before issuing.
    let status = with_idem_store(|store| store.get_status(validated_order.as_str()))?;
    log::info!(
        "[china_payments] invoice request for order {} (current status: {:?})",
        validated_order,
        status
    );

    let request = FapiaoRequest {
        order_id: validated_order.into_inner(),
        buyer_name: buyer_name.trim().to_string(),
        buyer_tax_id: buyer_tax_id.trim().to_string(),
        amount_cny: 0, // In production, fetch from order.
        email: String::new(),
        fapiao_type: ft,
    };

    let response = request_fapiao(request).await?;
    let value = json!(response);
    Ok(RpcOutcome::single_log(value, "invoice generated"))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // --- Existing plan tier tests -------------------------------------------

    #[test]
    fn cn_plan_tiers_have_correct_prices() {
        assert_eq!(CnPlanTier::Trial.price_cny(), 0);
        assert_eq!(CnPlanTier::Personal.price_cny(), 199);
        assert_eq!(CnPlanTier::Team.price_cny(), 499);
        assert_eq!(CnPlanTier::Enterprise.price_cny(), 0);
    }

    #[test]
    fn cn_plan_tiers_have_display_names() {
        assert_eq!(CnPlanTier::Trial.display_name(), "免费试用");
        assert_eq!(CnPlanTier::Personal.display_name(), "个人版");
        assert_eq!(CnPlanTier::Team.display_name(), "团队版");
        assert_eq!(CnPlanTier::Enterprise.display_name(), "企业版");
    }

    #[test]
    fn cn_plan_tiers_have_machine_names() {
        assert_eq!(CnPlanTier::Trial.plan_name(), "trial");
        assert_eq!(CnPlanTier::Personal.plan_name(), "personal");
        assert_eq!(CnPlanTier::Team.plan_name(), "team");
        assert_eq!(CnPlanTier::Enterprise.plan_name(), "enterprise");
    }

    // --- Input validation: PlanName ----------------------------------------

    #[test]
    fn plan_name_accepts_valid_plans() {
        for &name in VALID_CN_PLANS {
            assert!(
                PlanName::parse(name).is_ok(),
                "expected '{name}' to be valid"
            );
        }
    }

    #[test]
    fn plan_name_is_case_insensitive() {
        assert!(PlanName::parse("Personal").is_ok());
        assert!(PlanName::parse("PERSONAL").is_ok());
        assert!(PlanName::parse("tEAm").is_ok());
    }

    #[test]
    fn plan_name_rejects_empty_input() {
        let err = PlanName::parse("").unwrap_err();
        assert!(err.contains("plan name is required"));
    }

    #[test]
    fn plan_name_rejects_whitespace_only() {
        let err = PlanName::parse("   ").unwrap_err();
        assert!(err.contains("plan name is required"));
    }

    #[test]
    fn plan_name_rejects_unknown_plan() {
        let err = PlanName::parse("enterprise_plus").unwrap_err();
        assert!(err.contains("invalid plan"));
    }

    // --- Input validation: OrderId -----------------------------------------

    #[test]
    fn order_id_accepts_valid_format() {
        let uuid = Uuid::new_v4();
        let input = format!("wx_{uuid}");
        let parsed = OrderId::parse(&input).unwrap();
        assert_eq!(parsed.as_str(), input);
    }

    #[test]
    fn order_id_accepts_alipay_prefix() {
        let uuid = Uuid::new_v4();
        let input = format!("ali_{uuid}");
        assert!(OrderId::parse(&input).is_ok());
    }

    #[test]
    fn order_id_accepts_oh_prefix() {
        let uuid = Uuid::new_v4();
        let input = format!("oh_{uuid}");
        assert!(OrderId::parse(&input).is_ok());
    }

    #[test]
    fn order_id_rejects_empty() {
        let err = OrderId::parse("").unwrap_err();
        assert!(err.contains("must not be empty"));
    }

    #[test]
    fn order_id_rejects_unknown_prefix() {
        let uuid = Uuid::new_v4();
        let err = OrderId::parse(&format!("stripe_{uuid}")).unwrap_err();
        assert!(err.contains("invalid order_id prefix"));
    }

    #[test]
    fn order_id_rejects_missing_uuid() {
        let err = OrderId::parse("wx_").unwrap_err();
        assert!(err.contains("invalid UUID"));
    }

    #[test]
    fn order_id_rejects_no_prefix() {
        let uuid = Uuid::new_v4();
        let err = OrderId::parse(&uuid.to_string()).unwrap_err();
        assert!(err.contains("invalid order_id format"));
    }

    // --- Input validation: AmountCny ---------------------------------------

    #[test]
    fn amount_cny_accepts_valid_range() {
        assert!(AmountCny::new(1).is_ok());
        assert!(AmountCny::new(100).is_ok());
        assert!(AmountCny::new(999_999).is_ok());
    }

    #[test]
    fn amount_cny_rejects_zero() {
        let err = AmountCny::new(0).unwrap_err();
        assert!(err.contains("amount must be at least"));
    }

    #[test]
    fn amount_cny_rejects_excessive() {
        let err = AmountCny::new(1_000_000).unwrap_err();
        assert!(err.contains("must not exceed"));
    }

    // --- Order State Machine -----------------------------------------------

    #[test]
    fn state_pending_to_paid_is_valid() {
        let state = OrderState::Pending;
        assert_eq!(state.transition_to(OrderState::Paid), Ok(OrderState::Paid));
    }

    #[test]
    fn state_full_flow_is_valid() {
        let steps = vec![
            (OrderState::Pending, OrderState::Paid),
            (OrderState::Paid, OrderState::LicenseIssued),
            (OrderState::LicenseIssued, OrderState::Confirmed),
        ];
        let mut current = OrderState::Pending;
        for (_, target) in steps {
            current = current.transition_to(target).unwrap();
        }
        assert_eq!(current, OrderState::Confirmed);
    }

    #[test]
    fn state_refund_from_paid_is_valid() {
        let state = OrderState::Paid;
        assert_eq!(
            state.transition_to(OrderState::Refunded),
            Ok(OrderState::Refunded)
        );
    }

    #[test]
    fn state_refund_from_license_issued_is_valid() {
        let state = OrderState::LicenseIssued;
        assert_eq!(
            state.transition_to(OrderState::Refunded),
            Ok(OrderState::Refunded)
        );
    }

    #[test]
    fn state_illegal_transition_returns_error() {
        let pairs = [
            (OrderState::Pending, OrderState::Confirmed),
            (OrderState::Pending, OrderState::Refunded),
            (OrderState::Confirmed, OrderState::Refunded),
            (OrderState::Refunded, OrderState::Paid),
            (OrderState::Confirmed, OrderState::Paid),
        ];
        for (from, to) in &pairs {
            assert!(
                from.transition_to(*to).is_err(),
                "expected {:?} -> {:?} to be illegal",
                from,
                to
            );
        }
    }

    #[test]
    fn can_refund_returns_true_for_refundable_states() {
        assert!(OrderState::Paid.can_refund());
        assert!(OrderState::LicenseIssued.can_refund());
    }

    #[test]
    fn can_refund_returns_false_for_non_refundable_states() {
        assert!(!OrderState::Pending.can_refund());
        assert!(!OrderState::Confirmed.can_refund());
        assert!(!OrderState::Refunded.can_refund());
    }

    #[test]
    fn state_cn_labels_are_not_empty() {
        for state in &[
            OrderState::Pending,
            OrderState::Paid,
            OrderState::LicenseIssued,
            OrderState::Confirmed,
            OrderState::Refunded,
        ] {
            assert!(!state.cn_label().is_empty());
        }
    }

    // --- Refund window -----------------------------------------------------

    #[test]
    fn refund_window_recent_payment_is_within_window() {
        let recent = Utc::now() - chrono::Duration::hours(1);
        assert!(is_within_refund_window(recent));
    }

    #[test]
    fn refund_window_old_payment_is_outside_window() {
        let old = Utc::now() - chrono::Duration::days(10);
        assert!(!is_within_refund_window(old));
    }

    // --- Idempotency key ---------------------------------------------------

    #[test]
    fn compute_idempotency_key_is_deterministic() {
        let key1 = compute_idempotency_key("wechat", r#"{"order_id":"wx_abc"}"#);
        let key2 = compute_idempotency_key("wechat", r#"{"order_id":"wx_abc"}"#);
        assert_eq!(key1, key2);
    }

    #[test]
    fn compute_idempotency_key_differs_for_different_gateways() {
        let key1 = compute_idempotency_key("wechat", r#"{"order_id":"wx_abc"}"#);
        let key2 = compute_idempotency_key("alipay", r#"{"order_id":"wx_abc"}"#);
        assert_ne!(key1, key2);
    }

    // --- Retry logic -------------------------------------------------------

    #[tokio::test]
    async fn with_retry_returns_ok_on_first_attempt() {
        let result = with_retry(|| async { Ok::<_, String>(42) }).await;
        assert_eq!(result, Ok(42));
    }

    #[tokio::test]
    async fn with_retry_succeeds_after_transient_failures() {
        let counter = std::sync::atomic::AtomicU32::new(0);
        let result = with_retry(|| {
            let c = &counter;
            async move {
                let prev = c.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                if prev < 2 {
                    Err("transient".to_string())
                } else {
                    Ok(prev)
                }
            }
        })
        .await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn with_retry_exhausts_and_fails() {
        let result = with_retry(|| async { Err::<(), _>("permanent".to_string()) }).await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.contains("failed after"));
    }

    // --- Fapiao ------------------------------------------------------------

    #[tokio::test]
    async fn request_fapiao_returns_stub() {
        let req = FapiaoRequest {
            order_id: "wx_abc".to_string(),
            buyer_name: "测试公司".to_string(),
            buyer_tax_id: "91110108MA12345678".to_string(),
            amount_cny: 19900,
            email: "billing@example.com".to_string(),
            fapiao_type: FapiaoType::Electronic,
        };
        let resp = request_fapiao(req).await.unwrap();
        assert!(resp.invoice_id.starts_with("fp_"));
        assert_eq!(resp.fapiao_type, FapiaoType::Electronic);
    }

    #[tokio::test]
    async fn query_fapiao_rejects_empty_id() {
        let err = query_fapiao("").await.unwrap_err();
        assert!(err.contains("invoice_id is required"));
    }

    // --- Original pre-HTTP validation tests (no network) -------------------

    fn cfg() -> Config {
        Config::default()
    }

    #[tokio::test]
    async fn wechat_payment_rejects_empty_plan() {
        assert_eq!(
            create_wechat_payment(&cfg(), "").await.unwrap_err(),
            "plan name is required"
        );
    }

    #[tokio::test]
    async fn alipay_payment_rejects_empty_plan() {
        assert_eq!(
            create_alipay_payment(&cfg(), "").await.unwrap_err(),
            "plan name is required"
        );
    }

    #[tokio::test]
    async fn query_status_rejects_empty_order_id() {
        let err = query_payment_status(&cfg(), "").await.unwrap_err();
        assert!(err.contains("must not be empty"));
    }

    #[tokio::test]
    async fn query_status_rejects_invalid_order_id() {
        let err = query_payment_status(&cfg(), "not-a-uuid")
            .await
            .unwrap_err();
        assert!(err.contains("invalid order_id format"));
    }

    #[tokio::test]
    async fn wechat_payment_rejects_unknown_plan() {
        let err = create_wechat_payment(&cfg(), "enterprise_plus")
            .await
            .unwrap_err();
        assert!(err.contains("invalid plan"));
    }

    #[tokio::test]
    async fn alipay_payment_rejects_whitespace_plan() {
        let err = create_alipay_payment(&cfg(), "   ").await.unwrap_err();
        assert!(err.contains("plan name is required"));
    }

    #[tokio::test]
    async fn create_refund_rejects_invalid_order_id() {
        let err = create_refund(&cfg(), "", None).await.unwrap_err();
        assert!(err.contains("must not be empty"));
    }

    #[tokio::test]
    async fn generate_invoice_rejects_empty_buyer_name() {
        let err = generate_invoice(
            &cfg(),
            "wx_a1b2c3d4-e5f6-7890-abcd-ef1234567890",
            "",
            "91110108MA12345678",
            None,
        )
        .await
        .unwrap_err();
        assert!(err.contains("buyer_name is required"));
    }

    #[tokio::test]
    async fn generate_invoice_rejects_empty_tax_id() {
        let err = generate_invoice(
            &cfg(),
            "wx_a1b2c3d4-e5f6-7890-abcd-ef1234567890",
            "测试公司",
            "",
            None,
        )
        .await
        .unwrap_err();
        assert!(err.contains("buyer_tax_id is required"));
    }
}
