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
        .route("/me", get(team_info))
        .route("/me/members", get(team_members))
        .route("/me/invite", post(team_invite))
}

async fn team_info(
    Extension(state): Extension<Arc<crate::server::AppState>>,
    req: axum::extract::Request,
) -> Result<axum::Json<serde_json::Value>, (StatusCode, axum::Json<serde_json::Value>)> {
    let email = require_auth_from_headers(req.headers(), &state).await?;
    Ok(success_response(
        serde_json::json!({"name":format!("{} 的团队",email.split('@').next().unwrap_or("")),"owner":email,"memberCount":1,"plan":"personal","createdAt":chrono::Utc::now().to_rfc3339()}),
    ))
}

async fn team_members(
    Extension(state): Extension<Arc<crate::server::AppState>>,
    req: axum::extract::Request,
) -> Result<axum::Json<serde_json::Value>, (StatusCode, axum::Json<serde_json::Value>)> {
    let email = require_auth_from_headers(req.headers(), &state).await?;
    Ok(success_response(
        serde_json::json!({"members":[{"email":email,"role":"owner"}],"total":1}),
    ))
}

async fn team_invite(
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
        serde_json::json!({"invited":body.get("email").and_then(|v|v.as_str()).unwrap_or(""),"message":"邀请已发送"}),
    ))
}
