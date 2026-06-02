// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OpenHuman-ZN Contributors

use crate::routes::{require_auth_from_headers, success_response};
use axum::{
    extract::Extension,
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use std::sync::Arc;

pub fn router() -> Router {
    Router::new()
        .route("/", get(webhook_list))
        .route("/", post(webhook_create))
        .route("/feishu/event", post(feishu_event_callback))
}

async fn webhook_list(
    Extension(state): Extension<Arc<crate::server::AppState>>,
    req: axum::extract::Request,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    let _email = require_auth_from_headers(req.headers(), &state).await?;
    Ok(success_response(
        serde_json::json!({"webhooks":[],"total":0}),
    ))
}

async fn webhook_create(
    Extension(state): Extension<Arc<crate::server::AppState>>,
    req: axum::extract::Request,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    let _email = require_auth_from_headers(req.headers(), &state).await?;
    let body: serde_json::Value = serde_json::from_slice(
        &axum::body::to_bytes(req.into_body(), 1024 * 1024)
            .await
            .map_err(|e| {
                (
                    StatusCode::BAD_REQUEST,
                    Json(serde_json::json!({"error":format!("{e}")})),
                )
            })?,
    )
    .map_err(|e| {
        (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error":format!("{e}")})),
        )
    })?;
    let wid = uuid::Uuid::new_v4().to_string();
    log::info!("[webhooks] 创建: id={wid}");
    Ok(success_response(
        serde_json::json!({"id":wid,"url":body.get("url").and_then(|v|v.as_str()).unwrap_or(""),"createdAt":chrono::Utc::now().to_rfc3339()}),
    ))
}

async fn feishu_event_callback(Json(body): Json<serde_json::Value>) -> Json<serde_json::Value> {
    if let Some(c) = body.get("challenge").and_then(|v| v.as_str()) {
        return Json(serde_json::json!({"challenge":c}));
    }
    Json(serde_json::json!({"code":0}))
}
