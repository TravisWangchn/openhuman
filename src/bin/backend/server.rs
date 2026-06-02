// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OpenHuman-ZN Contributors
//! Axum HTTP 服务器 — 国产化 Backend

use crate::routes::{auth, channels, license, payments, referral, teams, webhooks};
use axum::{routing::get, Extension, Router};
use log::info;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};

#[derive(Clone)]
pub struct AppState {
    pub db: Arc<Mutex<rusqlite::Connection>>,
    pub jwt_secret: Arc<str>,
    pub feishu: FeishuConfig,
    pub payments: PaymentConfig,
}

#[derive(Clone, Default)]
pub struct FeishuConfig {
    pub app_id: Option<String>,
    pub app_secret: Option<String>,
}
#[derive(Clone, Default)]
pub struct PaymentConfig {
    pub wechat_mch_id: Option<String>,
    pub wechat_api_v3_key: Option<String>,
    pub alipay_app_id: Option<String>,
    pub alipay_private_key_path: Option<String>,
}

impl AppState {
    /// 获取数据库锁（便捷方法）
    pub fn db(&self) -> std::sync::MutexGuard<rusqlite::Connection> {
        self.db.lock().unwrap()
    }

    #[allow(dead_code)]
    pub async fn new(args: &crate::Args) -> anyhow::Result<Self> {
        if let Some(p) = std::path::Path::new(&args.db_path).parent() {
            std::fs::create_dir_all(p)?;
        }
        let conn = rusqlite::Connection::open(&args.db_path)?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS users(id TEXT PRIMARY KEY,email TEXT UNIQUE NOT NULL,name TEXT,avatar_url TEXT,feishu_open_id TEXT,created_at TEXT NOT NULL DEFAULT(datetime('now')));
             CREATE TABLE IF NOT EXISTS license_activations(license_key_hash TEXT NOT NULL,device_fingerprint TEXT NOT NULL,user_email TEXT NOT NULL,state TEXT NOT NULL,activated_at TEXT NOT NULL DEFAULT(datetime('now')),expires_at TEXT,PRIMARY KEY(license_key_hash,device_fingerprint));
             CREATE TABLE IF NOT EXISTS payment_orders(order_id TEXT PRIMARY KEY,plan TEXT NOT NULL,gateway TEXT NOT NULL,amount_cny INTEGER NOT NULL,state TEXT NOT NULL DEFAULT'pending',user_email TEXT NOT NULL,created_at TEXT NOT NULL DEFAULT(datetime('now')),paid_at TEXT,gateway_transaction_id TEXT);
             CREATE TABLE IF NOT EXISTS channel_connections(id TEXT PRIMARY KEY,user_id TEXT NOT NULL,channel TEXT NOT NULL,provider TEXT NOT NULL,provider_chat_id TEXT,config_json TEXT NOT NULL DEFAULT'{}',created_at TEXT NOT NULL DEFAULT(datetime('now')));
             CREATE INDEX IF NOT EXISTS idx_channel_user ON channel_connections(user_id,channel);",
        )?;
        Ok(Self {
            db: Arc::new(Mutex::new(conn)),
            jwt_secret: Arc::from(args.jwt_secret.as_str()),
            feishu: FeishuConfig {
                app_id: args.feishu_app_id.clone(),
                app_secret: args.feishu_app_secret.clone(),
            },
            payments: PaymentConfig {
                wechat_mch_id: args.wechat_mch_id.clone(),
                wechat_api_v3_key: args.wechat_api_v3_key.clone(),
                alipay_app_id: args.alipay_app_id.clone(),
                alipay_private_key_path: args.alipay_private_key_path.clone(),
            },
        })
    }
}

pub async fn run(state: AppState, bind: &str) -> anyhow::Result<()> {
    let state = Arc::new(state);
    let app = Router::new()
        .route("/", get(root_handler))
        .route("/health", get(health_handler))
        .nest("/auth", auth::router())
        .nest("/payments", payments::router())
        .nest("/api/v1/license", license::router())
        .nest("/channels", channels::router())
        .nest("/referral", referral::router())
        .nest("/teams", teams::router())
        .nest("/webhooks", webhooks::router())
        .layer(Extension(state));
    let addr: SocketAddr = bind.parse()?;
    info!("[backend] 监听 http://{addr}");
    axum::serve(tokio::net::TcpListener::bind(addr).await?, app).await?;
    Ok(())
}

async fn root_handler() -> String {
    "OpenHuman-ZN Backend — 国产化服务运行中".into()
}
async fn health_handler() -> axum::Json<serde_json::Value> {
    axum::Json(
        serde_json::json!({"status":"ok","version":env!("CARGO_PKG_VERSION"),"backend":"openhuman-zn"}),
    )
}
