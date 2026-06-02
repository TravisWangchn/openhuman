// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OpenHuman-ZN Contributors
//! 火山引擎（豆包）语音识别 ASR — v3 录音文件识别大模型（AUC submit/query）

use crate::openhuman::config::Config;
use crate::rpc::RpcOutcome;
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use log::{debug, info};
use serde::{Deserialize, Serialize};

const LOG_PREFIX: &str = "[doubao_stt]";
const DOUBAO_ASR_SUBMIT: &str = "https://openspeech.bytedance.com/api/v3/auc/bigmodel/submit";
const DOUBAO_ASR_QUERY: &str = "https://openspeech.bytedance.com/api/v3/auc/bigmodel/query";
const POLL_MAX_ATTEMPTS: u32 = 30;
const POLL_INTERVAL_MS: u64 = 500;

#[derive(Debug, Clone)]
pub struct DoubaoSttOptions {
    pub app_id: Option<String>,
    pub access_token: Option<String>,
    pub audio_format: Option<String>,
    pub language: Option<String>,
}
impl Default for DoubaoSttOptions {
    fn default() -> Self {
        Self {
            app_id: None,
            access_token: None,
            audio_format: None,
            language: Some("zh-CN".into()),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DoubaoSttResult {
    pub text: String,
}

pub async fn transcribe_doubao(
    config: &Config,
    audio_base64: &str,
    opts: &DoubaoSttOptions,
) -> Result<RpcOutcome<DoubaoSttResult>, String> {
    let trimmed = audio_base64.trim();
    if trimmed.is_empty() {
        return Err("audio_base64 is required".into());
    }
    BASE64.decode(trimmed).map_err(|e| format!("base64: {e}"))?;

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
    let af = opts.audio_format.as_deref().unwrap_or("wav");

    // Try reqwest first; fall back to curl subprocess on 401
    match transcribe_doubao_reqwest(app_id, access_token, trimmed, af).await {
        Ok(text) => {
            return Ok(RpcOutcome::single_log(
                DoubaoSttResult { text },
                "doubao-stt",
            ));
        }
        Err(e) if e.contains("401") || e.contains("45000010") || e.contains("grant") => {
            info!("{LOG_PREFIX} reqwest failed with 401, falling back to curl: {e}");
        }
        Err(e) => return Err(e),
    }

    // Fallback: use curl subprocess
    let text = transcribe_doubao_curl(app_id, access_token, trimmed, af).await?;
    Ok(RpcOutcome::single_log(
        DoubaoSttResult { text },
        "doubao-stt-curl",
    ))
}

async fn transcribe_doubao_reqwest(
    app_id: &str,
    access_token: &str,
    trimmed: &str,
    af: &str,
) -> Result<String, String> {
    let task_id = uuid::Uuid::new_v4().to_string();
    info!(
        "{LOG_PREFIX} reqwest submit: app_id={app_id} resource_id=volc.bigasr.auc fmt={af} b64_len={}",
        trimmed.len()
    );

    let client = reqwest::Client::builder()
        .http1_only()
        .no_proxy()
        .user_agent("curl/8.0")
        .build()
        .map_err(|e| format!("reqwest client: {e}"))?;

    let submit_body_str = format!(
        r#"{{"user":{{"uid":"openhuman-zn"}},"audio":{{"format":"{}","data":"{}"}}}}"#,
        af, trimmed
    );

    let submit_resp = client
        .post(DOUBAO_ASR_SUBMIT)
        .header("X-Api-App-Key", app_id)
        .header("X-Api-Access-Key", access_token)
        .header("X-Api-Resource-Id", "volc.bigasr.auc")
        .header("X-Api-Request-Id", &task_id)
        .header("X-Api-Sequence", "-1")
        .header("Content-Type", "application/json")
        .header("Accept", "application/json")
        .header("Connection", "keep-alive")
        .body(submit_body_str)
        .send()
        .await
        .map_err(|e| format!("火山 ASR submit: {e}"))?;

    let submit_status = submit_resp.status();
    if !submit_status.is_success() {
        let body_text = submit_resp.text().await.unwrap_or_default();
        info!(
            "{LOG_PREFIX} reqwest submit FAILED: status={} task_id={task_id} body={body_text}",
            submit_status.as_u16()
        );
        let preview = &body_text[..body_text.len().min(500)];
        return Err(format!(
            "火山 ASR submit (status {}): {preview}",
            submit_status.as_u16()
        ));
    }

    debug!("{LOG_PREFIX} reqwest task_id={task_id}, polling...");

    for attempt in 1..=POLL_MAX_ATTEMPTS {
        tokio::time::sleep(std::time::Duration::from_millis(POLL_INTERVAL_MS)).await;

        let q_resp = client
            .post(DOUBAO_ASR_QUERY)
            .header("X-Api-App-Key", app_id)
            .header("X-Api-Access-Key", access_token)
            .header("X-Api-Resource-Id", "volc.bigasr.auc")
            .header("X-Api-Request-Id", &task_id)
            .header("Content-Type", "application/json")
            .header("Accept", "application/json")
            .header("Connection", "keep-alive")
            .body("{}")
            .send()
            .await
            .map_err(|e| format!("火山 ASR query: {e}"))?;

        let q_status = q_resp.status();
        let q_text = q_resp
            .text()
            .await
            .map_err(|e| format!("query body read: {e}"))?;

        if !q_status.is_success() {
            let preview = &q_text[..q_text.len().min(500)];
            return Err(format!(
                "火山 ASR query (status {}): {preview}",
                q_status.as_u16()
            ));
        }

        if q_text.is_empty() || q_text.trim() == "{}" {
            debug!("{LOG_PREFIX} reqwest poll {attempt}/{POLL_MAX_ATTEMPTS}: still processing...");
            continue;
        }

        let q_json: serde_json::Value = serde_json::from_str(&q_text)
            .map_err(|e| format!("query parse: {e} — {}", &q_text[..q_text.len().min(300)]))?;

        if let Some(text) = q_json["result"]["text"].as_str() {
            let t = text.trim();
            if !t.is_empty() {
                debug!("{LOG_PREFIX} reqwest => {t}");
                return Ok(t.into());
            }
        }

        debug!("{LOG_PREFIX} reqwest poll {attempt}/{POLL_MAX_ATTEMPTS}: no text yet");
    }

    Err(format!(
        "火山 ASR reqwest: 轮询超时 ({} attempts)",
        POLL_MAX_ATTEMPTS
    ))
}

async fn transcribe_doubao_curl(
    app_id: &str,
    access_token: &str,
    audio_base64: &str,
    audio_format: &str,
) -> Result<String, String> {
    let task_id = uuid::Uuid::new_v4().to_string();
    info!("{LOG_PREFIX} curl submit: app_id={app_id} resource_id=volc.bigasr.auc");

    // Log DNS resolution to help diagnose environment-specific API routing
    let host = "openspeech.bytedance.com:443";
    match std::net::ToSocketAddrs::to_socket_addrs(host) {
        Ok(addrs) => {
            let ips: Vec<_> = addrs.map(|a| a.ip().to_string()).collect();
            info!("{LOG_PREFIX} DNS resolve: {host} -> {:?}", ips);
        }
        Err(e) => info!("{LOG_PREFIX} DNS resolve failed: {e}"),
    }

    // Write submit body to temp file to avoid command-line length limits
    let submit_body = format!(
        r#"{{"user":{{"uid":"openhuman-zn"}},"audio":{{"format":"{}","data":"{}"}}}}"#,
        audio_format, audio_base64
    );
    let mut tmp_path = std::env::temp_dir();
    tmp_path.push(format!("doubao_submit_{task_id}.json"));
    std::fs::write(&tmp_path, &submit_body).map_err(|e| format!("temp file: {e}"))?;

    // Preserve PATH so curl can be found, but clear everything else to
    // avoid inherited env vars (proxy, SSL, etc.) breaking the request.
    let path_val = std::env::var("PATH").unwrap_or_default();
    let system_root = std::env::var("SystemRoot").unwrap_or_else(|_| "C:\\Windows".into());

    let curl_result = tokio::task::spawn_blocking({
        let app_id = app_id.to_string();
        let access_token = access_token.to_string();
        let task_id = task_id.clone();
        let tmp_path = tmp_path.clone();
        let path_val = path_val.clone();
        let system_root = system_root.clone();
        move || -> Result<String, String> {
            // Log which curl binary we're using
            let curl_ver = std::process::Command::new("curl")
                .env_clear()
                .env("PATH", &path_val)
                .env("SystemRoot", &system_root)
                .arg("--version")
                .output()
                .map(|o| {
                    String::from_utf8_lossy(&o.stdout)
                        .trim()
                        .lines()
                        .next()
                        .unwrap_or("unknown")
                        .to_string()
                })
                .unwrap_or_else(|e| format!("curl --version failed: {e}"));
            info!("{LOG_PREFIX} curl binary: {curl_ver}");

            // Submit
            let output = std::process::Command::new("curl")
                .env_clear()
                .env("PATH", &path_val)
                .env("SystemRoot", &system_root)
                .args([
                    "--noproxy",
                    "*",
                    "-v",
                    "--ssl-no-revoke",
                    "-X",
                    "POST",
                    DOUBAO_ASR_SUBMIT,
                    "-H",
                    &format!("X-Api-App-Key: {app_id}"),
                    "-H",
                    &format!("X-Api-Access-Key: {access_token}"),
                    "-H",
                    "X-Api-Resource-Id: volc.bigasr.auc",
                    "-H",
                    &format!("X-Api-Request-Id: {task_id}"),
                    "-H",
                    "X-Api-Sequence: -1",
                    "-H",
                    "Content-Type: application/json",
                    "-d",
                    &format!("@{}", tmp_path.display()),
                ])
                .output()
                .map_err(|e| format!("curl exec (is curl on PATH?): {e}"))?;

            let _ = std::fs::remove_file(&tmp_path);

            if !output.status.success() {
                return Err(format!(
                    "curl submit failed: exit={} stdout={} stderr={}",
                    output.status.code().unwrap_or(-1),
                    String::from_utf8_lossy(&output.stdout).trim(),
                    String::from_utf8_lossy(&output.stderr).trim()
                ));
            }

            let stdout = String::from_utf8_lossy(&output.stdout);
            let resp: serde_json::Value = serde_json::from_str(&stdout).map_err(|e| {
                format!(
                    "curl submit parse: {e} — {}",
                    &stdout[..stdout.len().min(300)]
                )
            })?;

            if let Some(code) = resp["header"]["code"].as_u64() {
                if code != 0 {
                    let msg = resp["header"]["message"].as_str().unwrap_or("unknown");
                    return Err(format!(
                        "curl submit error {code}: {msg} | curl_stderr={}",
                        String::from_utf8_lossy(&output.stderr).trim()
                    ));
                }
            }

            // Poll
            for attempt in 1..=POLL_MAX_ATTEMPTS {
                std::thread::sleep(std::time::Duration::from_millis(POLL_INTERVAL_MS));

                let output = std::process::Command::new("curl")
                    .env_clear()
                    .env("PATH", &path_val)
                    .env("SystemRoot", &system_root)
                    .args([
                        "--noproxy",
                        "*",
                        "-v",
                        "--ssl-no-revoke",
                        "-X",
                        "POST",
                        DOUBAO_ASR_QUERY,
                        "-H",
                        &format!("X-Api-App-Key: {app_id}"),
                        "-H",
                        &format!("X-Api-Access-Key: {access_token}"),
                        "-H",
                        "X-Api-Resource-Id: volc.bigasr.auc",
                        "-H",
                        &format!("X-Api-Request-Id: {task_id}"),
                        "-H",
                        "Content-Type: application/json",
                        "-d",
                        "{}",
                    ])
                    .output()
                    .map_err(|e| format!("curl query exec: {e}"))?;

                if !output.status.success() {
                    return Err(format!(
                        "curl query failed: exit={} stdout={} stderr={}",
                        output.status.code().unwrap_or(-1),
                        String::from_utf8_lossy(&output.stdout).trim(),
                        String::from_utf8_lossy(&output.stderr).trim()
                    ));
                }

                let stdout = String::from_utf8_lossy(&output.stdout);
                if stdout.trim().is_empty() || stdout.trim() == "{}" {
                    continue;
                }

                let q_json: serde_json::Value =
                    serde_json::from_str(&stdout).map_err(|e| format!("curl query parse: {e}"))?;

                if let Some(text) = q_json["result"]["text"].as_str() {
                    let t = text.trim();
                    if !t.is_empty() {
                        return Ok(t.to_string());
                    }
                }
            }

            Err(format!("curl poll timeout ({POLL_MAX_ATTEMPTS} attempts)"))
        }
    })
    .await
    .map_err(|e| format!("curl task join: {e}"))?;

    curl_result
}
