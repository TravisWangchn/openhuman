// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OpenHuman-ZN Contributors

use crate::routes::{require_auth_from_headers, success_response};
use axum::{
    extract::Extension,
    http::StatusCode,
    routing::{get, post},
    Router,
};
use std::sync::Arc;

pub fn router() -> Router {
    Router::new()
        .route("/stats", get(referral_stats))
        .route("/claim", post(referral_claim))
}

async fn referral_stats(
    Extension(state): Extension<Arc<crate::server::AppState>>,
    req: axum::extract::Request,
) -> Result<axum::Json<serde_json::Value>, (StatusCode, axum::Json<serde_json::Value>)> {
    let email = require_auth_from_headers(req.headers(), &state).await?;
    Ok(success_response(
        serde_json::json!({"email":email,"referralCode":format!("ZN{}",&email[..email.find('@').unwrap_or(0)].to_uppercase()),"invitedCount":0,"earningsCny":0,"referrals":[]}),
    ))
}

async fn referral_claim(
    Extension(state): Extension<Arc<crate::server::AppState>>,
    req: axum::extract::Request,
) -> Result<axum::Json<serde_json::Value>, (StatusCode, axum::Json<serde_json::Value>)> {
    let _email = require_auth_from_headers(req.headers(), &state).await?;
    let body: serde_json::Value = serde_json::from_slice(
        &axum::body::to_bytes(req.into_body(), 1024 * 1024)
            .await
            .map_err(|e| {
                (
                    StatusCode::BAD_REQUEST,
                    axum::Json(serde_json::json!({"error":format!("{e}")})),
                )
            })?,
    )
    .map_err(|e| {
        (
            StatusCode::BAD_REQUEST,
            axum::Json(serde_json::json!({"error":format!("{e}")})),
        )
    })?;
    Ok(success_response(
        serde_json::json!({"claimed":true,"code":body.get("code").and_then(|v|v.as_str()).unwrap_or(""),"message":"推荐码已绑定"}),
    ))
}
