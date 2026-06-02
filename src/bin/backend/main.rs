// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OpenHuman-ZN Contributors
//!
//! OpenHuman 国产化 Backend Server
//!
//! 部署在阿里云/腾讯云/华为云 ECS 上的 HTTP 服务器，
//! 提供与 core 兼容的 API 表面，集成飞书、微信支付、支付宝等国产服务。

mod routes;
mod server;

use clap::Parser;
use log::info;

/// OpenHuman 国产化 Backend — 飞书生态 + 国产支付 + 许可证服务
#[derive(Parser, Debug)]
#[command(name = "openhuman-backend", version, about)]
struct Args {
    /// 监听地址 (默认 0.0.0.0:3000)
    #[arg(long, default_value = "0.0.0.0:3000")]
    bind: String,

    /// JWT 签名密钥 (也可通过 OPENHUMAN_JWT_SECRET 环境变量设置)
    #[arg(long, env = "OPENHUMAN_JWT_SECRET")]
    jwt_secret: String,

    /// 飞书 App ID
    #[arg(long, env = "FEISHU_APP_ID")]
    feishu_app_id: Option<String>,

    /// 飞书 App Secret
    #[arg(long, env = "FEISHU_APP_SECRET")]
    feishu_app_secret: Option<String>,

    /// 微信支付商户号
    #[arg(long, env = "WECHAT_MCH_ID")]
    wechat_mch_id: Option<String>,

    /// 微信支付 API v3 Key
    #[arg(long, env = "WECHAT_API_V3_KEY")]
    wechat_api_v3_key: Option<String>,

    /// 支付宝 App ID
    #[arg(long, env = "ALIPAY_APP_ID")]
    alipay_app_id: Option<String>,

    /// 支付宝私钥路径
    #[arg(long, env = "ALIPAY_PRIVATE_KEY_PATH")]
    alipay_private_key_path: Option<String>,

    /// SQLite 数据库路径 (默认 ./data/backend.db)
    #[arg(long, default_value = "./data/backend.db")]
    db_path: String,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let args = Args::parse();
    info!("[backend] OpenHuman-ZN Backend 启动中...");

    let state = server::AppState::new(&args).await?;
    server::run(state, &args.bind).await?;

    Ok(())
}
