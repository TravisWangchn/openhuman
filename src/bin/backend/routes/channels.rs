// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OpenHuman-ZN Contributors

use crate::routes::{require_auth_from_headers, success_response};
use axum::{
    extract::{Extension, Path},
    http::StatusCode,
    routing::post,
    Router,
};
use std::sync::Arc;

pub fn router() -> Router {
    Router::new().route("/{channel}/messages", post(send_message))
}

async fn send_message(
    Path(channel): Path<String>,
    Extension(state): Extension<Arc<crate::server::AppState>>,
    req: axum::extract::Request,
) -> Result<axum::Json<serde_json::Value>, (StatusCode, axum::Json<serde_json::Value>)> {
    let headers = req.headers().clone();
    let _email = require_auth_from_headers(&headers, &state).await?;
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
    let mid = uuid::Uuid::new_v4().to_string();
    if channel.starts_with("feishu_") || channel == "feishu" {
        let text = body.get("text").and_then(|v| v.as_str()).unwrap_or("");
        if let Err(e) = send_feishu_message(&state, &channel, text).await {
            log::warn!("[channels] 飞书发送失败: {e}");
        }
    }
    Ok(success_response(
        serde_json::json!({"id":mid,"channel":channel,"sentAt":chrono::Utc::now().to_rfc3339()}),
    ))
}

async fn send_feishu_message(
    state: &crate::server::AppState,
    channel: &str,
    text: &str,
) -> Result<(), String> {
    let app_id = state.feishu.app_id.as_deref().ok_or("飞书未配置")?;
    let app_secret = state.feishu.app_secret.as_deref().ok_or("飞书密钥未配置")?;
    let tr: serde_json::Value = reqwest::Client::new()
        .post("https://open.feishu.cn/open-apis/auth/v3/tenant_access_token/internal")
        .json(&serde_json::json!({"app_id":app_id,"app_secret":app_secret}))
        .send()
        .await
        .map_err(|e| format!("token:{e}"))?
        .json()
        .await
        .map_err(|e| format!("parse:{e}"))?;
    let at = tr["tenant_access_token"].as_str().ok_or("token缺失")?;
    let cid = channel.strip_prefix("feishu_").unwrap_or(channel);
    let resp = reqwest::Client::new()
        .post("https://open.feishu.cn/open-apis/im/v1/messages?receive_id_type=chat_id")
        .header("Authorization", format!("Bearer {at}"))
        .json(&serde_json::json!({"receive_id":cid,"msg_type":"text","content":serde_json::json!({"text":text}).to_string()}))
        .send().await.map_err(|e| format!("send:{e}"))?;
    if !resp.status().is_success() {
        return Err(format!(
            "send fail ({}): {}",
            resp.status(),
            resp.text().await.unwrap_or_default()
        ));
    }
    Ok(())
}
