// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OpenHuman-ZN Contributors
//!
//! API 路由 — 国产化 Backend（Axum 0.8 兼容）

pub mod auth;
pub mod channels;
pub mod license;
pub mod payments;
pub mod referral;
pub mod teams;
pub mod webhooks;

use axum::{http::StatusCode, Extension, Json};
use serde::Serialize;
use serde_json::Value;

/// 从 Extension 中提取 AppState（handler 用 Request 时的内部方法）
pub fn get_state(
    req: &axum::extract::Request,
) -> Result<&crate::server::AppState, (StatusCode, Json<Value>)> {
    req.extensions()
        .get::<Extension<crate::server::AppState>>()
        .map(|ext| &ext.0)
        .ok_or_else(|| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error":"state missing"})),
            )
        })
}

pub async fn require_auth_from_headers(
    headers: &axum::http::HeaderMap,
    state: &crate::server::AppState,
) -> Result<String, (StatusCode, Json<Value>)> {
    let h = headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| {
            (
                StatusCode::UNAUTHORIZED,
                Json(serde_json::json!({"error":"缺少 Authorization 头"})),
            )
        })?;
    let token = h.strip_prefix("Bearer ").ok_or_else(|| {
        (
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({"error":"Authorization 格式需为 Bearer <token>"})),
        )
    })?;
    verify_jwt(token, &state.jwt_secret)
}

fn verify_jwt(token: &str, secret: &str) -> Result<String, (StatusCode, Json<Value>)> {
    use hmac::{Hmac, Mac};
    use sha2::Sha256;
    let parts: Vec<&str> = token.split('.').collect();
    if parts.len() != 3 {
        return Err((
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({"error":"JWT 格式无效"})),
        ));
    }
    type HmacSha256 = Hmac<Sha256>;
    let mut mac = HmacSha256::new_from_slice(secret.as_bytes()).map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error":"internal"})),
        )
    })?;
    mac.update(format!("{}.{}", parts[0], parts[1]).as_bytes());
    let expected = base64::Engine::encode(
        &base64::engine::general_purpose::URL_SAFE_NO_PAD,
        &mac.finalize().into_bytes(),
    );
    if expected != parts[2] {
        return Err((
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({"error":"JWT 签名无效"})),
        ));
    }
    let bytes = base64::Engine::decode(&base64::engine::general_purpose::URL_SAFE_NO_PAD, parts[1])
        .map_err(|_| {
            (
                StatusCode::UNAUTHORIZED,
                Json(serde_json::json!({"error":"decode"})),
            )
        })?;
    let payload: Value = serde_json::from_slice(&bytes).map_err(|_| {
        (
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({"error":"JSON"})),
        )
    })?;
    payload
        .get("email")
        .and_then(|v| v.as_str())
        .map(String::from)
        .ok_or_else(|| {
            (
                StatusCode::UNAUTHORIZED,
                Json(serde_json::json!({"error":"email missing"})),
            )
        })
}

pub fn sign_jwt(email: &str, secret: &str) -> Result<String, String> {
    use hmac::{Hmac, Mac};
    use sha2::Sha256;
    let h = base64::Engine::encode(
        &base64::engine::general_purpose::URL_SAFE_NO_PAD,
        r#"{"alg":"HS256","typ":"JWT"}"#,
    );
    let n = chrono::Utc::now();
    let p = serde_json::json!({"email":email,"iat":n.timestamp(),"exp":(n+chrono::Duration::days(30)).timestamp()});
    let pb = base64::Engine::encode(
        &base64::engine::general_purpose::URL_SAFE_NO_PAD,
        serde_json::to_string(&p).unwrap_or_default(),
    );
    type HmacSha256 = Hmac<Sha256>;
    let mut mac = HmacSha256::new_from_slice(secret.as_bytes()).map_err(|e| format!("HMAC:{e}"))?;
    mac.update(format!("{}.{}", h, pb).as_bytes());
    let s = base64::Engine::encode(
        &base64::engine::general_purpose::URL_SAFE_NO_PAD,
        &mac.finalize().into_bytes(),
    );
    Ok(format!("{}.{}.{}", h, pb, s))
}

pub fn success_response(data: impl Serialize) -> Json<Value> {
    Json(serde_json::json!({"success":true,"data":data}))
}
pub fn user_response(user: impl Serialize) -> Json<Value> {
    Json(serde_json::json!({"success":true,"user":user}))
}
