// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OpenHuman-ZN Contributors
//! 火山引擎（豆包）语音合成 TTS

use crate::openhuman::config::Config;
use crate::rpc::RpcOutcome;
use log::{error, info};
use serde::{Deserialize, Serialize};

const LOG_PREFIX: &str = "[doubao_tts]";
const DOUBAO_TTS_URL: &str = "https://openspeech.bytedance.com/api/v1/tts";

fn is_doubao_voice(v: &str) -> bool {
    v.starts_with("BV") && v.ends_with("_streaming")
}

pub const DOUBAO_VOICES: &[(&str, &str)] = &[
    ("BV001_streaming", "通用女声"),
    ("BV002_streaming", "通用男声"),
    ("BV700_streaming", "甜美女声"),
    ("BV701_streaming", "成熟女声"),
];

#[derive(Debug, Clone)]
pub struct DoubaoTtsOptions {
    pub app_id: Option<String>,
    pub access_token: Option<String>,
    pub voice: Option<String>,
    pub speed: Option<f32>,
    pub volume: Option<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DoubaoTtsResult {
    pub audio_base64: String,
    pub audio_mime: String,
}

pub async fn synthesize_doubao(
    config: &Config,
    text: &str,
    opts: &DoubaoTtsOptions,
) -> Result<RpcOutcome<DoubaoTtsResult>, String> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Err("text is required".into());
    }

    let app_id = opts
        .app_id
        .as_deref()
        .or(config.local_ai.doubao_app_id.as_deref())
        .ok_or("火山引擎 App ID 未配置".to_string())?;
    let access_token = opts
        .access_token
        .as_deref()
        .or(config.local_ai.doubao_access_token.as_deref())
        .ok_or("火山引擎 Access Token 未配置".to_string())?;
    let voice = opts
        .voice
        .as_deref()
        .filter(|v| is_doubao_voice(v))
        .unwrap_or("BV001_streaming");

    info!("{LOG_PREFIX} TTS: voice={voice} text_len={}", trimmed.len());

    let body = serde_json::json!({
        "app": {"appid": app_id, "token": access_token, "cluster": "volcano_tts"},
        "user": {"uid": "openhuman-zn"},
        "audio": {"voice_type": voice, "encoding": "mp3",
            "speed_ratio": opts.speed.unwrap_or(1.0), "volume_ratio": opts.volume.unwrap_or(1.0)},
        "request": {"reqid": uuid::Uuid::new_v4().to_string(), "operation": "query", "text": trimmed, "text_type": "plain"},
    });

    let resp = reqwest::Client::builder()
        .no_proxy()
        .build()
        .unwrap_or_default()
        .post(DOUBAO_TTS_URL)
        .header("Authorization", format!("Bearer;{}", access_token))
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("火山 TTS 网络错误: {e}"))?;

    let status = resp.status();
    let resp_body: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| format!("火山 TTS 响应解析失败: {e}"))?;

    if !status.is_success() {
        error!("{LOG_PREFIX} TTS API error status={status} body={resp_body}");
        let msg = resp_body["message"].as_str().unwrap_or("unknown error");
        return Err(format!("火山 TTS API 错误 (HTTP {status}): {msg}"));
    }

    // Volcengine TTS returns code=3000 for success, other codes are errors
    if let Some(code) = resp_body["code"].as_i64() {
        if code != 3000 {
            let msg = resp_body["message"].as_str().unwrap_or("unknown error");
            error!("{LOG_PREFIX} TTS API error code={code} message={msg}");
            return Err(format!("火山 TTS API 错误 (code {code}): {msg}"));
        }
    }

    let audio_b64 = resp_body["data"].as_str().ok_or_else(|| {
        error!("{LOG_PREFIX} TTS response missing data field: {resp_body}");
        "火山 TTS 响应中没有 data 字段".to_string()
    })?;

    info!("{LOG_PREFIX} TTS 完成: {} base64 chars", audio_b64.len());
    Ok(RpcOutcome::single_log(
        DoubaoTtsResult {
            audio_base64: audio_b64.into(),
            audio_mime: "audio/mp3".into(),
        },
        "doubao-tts",
    ))
}
