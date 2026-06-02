// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OpenHuman-ZN Contributors
//!
//! Auth 路由

use crate::routes::{require_auth_from_headers, sign_jwt, success_response, user_response};
use axum::{
    extract::{Extension, Json, Query},
    http::StatusCode,
    routing::{get, post},
    Router,
};
use serde::Deserialize;
use std::sync::Arc;

pub fn router() -> Router {
    Router::new()
        .route("/me", get(auth_me))
        .route("/feishu/login", get(feishu_login_url))
        .route("/feishu/callback", get(feishu_callback))
        .route("/login", post(email_login))
        .route("/register", post(email_register))
}

async fn auth_me(
    Extension(state): Extension<Arc<crate::server::AppState>>,
    req: axum::extract::Request,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    let headers = req.headers().clone();
    let email = require_auth_from_headers(&headers, &state).await?;
    let user = state.db().query_row(
        "SELECT id,email,name,avatar_url,feishu_open_id FROM users WHERE email=?1",
        rusqlite::params![email],
        |row| Ok(serde_json::json!({"id":row.get::<_,String>(0)?,"email":row.get::<_,String>(1)?,"name":row.get::<_,Option<String>>(2)?,"avatarUrl":row.get::<_,Option<String>>(3)?,"feishuOpenId":row.get::<_,Option<String>>(4)?})),
    ).map_err(|_| (StatusCode::UNAUTHORIZED, Json(serde_json::json!({"success":false,"error":"用户不存在"}))))?;
    Ok(user_response(user))
}

#[derive(Deserialize)]
struct LoginQuery {
    redirect_uri: Option<String>,
}

async fn feishu_login_url(
    Query(q): Query<LoginQuery>,
    Extension(state): Extension<Arc<crate::server::AppState>>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    let app_id = state.feishu.app_id.as_deref().ok_or_else(|| {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({"success":false,"error":"飞书未配置"})),
        )
    })?;
    let redirect = q
        .redirect_uri
        .as_deref()
        .unwrap_or("http://localhost:3000/auth/feishu/callback");
    Ok(success_response(
        serde_json::json!({"oauthUrl":format!("https://open.feishu.cn/open-apis/authen/v1/index?app_id={app_id}&redirect_uri={redirect}&state=openhuman-zn")}),
    ))
}

#[derive(Deserialize)]
struct FeishuCallback {
    code: String,
    #[serde(default)]
    state: String,
}

async fn feishu_callback(
    Query(params): Query<FeishuCallback>,
    Extension(state): Extension<Arc<crate::server::AppState>>,
) -> Result<String, (StatusCode, String)> {
    let (id, secret) = match (
        state.feishu.app_id.as_deref(),
        state.feishu.app_secret.as_deref(),
    ) {
        (Some(id), Some(secret)) => (id, secret),
        _ => return Err((StatusCode::SERVICE_UNAVAILABLE, "飞书未配置".into())),
    };
    let auth = base64::Engine::encode(
        &base64::engine::general_purpose::STANDARD,
        format!("{id}:{secret}"),
    );
    let tr: serde_json::Value = reqwest::Client::new()
        .post("https://open.feishu.cn/open-apis/authen/v1/oidc/access_token")
        .json(&serde_json::json!({"grant_type":"authorization_code","code":params.code}))
        .header("Authorization", format!("Bearer {auth}"))
        .send()
        .await
        .map_err(|e| (StatusCode::BAD_GATEWAY, format!("飞书:{e}")))?
        .json()
        .await
        .map_err(|e| (StatusCode::BAD_GATEWAY, format!("解析:{e}")))?;
    let at = tr["data"]["access_token"]
        .as_str()
        .ok_or((StatusCode::UNAUTHORIZED, "token缺失".into()))?;
    let ur: serde_json::Value = reqwest::Client::new()
        .get("https://open.feishu.cn/open-apis/authen/v1/user_info")
        .header("Authorization", format!("Bearer {at}"))
        .send()
        .await
        .map_err(|e| (StatusCode::BAD_GATEWAY, format!("用户:{e}")))?
        .json()
        .await
        .map_err(|e| (StatusCode::BAD_GATEWAY, format!("解析:{e}")))?;
    let d = ur["data"]
        .as_object()
        .ok_or((StatusCode::UNAUTHORIZED, "用户为空".into()))?;
    let name = d.get("name").and_then(|v| v.as_str()).unwrap_or("飞书用户");
    let email = d
        .get("email")
        .or(d.get("mobile"))
        .and_then(|v| v.as_str())
        .unwrap_or("unknown@feishu.cn");
    let av = d.get("avatar_url").and_then(|v| v.as_str());
    let oi = d.get("open_id").and_then(|v| v.as_str());
    state.db().execute("INSERT INTO users(id,email,name,avatar_url,feishu_open_id) VALUES(?1,?2,?3,?4,?5) ON CONFLICT(email) DO UPDATE SET name=excluded.name,avatar_url=excluded.avatar_url,feishu_open_id=excluded.feishu_open_id",
        rusqlite::params![uuid::Uuid::new_v4().to_string(), email, name, av, oi],
    ).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("DB:{e}")))?;
    let jwt =
        sign_jwt(email, &state.jwt_secret).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    Ok(format!("<!DOCTYPE html><html><head><meta charset=\"utf-8\"><title>登录成功</title></head><body><h1>飞书登录成功</h1><p>欢迎，{name}({email})</p><script>localStorage.setItem('openhuman_jwt','{jwt}');window.close()</script></body></html>"))
}

#[derive(Deserialize)]
struct LoginRequest {
    email: String,
    password: Option<String>,
}

async fn email_login(
    Extension(state): Extension<Arc<crate::server::AppState>>,
    Json(body): Json<LoginRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    let email = body.email.trim().to_lowercase();
    if state
        .db()
        .query_row(
            "SELECT COUNT(*)>0 FROM users WHERE email=?1",
            rusqlite::params![email],
            |r| r.get::<_, bool>(0),
        )
        .unwrap_or(false)
        == false
    {
        state
            .db()
            .execute(
                "INSERT INTO users(id,email,name) VALUES(?1,?2,?3)",
                rusqlite::params![
                    uuid::Uuid::new_v4().to_string(),
                    email,
                    email.split('@').next().unwrap_or("用户")
                ],
            )
            .map_err(|e| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({"success":false,"error":format!("DB:{e}")})),
                )
            })?;
    }
    let jwt = sign_jwt(&email, &state.jwt_secret).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"success":false,"error":e})),
        )
    })?;
    Ok(success_response(
        serde_json::json!({"jwtToken":jwt,"email":email}),
    ))
}

async fn email_register(
    Extension(state): Extension<Arc<crate::server::AppState>>,
    Json(body): Json<LoginRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    let email = body.email.trim().to_lowercase();
    if email.is_empty() || !email.contains('@') {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"success":false,"error":"邮箱格式无效"})),
        ));
    }
    state
        .db()
        .execute(
            "INSERT OR IGNORE INTO users(id,email,name) VALUES(?1,?2,?3)",
            rusqlite::params![
                uuid::Uuid::new_v4().to_string(),
                email,
                email.split('@').next().unwrap_or("用户")
            ],
        )
        .map_err(|e| {
            (
                StatusCode::CONFLICT,
                Json(serde_json::json!({"success":false,"error":format!("DB:{e}")})),
            )
        })?;
    let jwt = sign_jwt(&email, &state.jwt_secret).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"success":false,"error":e})),
        )
    })?;
    Ok(success_response(
        serde_json::json!({"jwtToken":jwt,"email":email}),
    ))
}
