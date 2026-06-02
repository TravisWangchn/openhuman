// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OpenHuman-ZN Contributors

//! License operations: activation, validation, quota enforcement, offline Ed25519
//! verification, hardware-fingerprint binding, and state-machine management.
//!
//! # State machine
//!
//! ```text
//! Unlicensed ──► Trial ──► Active ──► Expired
//!      │                    │            │
//!      └────────────────────┼────────────┘
//!                           │
//!                      FallbackGrace
//!                           │
//!                      Revoked
//! ```
//!
//! # Architecture
//!
//! - **In-memory cache** via `RwLock<LicenseState>` for O(1) access checks.
//! - **SQLite backing** for daily quota, activation attempt rate-limits, and
//!   offline signature cache (via `rusqlite`).
//! - **Ed25519** using the [`ring`] crate: the hardcoded public key in this
//!   binary validates offline activation blobs so the app stays functional
//!   when the licensing server is unreachable.
//! - **Hardware binding**: CPU identifier + motherboard UUID + hostname,
//!   hashed with SHA-256 to produce a device fingerprint hash.
//! - **Rate limiting**: max 5 failed activation attempts per 24-hour window,
//!   persisted in SQLite to survive restarts.
//! - **SafeMode**: when the server has been unreachable for >72 consecutive
//!   hours, the state machine auto-transitions to `FallbackGrace`, keeping
//!   local-only features alive.

use std::path::PathBuf;
use std::sync::RwLock;

use anyhow::{Context, Result};
use chrono::{DateTime, Duration, Utc};
use log::{debug, error, info, warn};
use once_cell::sync::Lazy;
use ring::signature::{UnparsedPublicKey, ED25519};
use rusqlite::{params, Connection};
use sha2::{Digest, Sha256};

use super::types::*;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Ed25519 public key (32 bytes) used to verify offline license signatures.
///
/// This is the root signing key for the OpenHuman-ZN license server.
/// Replace the bytes with your deployed public key before shipping a release
/// build. A zeroed key will cause every offline signature check to fail,
/// effectively disabling offline activation.
pub(crate) const LICENSE_SERVER_PUBKEY_BYTES: [u8; 32] = [0u8; 32];

/// Maximum number of failed activation attempts per calendar day before the
/// activation endpoint locks out for the remainder of the day.
const MAX_FAILED_ACTIVATIONS_PER_DAY: u32 = 5;

/// Duration (in hours) of server unreachability before the state machine
/// auto-transitions from `Active` to `FallbackGrace`.
const SAFEMODE_GRACE_HOURS: i64 = 72;

/// Schema version tracked in the meta table for future migrations.
const SCHEMA_VERSION: i64 = 1;

// ---------------------------------------------------------------------------
// Database helpers
// ---------------------------------------------------------------------------

/// Resolve the path to the SQLite database storing persistent license state.
///
/// The database lives under the user's platform-appropriate data directory:
///
/// | Platform | Path |
/// |----------|------|
/// | Linux    | `$XDG_DATA_HOME/OpenHuman-ZN/license/state.db` |
/// | macOS    | `$HOME/Library/Application Support/OpenHuman-ZN/license/state.db` |
/// | Windows  | `C:\Users\<user>\AppData\Local\OpenHuman-ZN\license\state.db` |
fn db_path() -> PathBuf {
    let base = dirs::data_dir().unwrap_or_else(|| PathBuf::from("."));
    let dir = base.join("OpenHuman-ZN").join("license");
    let _ = std::fs::create_dir_all(&dir);
    dir.join("state.db")
}

/// Open (or create) the SQLite database and run schema migrations.
///
/// Enables WAL journal mode for better concurrent read performance from
/// multiple tokio tasks calling `record_usage` simultaneously.
fn open_db() -> Result<Connection> {
    let path = db_path();
    let conn = Connection::open(&path)
        .with_context(|| format!("failed to open license database at {}", path.display()))?;

    conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=NORMAL;")
        .context("failed to set pragmas on license database")?;

    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS meta (
            key   TEXT PRIMARY KEY,
            value TEXT NOT NULL
        ) STRICT;

        CREATE TABLE IF NOT EXISTS daily_usage (
            date          TEXT    PRIMARY KEY,
            request_count INTEGER NOT NULL DEFAULT 0,
            token_count   INTEGER NOT NULL DEFAULT 0
        ) STRICT;

        CREATE TABLE IF NOT EXISTS activation_failures (
            date       TEXT    PRIMARY KEY,
            fail_count INTEGER NOT NULL DEFAULT 0
        ) STRICT;",
    )
    .context("failed to create license schema tables")?;

    conn.execute(
        "INSERT OR IGNORE INTO meta (key, value) VALUES ('schema_version', ?1)",
        params![SCHEMA_VERSION],
    )?;

    info!(
        "[license] database opened at {} (schema v{})",
        path.display(),
        SCHEMA_VERSION
    );
    Ok(conn)
}

/// Acquire the database connection, lazily initialising it on first call.
///
/// The closure `f` receives an exclusive borrow of the connection. The write
/// lock on `DB` is held for the duration of `f`, so keep operations short.
fn with_db<F, T>(f: F) -> Result<T>
where
    F: FnOnce(&Connection) -> Result<T>,
{
    let mut guard = DB
        .lock()
        .map_err(|e| anyhow::anyhow!("license database lock poisoned: {e}"))?;
    if guard.is_none() {
        *guard = Some(open_db()?);
    }
    let conn = guard.as_ref().expect("DB initialized just above");
    f(conn)
}

// ---------------------------------------------------------------------------
// Meta key-value helpers (thin wrapper over the `meta` table)
// ---------------------------------------------------------------------------

/// Read a value from the `meta` table.
fn meta_get(conn: &Connection, key: &str) -> Result<Option<String>> {
    let mut stmt = conn.prepare("SELECT value FROM meta WHERE key = ?1")?;
    let mut rows = stmt.query(params![key])?;
    match rows.next()? {
        Some(row) => Ok(Some(row.get::<_, String>(0)?)),
        None => Ok(None),
    }
}

/// Upsert a value in the `meta` table.
fn meta_set(conn: &Connection, key: &str, value: &str) -> Result<()> {
    conn.execute(
        "INSERT OR REPLACE INTO meta (key, value) VALUES (?1, ?2)",
        params![key, value],
    )?;
    Ok(())
}

/// Delete a key from the `meta` table.
fn meta_delete(conn: &Connection, key: &str) -> Result<()> {
    conn.execute("DELETE FROM meta WHERE key = ?1", params![key])?;
    Ok(())
}

// ---------------------------------------------------------------------------
// In-memory caches
// ---------------------------------------------------------------------------

/// In-memory license state (backed by SQLite for persistence across restarts).
static LICENSE_STATE: Lazy<RwLock<LicenseState>> =
    Lazy::new(|| RwLock::new(LicenseState::Unlicensed));

/// In-memory daily usage cache (flushed to SQLite on every write).
static DAILY_USAGE: Lazy<RwLock<DailyUsage>> = Lazy::new(|| {
    RwLock::new(DailyUsage {
        date: String::new(),
        request_count: 0,
        token_count: 0,
    })
});

/// Lazily-initialised SQLite connection, wrapped in a mutex for interior
/// mutability across tokio tasks.  Uses `std::sync::Mutex` because
/// `rusqlite::Connection` is `Send` but not `Sync` (it contains `RefCell`).
static DB: Lazy<std::sync::Mutex<Option<Connection>>> = Lazy::new(|| std::sync::Mutex::new(None));

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Return today's date as an ISO-8601 date string in the UTC+8 time zone
/// (Asia/Shanghai), which is the canonical date boundary for quota resets.
fn today_iso() -> String {
    let now = Utc::now();
    // UTC+8 = Asia/Shanghai (no DST, always +8).
    let shanghai = now + Duration::hours(8);
    shanghai.format("%Y-%m-%d").to_string()
}

/// Build a device fingerprint hash from CPU ID, motherboard serial/UUID,
/// and hostname.
///
/// The three raw identifiers are concatenated with `|` separators and fed
/// through SHA-256. This produces a stable, collision-resistant binding
/// that changes only when the hardware changes.
fn fingerprint_device() -> DeviceFingerprint {
    let hostname = std::env::var("COMPUTERNAME")
        .or_else(|_| std::env::var("HOSTNAME"))
        .unwrap_or_else(|_| "unknown".to_string());

    let cpu_id = get_cpu_id();
    let machine_uid = get_machine_uuid();

    let combined = format!("{cpu_id}|{machine_uid}|{hostname}");
    let hash = hex::encode(Sha256::digest(combined.as_bytes()));

    debug!(
        "[license] fingerprint: hostname={}, hash={}",
        hostname, hash
    );

    DeviceFingerprint {
        hostname,
        hardware_hash: hash,
    }
}

/// Read the CPU identifier string from the platform-appropriate source.
///
/// | Platform | Source |
/// |----------|--------|
/// | Windows  | `wmic cpu get processorid` |
/// | macOS    | `sysctl -n machdep.cpu.brand_string` |
/// | Linux    | `/proc/cpuinfo` (first `model name` line) |
fn get_cpu_id() -> String {
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("wmic")
            .args(["cpu", "get", "processorid"])
            .output()
            .ok()
            .and_then(|out| {
                let s = String::from_utf8_lossy(&out.stdout);
                s.lines().nth(1).map(|l| l.trim().to_string())
            })
            .filter(|s| !s.is_empty() && s != "unknown")
            .unwrap_or_else(|| "unknown-cpu".to_string())
    }
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("sysctl")
            .args(["-n", "machdep.cpu.brand_string"])
            .output()
            .ok()
            .and_then(|out| {
                let s = String::from_utf8_lossy(&out.stdout);
                let trimmed = s.trim().to_string();
                if trimmed.is_empty() {
                    None
                } else {
                    Some(trimmed)
                }
            })
            .unwrap_or_else(|| "unknown-cpu".to_string())
    }
    #[cfg(target_os = "linux")]
    {
        std::fs::read_to_string("/proc/cpuinfo")
            .ok()
            .and_then(|content| {
                content
                    .lines()
                    .find(|l| l.starts_with("model name"))
                    .and_then(|l| l.split(':').nth(1))
                    .map(|s| s.trim().to_string())
            })
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "unknown-cpu".to_string())
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    {
        "unknown-cpu".to_string()
    }
}

/// Read the machine-unique identifier (motherboard serial/UUID).
///
/// | Platform | Source |
/// |----------|--------|
/// | Windows  | `wmic csproduct get uuid` |
/// | macOS    | `ioreg -rd1 -c IOPlatformExpertDevice` |
/// | Linux    | `/etc/machine-id` |
fn get_machine_uuid() -> String {
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("wmic")
            .args(["csproduct", "get", "uuid"])
            .output()
            .ok()
            .and_then(|out| {
                let s = String::from_utf8_lossy(&out.stdout);
                s.lines().nth(1).map(|l| l.trim().to_string())
            })
            .filter(|s| !s.is_empty() && s != "unknown")
            .unwrap_or_else(|| "unknown-mb".to_string())
    }
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("ioreg")
            .args(["-rd1", "-c", "IOPlatformExpertDevice"])
            .output()
            .ok()
            .and_then(|out| {
                let s = String::from_utf8_lossy(&out.stdout);
                // The IOPlatformUUID line looks like:
                //   "IOPlatformUUID" = "XXXXXXX-XXXX-XXXX-XXXX-XXXXXXXXXX"
                s.lines()
                    .find(|l| l.contains("IOPlatformUUID"))
                    .and_then(|l| l.split('"').nth(3))
                    .map(|s| s.to_string())
            })
            .unwrap_or_else(|| "unknown-mb".to_string())
    }
    #[cfg(target_os = "linux")]
    {
        std::fs::read_to_string("/etc/machine-id")
            .map(|s| s.trim().to_string())
            .unwrap_or_else(|_| "unknown-mb".to_string())
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    {
        "unknown-mb".to_string()
    }
}

// ---------------------------------------------------------------------------
// Ed25519 offline signature verification
// ---------------------------------------------------------------------------

/// Verify an Ed25519 signature over `message` using the hardcoded
/// [`LICENSE_SERVER_PUBKEY_BYTES`].
///
/// Returns `true` when the signature is cryptographically valid, `false`
/// otherwise (including when the public key is all-zeros — the placeholder
/// shipped in source).
fn verify_ed25519_signature(message: &[u8], signature: &[u8]) -> bool {
    if LICENSE_SERVER_PUBKEY_BYTES == [0u8; 32] {
        warn!(
            "[license] Ed25519 public key is still the default placeholder; \
               offline verification will fail"
        );
        return false;
    }
    let public_key = UnparsedPublicKey::new(&ED25519, &LICENSE_SERVER_PUBKEY_BYTES);
    match public_key.verify(message, signature) {
        Ok(()) => {
            debug!("[license] Ed25519 signature verified successfully");
            true
        }
        Err(e) => {
            warn!("[license] Ed25519 signature verification failed: {e}");
            false
        }
    }
}

/// Build the message that the licensing server signs for offline activation.
///
/// The message is `SHA-256(license_key || "::" || device_hash)`. The server
/// signs this hash with its Ed25519 private key; we re-compute the hash and
/// verify the signature on this end.
fn build_offline_message(license_key: &str, device: &DeviceFingerprint) -> Vec<u8> {
    let input = format!("{}::{}", license_key, device.hardware_hash);
    Sha256::digest(input.as_bytes()).to_vec()
}

// ---------------------------------------------------------------------------
// State persistence (SQLite-backed)
// ---------------------------------------------------------------------------

/// Persist the current license state to SQLite as JSON.
fn persist_license_state(conn: &Connection, state: &LicenseState) -> Result<()> {
    let json = serde_json::to_string(state).context("failed to serialize LicenseState")?;
    meta_set(conn, "license_state", &json)
}

/// Load the persisted license state from SQLite, returning `Unlicensed` if
/// no state has ever been saved.
fn load_license_state(conn: &Connection) -> Result<LicenseState> {
    match meta_get(conn, "license_state")? {
        Some(json) => serde_json::from_str(&json).context("corrupt license_state in database"),
        None => Ok(LicenseState::Unlicensed),
    }
}

/// Persist today's usage counters to the `daily_usage` table.
fn persist_daily_usage(conn: &Connection, usage: &DailyUsage) -> Result<()> {
    conn.execute(
        "INSERT OR REPLACE INTO daily_usage (date, request_count, token_count)
         VALUES (?1, ?2, ?3)",
        params![usage.date, usage.request_count, usage.token_count],
    )?;
    Ok(())
}

/// Load today's usage from SQLite, returning a zeroed counter if none exists.
fn load_daily_usage(conn: &Connection) -> Result<DailyUsage> {
    let today = today_iso();
    let mut stmt =
        conn.prepare("SELECT request_count, token_count FROM daily_usage WHERE date = ?1")?;
    let mut rows = stmt.query(params![&today])?;
    match rows.next()? {
        Some(row) => Ok(DailyUsage {
            date: today,
            request_count: row.get(0)?,
            token_count: row.get(1)?,
        }),
        None => Ok(DailyUsage {
            date: today,
            request_count: 0,
            token_count: 0,
        }),
    }
}

/// Save an offline activation response and its Ed25519 signature to the cache.
fn persist_offline_cache(
    conn: &Connection,
    response: &LicenseActivationResponse,
    signature: &str,
) -> Result<()> {
    let response_json =
        serde_json::to_string(response).context("failed to serialize offline cache response")?;
    meta_set(conn, "offline_response", &response_json)?;
    meta_set(conn, "offline_signature", signature)?;
    Ok(())
}

/// Attempt to restore a previously-cached offline activation session.
///
/// This function:
/// 1. Loads the cached `LicenseActivationResponse` from SQLite.
/// 2. Rebuilds the signed message from the cached data.
/// 3. Verifies the Ed25519 signature.
/// 4. Returns the response on success, or `None` if no valid cache exists.
fn try_restore_offline_session(
    conn: &Connection,
    license_key: &str,
    device: &DeviceFingerprint,
) -> Result<Option<LicenseActivationResponse>> {
    let response_json = match meta_get(conn, "offline_response")? {
        Some(j) => j,
        None => {
            debug!("[license] no cached offline session found");
            return Ok(None);
        }
    };
    let sig_hex = match meta_get(conn, "offline_signature")? {
        Some(s) => s,
        None => {
            warn!("[license] offline response cached but signature missing; discarding");
            meta_delete(conn, "offline_response")?;
            return Ok(None);
        }
    };

    let resp: LicenseActivationResponse = serde_json::from_str(&response_json)
        .context("corrupt cached offline activation response")?;

    // Rebuild the signed message from the current key + device and verify.
    let message = build_offline_message(license_key, device);
    let signature = match hex::decode(&sig_hex) {
        Ok(s) => s,
        Err(e) => {
            warn!("[license] cached signature is not valid hex: {e}; discarding cache");
            meta_delete(conn, "offline_response")?;
            meta_delete(conn, "offline_signature")?;
            return Ok(None);
        }
    };

    if verify_ed25519_signature(&message, &signature) {
        info!("[license] offline session restored from verified Ed25519 cache");
        Ok(Some(resp))
    } else {
        warn!("[license] cached offline signature invalid; discarding cache");
        meta_delete(conn, "offline_response")?;
        meta_delete(conn, "offline_signature")?;
        Ok(None)
    }
}

/// Update the `last_server_check` timestamp in the meta table.
fn touch_last_server_check(conn: &Connection) -> Result<()> {
    let now = Utc::now().to_rfc3339();
    meta_set(conn, "last_server_check", &now)
}

// ---------------------------------------------------------------------------
// Rate limiting (activation attempt throttling)
// ---------------------------------------------------------------------------

/// Check whether activation attempts have been rate-limited for today.
///
/// Returns an error with a descriptive message if the caller is locked out.
fn check_activation_rate_limit(conn: &Connection) -> Result<()> {
    let today = today_iso();
    let mut stmt = conn.prepare("SELECT fail_count FROM activation_failures WHERE date = ?1")?;
    let mut rows = stmt.query(params![&today])?;
    if let Some(row) = rows.next()? {
        let count: u32 = row.get(0)?;
        if count >= MAX_FAILED_ACTIVATIONS_PER_DAY {
            warn!(
                "[license] activation rate limit hit: {} failures today",
                count
            );
            return Err(anyhow::anyhow!(
                "激活尝试次数过多（{} 次/日），请 24 小时后再试",
                MAX_FAILED_ACTIVATIONS_PER_DAY
            ));
        }
    }
    Ok(())
}

/// Record a failed activation attempt, incrementing the daily counter.
fn record_activation_failure(conn: &Connection) -> Result<()> {
    let today = today_iso();
    conn.execute(
        "INSERT INTO activation_failures (date, fail_count)
         VALUES (?1, 1)
         ON CONFLICT(date) DO UPDATE SET fail_count = fail_count + 1",
        params![&today],
    )?;
    let mut stmt = conn.prepare("SELECT fail_count FROM activation_failures WHERE date = ?1")?;
    let count: u32 = stmt
        .query_row(params![&today], |row| row.get(0))
        .unwrap_or(0);
    info!(
        "[license] activation failure recorded ({} / {} today)",
        count, MAX_FAILED_ACTIVATIONS_PER_DAY
    );
    Ok(())
}

/// Reset today's activation failure counter (called after a successful
/// remote activation).
fn reset_activation_failures(conn: &Connection) -> Result<()> {
    let today = today_iso();
    conn.execute(
        "DELETE FROM activation_failures WHERE date = ?1",
        params![&today],
    )?;
    Ok(())
}

// ---------------------------------------------------------------------------
// SafeMode / FallbackGrace
// ---------------------------------------------------------------------------

/// Check whether the license server has been unreachable for longer than
/// [`SAFEMODE_GRACE_HOURS`]. If the current state is `Active` and the
/// threshold has been exceeded, transition to `FallbackGrace`.
///
/// Returns the (possibly modified) license state.
fn apply_safemode_transition(conn: &Connection, state: LicenseState) -> Result<LicenseState> {
    let last_check_raw = match meta_get(conn, "last_server_check")? {
        Some(s) => s,
        None => return Ok(state), // Never checked — not yet in grace.
    };

    let last_check: DateTime<Utc> = match last_check_raw.parse() {
        Ok(dt) => dt,
        Err(e) => {
            warn!("[license] corrupt last_server_check timestamp: {e}; ignoring");
            return Ok(state);
        }
    };

    let elapsed_hours = (Utc::now() - last_check).num_hours().max(0);

    if elapsed_hours < SAFEMODE_GRACE_HOURS {
        return Ok(state);
    }

    match &state {
        LicenseState::Active { tier, .. } => {
            let reason = format!(
                "许可证服务器已离线 {} 小时（阈值: {} 小时）",
                elapsed_hours, SAFEMODE_GRACE_HOURS
            );
            let grace = LicenseState::FallbackGrace {
                original_tier: tier.clone(),
                entered_at: Utc::now(),
                reason,
            };
            warn!(
                "[license] SafeMode activated: transitioning Active → FallbackGrace \
                 ({}h offline, threshold {}h)",
                elapsed_hours, SAFEMODE_GRACE_HOURS
            );
            persist_license_state(conn, &grace)?;
            Ok(grace)
        }
        LicenseState::FallbackGrace { .. } => {
            // Already in grace — that's fine, just log.
            debug!(
                "[license] SafeMode: already in FallbackGrace \
                 ({}h offline, threshold {}h)",
                elapsed_hours, SAFEMODE_GRACE_HOURS
            );
            Ok(state)
        }
        _ => {
            // Non-active states don't enter FallbackGrace.
            Ok(state)
        }
    }
}

/// Attempt to restore from `FallbackGrace` back to `Active` if the server
/// has become reachable again. Called after a successful remote activation.
fn exit_safemode(
    conn: &Connection,
    tier: LicenseTier,
    activated_at: DateTime<Utc>,
) -> Result<LicenseState> {
    let state = LicenseState::Active { tier, activated_at };
    info!("[license] SafeMode cleared: restored to Active");
    persist_license_state(conn, &state)?;
    Ok(state)
}

// ---------------------------------------------------------------------------
// Initialisation
// ---------------------------------------------------------------------------

/// Populate the in-memory caches from the SQLite database.
///
/// Safe to call multiple times — the second call is a no-op (guarded by
/// checking whether `DAILY_USAGE` already has a non-empty date).
pub(crate) fn init_caches() -> Result<()> {
    // Fast path: already initialised.
    {
        let usage = DAILY_USAGE
            .read()
            .map_err(|e| anyhow::anyhow!("license usage lock poisoned: {e}"))?;
        if !usage.date.is_empty() {
            return Ok(());
        }
    }

    with_db(|conn| {
        // 1. Load license state.
        let state = load_license_state(conn)?;
        let state = apply_safemode_transition(conn, state)?;

        {
            let mut guard = LICENSE_STATE
                .write()
                .map_err(|e| anyhow::anyhow!("license state lock poisoned: {e}"))?;
            *guard = state;
        }

        // 2. Load today's daily usage.
        let today_usage = load_daily_usage(conn)?;
        {
            let mut guard = DAILY_USAGE
                .write()
                .map_err(|e| anyhow::anyhow!("license usage lock poisoned: {e}"))?;
            *guard = today_usage;
        }

        info!("[license] caches initialised from database");
        Ok(())
    })
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Activate a license by verifying with the remote licensing server.
///
/// The activation process works as follows:
///
/// 1. **Format validation**: The license key must match `XXXX-XXXX-XXXX-XXXX`.
/// 2. **Rate-limit check**: If more than 5 failed attempts have occurred today,
///    the call is rejected immediately.
/// 3. **Remote activation**: Sends the key + device fingerprint + email to the
///    licensing server at `api_url`. If the server responds successfully, the
///    response is cached (including its Ed25519 signature) for offline use.
/// 4. **Offline fallback**: If the server is unreachable, the function attempts
///    to restore a previously-cached, Ed25519-verified activation session. The
///    restored state may be subject to SafeMode transitions.
/// 5. **SafeMode**: If the cache check also fails, a trial offline activation
///    is attempted as a last resort.
///
/// # Errors
///
/// Returns an error if:
/// - The license key format is invalid.
/// - The rate-limit threshold is exceeded.
/// - Both remote and offline activation fail.
pub async fn activate_license(
    license_key: &str,
    user_email: &str,
    api_url: &str,
) -> Result<LicenseActivationResponse> {
    init_caches()?;

    // 1. Format validation.
    if !LicenseKey::is_valid_format(license_key) {
        warn!("[license] activation rejected: invalid key format");
        return Ok(LicenseActivationResponse {
            state: LicenseState::Unlicensed,
            message: "无效的许可证密钥格式（应为 XXXX-XXXX-XXXX-XXXX）".into(),
            server_signature: None,
        });
    }

    let device = fingerprint_device();

    // 2. Rate-limit check.
    with_db(|conn| check_activation_rate_limit(conn))?;

    // 3. Try remote activation.
    let req = LicenseActivationRequest {
        license_key: LicenseKey::new(license_key.to_string()),
        device: device.clone(),
        user_email: user_email.to_string(),
    };

    match activate_remote(&req, api_url).await {
        Ok(resp) => {
            // Update the last server check timestamp.
            let final_state = with_db(|conn| {
                touch_last_server_check(conn)?;
                reset_activation_failures(conn)?;

                // Cache the response and its Ed25519 signature for offline use.
                if let Some(sig) = &resp.server_signature {
                    let message = build_offline_message(license_key, &device);
                    if let Ok(sig_bytes) = hex::decode(sig) {
                        if verify_ed25519_signature(&message, &sig_bytes) {
                            persist_offline_cache(conn, &resp, sig)?;
                            debug!("[license] offline activation cache updated");
                        } else {
                            warn!(
                                "[license] server-provided Ed25519 signature invalid; \
                                   not caching"
                            );
                        }
                    } else {
                        warn!("[license] server returned non-hex signature; skipping cache");
                    }
                }

                // Exit SafeMode if the server responded.
                let state = match &resp.state {
                    LicenseState::Active { tier, activated_at } => {
                        exit_safemode(conn, tier.clone(), *activated_at)?
                    }
                    _ => {
                        persist_license_state(conn, &resp.state)?;
                        resp.state.clone()
                    }
                };

                {
                    let mut cache = LICENSE_STATE
                        .write()
                        .map_err(|e| anyhow::anyhow!("lock poisoned: {e}"))?;
                    *cache = state.clone();
                }

                info!(
                    "[license] remote activation succeeded: tier={:?}",
                    resp.state.tier()
                );
                Ok(state)
            })?;

            Ok(LicenseActivationResponse {
                state: final_state,
                message: resp.message,
                server_signature: resp.server_signature,
            })
        }
        Err(e) => {
            warn!("[license] remote activation failed: {e}");

            // Record the failure for rate-limiting.
            with_db(|conn| record_activation_failure(conn))?;

            // 4. Offline fallback: try the Ed25519-verified cache.
            let offline_resp =
                with_db(|conn| try_restore_offline_session(conn, license_key, &device))?;

            if let Some(resp) = offline_resp {
                // Apply SafeMode transition before accepting the restored state.
                let final_state =
                    with_db(|conn| apply_safemode_transition(conn, resp.state.clone()))?;

                {
                    let mut state = LICENSE_STATE
                        .write()
                        .map_err(|e| anyhow::anyhow!("lock poisoned: {e}"))?;
                    *state = final_state.clone();
                }

                return Ok(LicenseActivationResponse {
                    state: final_state,
                    message: "离线验证成功（已缓存的 Ed25519 签名验证通过）".into(),
                    server_signature: None,
                });
            }

            // 5. Last resort: offline trial activation.
            let resp = activate_offline_trial(license_key)?;
            {
                let mut state = LICENSE_STATE
                    .write()
                    .map_err(|e| anyhow::anyhow!("lock poisoned: {e}"))?;
                *state = resp.state.clone();
            }
            Ok(resp)
        }
    }
}

/// Check if the current license allows the requested operation.
///
/// Returns the current [`LicenseState`]. Callers should use
/// [`LicenseState::is_active`] to determine if access should be granted.
///
/// This function also triggers the SafeMode check: if the persisted `Active`
/// state has been unable to contact the server for more than 72 hours, the
/// state is automatically downgraded to `FallbackGrace`.
pub fn check_access() -> LicenseState {
    // Ensure caches are loaded but don't propagate init errors to the caller.
    if let Err(e) = init_caches() {
        error!("[license] failed to init caches during check_access: {e}");
    }
    LICENSE_STATE
        .read()
        .map(|g| g.clone())
        .unwrap_or(LicenseState::Unlicensed)
}

/// Record a usage event and check against the daily quota.
///
/// This function atomically increments the daily request and token counters
/// in the in-memory cache, then flushes them to SQLite. If the resulting
/// count exceeds the tier's daily quota, an error is returned.
///
/// # Errors
///
/// Returns an error when the daily request quota has been exhausted. The
/// quota is enforced per `LicenseTier` (e.g., Trial: 20 requests/day).
pub fn record_usage(tokens: u64) -> Result<()> {
    init_caches()?;

    let today = today_iso();
    let mut usage = DAILY_USAGE
        .write()
        .map_err(|e| anyhow::anyhow!("license usage lock poisoned: {e}"))?;

    // Date rollover: reset daily counter.
    if usage.date != today {
        info!("[license] daily rollover: {} → {}", usage.date, today);
        usage.date = today;
        usage.request_count = 0;
        usage.token_count = 0;
    }

    // Check quota before incrementing (allow the Nth request for quota = N).
    let state = LICENSE_STATE
        .read()
        .map_err(|e| anyhow::anyhow!("license state lock poisoned: {e}"))?;
    if let Some(quota) = state.tier().and_then(|t| t.daily_quota()) {
        if usage.request_count >= quota {
            warn!(
                "[license] quota exceeded: {}/{} requests, tier={:?}",
                usage.request_count,
                quota,
                state.tier()
            );
            return Err(anyhow::anyhow!(
                "每日配额已用尽（{} / {} 条），请升级许可证或等待次日重置",
                usage.request_count,
                quota
            ));
        }
    }

    usage.request_count += 1;
    usage.token_count += tokens;

    // Flush to SQLite.
    with_db(|conn| persist_daily_usage(conn, &usage))?;

    debug!(
        "[license] usage recorded: {}/{} requests (quota {}), {} tokens",
        usage.request_count,
        state
            .tier()
            .and_then(|t| t.daily_quota())
            .map_or("unlimited".into(), |q| q.to_string()),
        state.tier().and_then(|t| t.daily_quota()).unwrap_or(0),
        usage.token_count,
    );

    Ok(())
}

/// Get current license info for display in settings UI.
///
/// Returns both the current [`LicenseState`] and the [`DailyUsage`] counters
/// for today. This is the primary data source for the license settings panel.
pub fn get_license_info() -> LicenseInfo {
    if let Err(e) = init_caches() {
        error!("[license] failed to init caches during get_license_info: {e}");
    }
    LicenseInfo {
        state: LICENSE_STATE
            .read()
            .map(|g| g.clone())
            .unwrap_or(LicenseState::Unlicensed),
        daily_usage: DAILY_USAGE.read().map(|g| g.clone()).unwrap_or_default(),
    }
}

/// Clear the license (for logout / reset).
///
/// Resets the in-memory state to `Unlicensed` and removes all persisted
/// state, usage, and offline cache entries from the SQLite database.
/// After calling this, the caller must call `activate_license` again
/// before the app can be used.
pub fn clear_license() {
    info!("[license] clearing all license state");

    // Reset in-memory state.
    if let Ok(mut state) = LICENSE_STATE.write() {
        *state = LicenseState::Unlicensed;
    }
    if let Ok(mut usage) = DAILY_USAGE.write() {
        usage.date = String::new();
        usage.request_count = 0;
        usage.token_count = 0;
    }

    // Clear the database.
    if let Err(e) = with_db(|conn| {
        meta_delete(conn, "license_state")?;
        meta_delete(conn, "last_server_check")?;
        meta_delete(conn, "offline_response")?;
        meta_delete(conn, "offline_signature")?;
        conn.execute("DELETE FROM daily_usage", [])?;
        conn.execute("DELETE FROM activation_failures", [])?;
        Ok::<_, anyhow::Error>(())
    }) {
        error!("[license] failed to clear database: {e}");
    }

    info!("[license] license cleared");
}

/// Snapshot of the current license state and usage for the settings UI.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct LicenseInfo {
    pub state: LicenseState,
    pub daily_usage: DailyUsage,
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Attempt to activate the license against the remote server.
///
/// Sends a POST request to `{api_url}/api/v1/license/activate` with the
/// activation request body. Times out after 15 seconds.
async fn activate_remote(
    req: &LicenseActivationRequest,
    api_url: &str,
) -> Result<LicenseActivationResponse> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .context("failed to build HTTP client for license activation")?;

    let resp = client
        .post(format!(
            "{}/api/v1/license/activate",
            api_url.trim_end_matches('/')
        ))
        .json(req)
        .send()
        .await
        .context("无法连接许可证服务器：连接超时或网络不可用")?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        let msg = if body.is_empty() {
            format!("许可证激活失败 (HTTP {status})")
        } else {
            format!("许可证激活失败 (HTTP {status}): {body}")
        };
        return Err(anyhow::anyhow!(msg));
    }

    let activation: LicenseActivationResponse = resp
        .json()
        .await
        .context("解析许可证服务器响应失败：响应格式异常")?;

    Ok(activation)
}

/// Offline trial activation fallback when neither remote nor cached offline
/// session is available.
///
/// This grants a 7-day trial period starting from the current time. The trial
/// state is persisted to SQLite so it survives restarts.
fn activate_offline_trial(license_key: &str) -> Result<LicenseActivationResponse> {
    let key_hash = hex::encode(Sha256::digest(license_key.as_bytes()));
    let state = LicenseState::Trial {
        started_at: Utc::now(),
        days_remaining: 7,
    };

    // Persist the trial state.
    if let Err(e) = with_db(|conn| persist_license_state(conn, &state)) {
        warn!("[license] failed to persist trial state: {e}");
    }

    info!(
        "[license] offline trial activated (7 days, key_hash={})",
        &key_hash[..8]
    );

    Ok(LicenseActivationResponse {
        state,
        message: "离线激活成功（7 天试用模式，请尽快联网验证完整许可证）".into(),
        server_signature: Some(key_hash),
    })
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: create a temporary in-memory database for testing.
    fn test_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS meta (
                key   TEXT PRIMARY KEY,
                value TEXT NOT NULL
            ) STRICT;
            CREATE TABLE IF NOT EXISTS daily_usage (
                date          TEXT    PRIMARY KEY,
                request_count INTEGER NOT NULL DEFAULT 0,
                token_count   INTEGER NOT NULL DEFAULT 0
            ) STRICT;
            CREATE TABLE IF NOT EXISTS activation_failures (
                date       TEXT    PRIMARY KEY,
                fail_count INTEGER NOT NULL DEFAULT 0
            ) STRICT;",
        )
        .unwrap();
        conn
    }

    #[test]
    fn test_today_iso_format() {
        let s = today_iso();
        assert_eq!(s.len(), 10, "ISO date should be YYYY-MM-DD");
        assert!(s.contains('-'), "ISO date should contain dashes");
    }

    #[test]
    fn test_fingerprint_device_returns_valid_hash() {
        let fp = fingerprint_device();
        assert!(!fp.hostname.is_empty(), "hostname should not be empty");
        assert_eq!(fp.hardware_hash.len(), 64, "SHA-256 hex should be 64 chars");
    }

    #[test]
    fn test_verify_ed25519_placeholder_key_returns_false() {
        // With the default zeroed key, verification must always fail.
        let message = b"test message";
        let signature = [0u8; 64];
        assert!(!verify_ed25519_signature(message, &signature));
    }

    #[test]
    fn test_meta_roundtrip() {
        let conn = test_db();
        meta_set(&conn, "test_key", "hello").unwrap();
        assert_eq!(meta_get(&conn, "test_key").unwrap(), Some("hello".into()));
        assert_eq!(meta_get(&conn, "nonexistent").unwrap(), None);
    }

    #[test]
    fn test_persist_and_load_license_state() {
        let conn = test_db();
        let state = LicenseState::Active {
            tier: LicenseTier::Personal,
            activated_at: Utc::now(),
        };
        persist_license_state(&conn, &state).unwrap();
        let loaded = load_license_state(&conn).unwrap();
        assert_eq!(loaded, state);
        assert!(loaded.is_active());
        assert_eq!(loaded.tier(), Some(LicenseTier::Personal));
    }

    #[test]
    fn test_load_empty_state_returns_unlicensed() {
        let conn = test_db();
        let state = load_license_state(&conn).unwrap();
        assert_eq!(state, LicenseState::Unlicensed);
        assert!(!state.is_active());
    }

    #[test]
    fn test_daily_usage_persist_and_load() {
        let conn = test_db();
        let usage = DailyUsage {
            date: "2026-05-16".into(),
            request_count: 10,
            token_count: 5000,
        };
        persist_daily_usage(&conn, &usage).unwrap();
        let loaded = load_daily_usage(&conn).unwrap();
        assert_eq!(loaded.request_count, 10);
        assert_eq!(loaded.token_count, 5000);
    }

    #[test]
    fn test_rate_limit_check_passes_when_below_threshold() {
        let conn = test_db();
        // No failures recorded yet — should pass.
        check_activation_rate_limit(&conn).unwrap();
    }

    #[test]
    fn test_rate_limit_rejects_when_at_maximum() {
        let conn = test_db();
        let today = today_iso();
        conn.execute(
            "INSERT INTO activation_failures (date, fail_count) VALUES (?1, ?2)",
            params![&today, MAX_FAILED_ACTIVATIONS_PER_DAY],
        )
        .unwrap();
        let err = check_activation_rate_limit(&conn).unwrap_err();
        assert!(err.to_string().contains("24"));
    }

    #[test]
    fn test_record_activation_failure_increments() {
        let conn = test_db();
        record_activation_failure(&conn).unwrap();

        let count: u32 = conn
            .query_row(
                "SELECT fail_count FROM activation_failures WHERE date = ?1",
                params![today_iso()],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);

        record_activation_failure(&conn).unwrap();
        let count: u32 = conn
            .query_row(
                "SELECT fail_count FROM activation_failures WHERE date = ?1",
                params![today_iso()],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 2);
    }

    #[test]
    fn test_reset_activation_failures_clears_today() {
        let conn = test_db();
        let today = today_iso();
        conn.execute(
            "INSERT INTO activation_failures (date, fail_count) VALUES (?1, 3)",
            params![&today],
        )
        .unwrap();
        reset_activation_failures(&conn).unwrap();
        let count: u32 = conn
            .query_row(
                "SELECT COALESCE(SUM(fail_count), 0) FROM activation_failures WHERE date = ?1",
                params![&today],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn test_safemode_no_transition_when_check_fresh() {
        let conn = test_db();
        let now = Utc::now();
        meta_set(&conn, "last_server_check", &now.to_rfc3339()).unwrap();

        let active = LicenseState::Active {
            tier: LicenseTier::Personal,
            activated_at: now,
        };
        let result = apply_safemode_transition(&conn, active.clone()).unwrap();
        assert_eq!(result, active, "should not transition with fresh check");
    }

    #[test]
    fn test_safemode_transitions_to_fallback_grace() {
        let conn = test_db();
        // Simulate last successful check >72h ago.
        let long_ago = Utc::now() - Duration::hours(SAFEMODE_GRACE_HOURS + 1);
        meta_set(&conn, "last_server_check", &long_ago.to_rfc3339()).unwrap();

        let active = LicenseState::Active {
            tier: LicenseTier::Team,
            activated_at: Utc::now(),
        };
        let result = apply_safemode_transition(&conn, active).unwrap();
        match &result {
            LicenseState::FallbackGrace { original_tier, .. } => {
                assert_eq!(*original_tier, LicenseTier::Team);
                assert!(result.is_active(), "FallbackGrace should report as active");
            }
            other => panic!("expected FallbackGrace, got {other:?}"),
        }
    }

    #[test]
    fn test_safemode_does_not_downgrade_unlicensed() {
        let conn = test_db();
        let long_ago = Utc::now() - Duration::hours(SAFEMODE_GRACE_HOURS + 10);
        meta_set(&conn, "last_server_check", &long_ago.to_rfc3339()).unwrap();

        let result = apply_safemode_transition(&conn, LicenseState::Unlicensed).unwrap();
        assert_eq!(result, LicenseState::Unlicensed);
    }

    #[test]
    fn test_license_state_is_active() {
        let now = Utc::now();
        assert!(LicenseState::Trial {
            started_at: now,
            days_remaining: 7,
        }
        .is_active());
        assert!(LicenseState::Active {
            tier: LicenseTier::Personal,
            activated_at: now,
        }
        .is_active());
        assert!(LicenseState::FallbackGrace {
            original_tier: LicenseTier::Personal,
            entered_at: now,
            reason: "test".into(),
        }
        .is_active());
        assert!(!LicenseState::Unlicensed.is_active());
        assert!(!LicenseState::Expired {
            previous_tier: LicenseTier::Personal,
            expired_at: now,
        }
        .is_active());
        assert!(!LicenseState::Revoked {
            reason: "test".into(),
            revoked_at: now,
        }
        .is_active());
    }

    #[test]
    fn test_license_state_tier() {
        let now = Utc::now();
        assert_eq!(
            LicenseState::Trial {
                started_at: now,
                days_remaining: 7,
            }
            .tier(),
            Some(LicenseTier::Trial)
        );
        assert_eq!(
            LicenseState::Active {
                tier: LicenseTier::Enterprise,
                activated_at: now,
            }
            .tier(),
            Some(LicenseTier::Enterprise)
        );
        assert_eq!(
            LicenseState::FallbackGrace {
                original_tier: LicenseTier::Team,
                entered_at: now,
                reason: "test".into(),
            }
            .tier(),
            Some(LicenseTier::Team)
        );
        assert_eq!(LicenseState::Unlicensed.tier(), None);
        assert_eq!(
            LicenseState::Revoked {
                reason: "test".into(),
                revoked_at: now,
            }
            .tier(),
            None
        );
    }

    #[test]
    fn test_build_offline_message_is_deterministic() {
        let fp = fingerprint_device();
        let msg1 = build_offline_message("AAAA-BBBB-CCCC-DDDD", &fp);
        let msg2 = build_offline_message("AAAA-BBBB-CCCC-DDDD", &fp);
        assert_eq!(msg1, msg2, "offline message must be deterministic");
        assert_eq!(msg1.len(), 32, "SHA-256 output must be 32 bytes");
    }

    #[test]
    fn test_activate_offline_trial_creates_valid_state() {
        let resp = activate_offline_trial("TEST-KEY-1234-5678").unwrap();
        assert!(resp.state.is_active());
        assert_eq!(resp.state.tier(), Some(LicenseTier::Trial));
        assert!(resp.server_signature.is_some());
    }
}
