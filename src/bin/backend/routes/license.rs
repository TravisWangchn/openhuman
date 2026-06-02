// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OpenHuman-ZN Contributors
//!
//! License 路由 — 国产许可证激活服务器

use axum::{
    extract::{Extension, Json},
    http::StatusCode,
    routing::post,
    Router,
};
use serde::Deserialize;
use std::sync::Arc;

use crate::routes::success_response;

pub fn router() -> Router {
    Router::new().route("/activate", post(license_activate))
}

#[derive(Deserialize)]
struct ActivateRequest {
    license_key: String,
    device: DeviceInfo,
    user_email: String,
}

#[derive(Deserialize)]
struct DeviceInfo {
    cpu_id: Option<String>,
    motherboard_uuid: Option<String>,
    hostname: Option<String>,
}

async fn license_activate(
    Extension(state): Extension<Arc<crate::server::AppState>>,
    Json(body): Json<ActivateRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    use sha2::{Digest, Sha256};

    let key = body.license_key.trim();
    let email = body.user_email.trim().to_lowercase();

    if !key.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') || key.len() < 19 {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(
                serde_json::json!({"success": false, "error": "许可证密钥格式无效 (需为 XXXX-XXXX-XXXX-XXXX)"}),
            ),
        ));
    }

    let key_hash = hex::encode(Sha256::digest(key.as_bytes()));

    let fingerprint = format!(
        "{}:{}:{}",
        body.device.cpu_id.as_deref().unwrap_or("unknown"),
        body.device.motherboard_uuid.as_deref().unwrap_or("unknown"),
        body.device.hostname.as_deref().unwrap_or("unknown"),
    );
    let device_hash = hex::encode(Sha256::digest(fingerprint.as_bytes()));

    let device_count: i64 = state
        .db()
        .query_row(
            "SELECT COUNT(*) FROM license_activations WHERE license_key_hash = ?1",
            rusqlite::params![key_hash],
            |row| row.get(0),
        )
        .unwrap_or(0);

    let max_devices: i64 = 5;
    if device_count >= max_devices {
        let already_activated: bool = state.db().query_row(
            "SELECT COUNT(*) > 0 FROM license_activations WHERE license_key_hash = ?1 AND device_fingerprint = ?2",
            rusqlite::params![key_hash, device_hash],
            |row| row.get(0),
        ).unwrap_or(false);

        if !already_activated {
            return Err((
                StatusCode::FORBIDDEN,
                Json(
                    serde_json::json!({"success": false, "error": format!("设备配额已满 ({device_count}/{max_devices})")}),
                ),
            ));
        }
    }

    let expires_at = chrono::Utc::now() + chrono::Duration::days(365);

    state.db().execute(
        "INSERT OR REPLACE INTO license_activations (license_key_hash, device_fingerprint, user_email, state, expires_at) VALUES (?1, ?2, ?3, 'active', ?4)",
        rusqlite::params![key_hash, device_hash, email, expires_at.to_rfc3339()],
    ).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"success": false, "error": format!("DB: {e}")}))))?;

    log::info!(
        "[license] 激活: email={} key_hash={}.. devices={}/{}",
        email,
        &key_hash[..8],
        device_count + 1,
        max_devices
    );

    Ok(success_response(serde_json::json!({
        "state": "active", "message": "许可证激活成功",
        "serverSignature": key_hash, "expiresAt": expires_at.to_rfc3339(),
        "deviceCount": device_count + 1, "maxDevices": max_devices,
    })))
}
