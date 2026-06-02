// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OpenHuman-ZN Contributors
//!
//! Payments 路由

use crate::routes::{require_auth_from_headers, success_response};
use axum::{
    extract::{Extension, Json, Path},
    http::StatusCode,
    routing::{get, post},
    Router,
};
use serde::Deserialize;
use std::sync::Arc;

pub fn router() -> Router {
    Router::new()
        .route("/wechat/create", post(wechat_create))
        .route("/alipay/create", post(alipay_create))
        .route("/status/{order_id}", get(payment_status))
        .route("/plans/cn", get(cn_plans))
}

#[derive(Deserialize)]
struct CreatePaymentRequest {
    plan: String,
}

async fn wechat_create(
    Extension(state): Extension<Arc<crate::server::AppState>>,
    req: axum::extract::Request,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    let headers = req.headers().clone();
    let email = require_auth_from_headers(&headers, &state).await?;
    let body: CreatePaymentRequest = serde_json::from_slice(
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
    let plan = body.plan.trim().to_lowercase();
    let amount_cny: u32 = match plan.as_str() {
        "trial" => 0,
        "personal" => 199,
        "team" => 499,
        "enterprise" => 0,
        _ => {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error":"无效套餐"})),
            ))
        }
    };
    let order_id = format!(
        "WX{}",
        &uuid::Uuid::new_v4().to_string().replace('-', "")[..24].to_uppercase()
    );
    state.db().execute("INSERT INTO payment_orders(order_id,plan,gateway,amount_cny,user_email) VALUES(?1,?2,'wechat_pay',?3,?4)", rusqlite::params![order_id,plan,amount_cny,email])
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR,Json(serde_json::json!({"error":format!("DB:{e}")}))))?;
    Ok(success_response(
        serde_json::json!({"orderId":order_id,"plan":plan,"amountCny":amount_cny,"gateway":"wechat_pay","state":"pending","qrCode":format!("wxp://f2f0?orderId={order_id}")}),
    ))
}

async fn alipay_create(
    Extension(state): Extension<Arc<crate::server::AppState>>,
    req: axum::extract::Request,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    let headers = req.headers().clone();
    let email = require_auth_from_headers(&headers, &state).await?;
    let body: CreatePaymentRequest = serde_json::from_slice(
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
    let plan = body.plan.trim().to_lowercase();
    let amount_cny: u32 = match plan.as_str() {
        "trial" => 0,
        "personal" => 199,
        "team" => 499,
        "enterprise" => 0,
        _ => {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error":"无效套餐"})),
            ))
        }
    };
    let order_id = format!(
        "ALI{}",
        &uuid::Uuid::new_v4().to_string().replace('-', "")[..24].to_uppercase()
    );
    state.db().execute("INSERT INTO payment_orders(order_id,plan,gateway,amount_cny,user_email) VALUES(?1,?2,'alipay',?3,?4)", rusqlite::params![order_id,plan,amount_cny,email])
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR,Json(serde_json::json!({"error":format!("DB:{e}")}))))?;
    Ok(success_response(
        serde_json::json!({"orderId":order_id,"plan":plan,"amountCny":amount_cny,"gateway":"alipay","state":"pending","qrCode":format!("https://qr.alipay.com/orderId={order_id}")}),
    ))
}

async fn payment_status(
    Path(order_id): Path<String>,
    Extension(state): Extension<Arc<crate::server::AppState>>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    let order = state.db().query_row("SELECT order_id,plan,gateway,amount_cny,state,user_email,created_at,paid_at,gateway_transaction_id FROM payment_orders WHERE order_id=?1",
        rusqlite::params![order_id],
        |row| Ok(serde_json::json!({"orderId":row.get::<_,String>(0)?,"plan":row.get::<_,String>(1)?,"gateway":row.get::<_,String>(2)?,"amountCny":row.get::<_,i64>(3)?,"state":row.get::<_,String>(4)?,"userEmail":row.get::<_,String>(5)?,"createdAt":row.get::<_,String>(6)?,"paidAt":row.get::<_,Option<String>>(7)?,"gatewayTransactionId":row.get::<_,Option<String>>(8)?})),
    ).map_err(|_| (StatusCode::NOT_FOUND, Json(serde_json::json!({"error":"订单不存在"}))))?;
    Ok(success_response(order))
}

async fn cn_plans() -> Json<serde_json::Value> {
    success_response(serde_json::json!([
        {"name":"trial","displayName":"免费试用","priceCny":0,"duration":"7天","features":["基础AI对话","GATE闸机自检","离线许可证"]},
        {"name":"personal","displayName":"个人版","priceCny":199,"duration":"1年","features":["无限AI对话","飞书集成","微信/支付宝支付","5台设备"]},
        {"name":"team","displayName":"团队版","priceCny":499,"duration":"1年","features":["个人版全部","团队管理","API访问","20台设备"]},
        {"name":"enterprise","displayName":"企业版","priceCny":0,"duration":"按需定制","features":["团队版全部","私有部署","SLA保障","无限设备"]},
    ]))
}
