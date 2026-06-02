// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OpenHuman-ZN Contributors

//! GATE-2: API key validation.

use super::{
    make_result, GateCheckResult, GateCode, GateErrorCode, GateSeverity, ENV_DEEPSEEK_KEY,
    ENV_DOUBAO_KEY, MIN_API_KEY_LENGTH,
};
use crate::openhuman::config::china_models::ChinaModelsConfig;

/// GATE-2: Checks that at least one API key source is configured:
/// - Env vars: `DEEPSEEK_API_KEY` or `DOUBAO_API_KEY`
/// - Config: `[china_models]` section in config.toml
///
/// At least one valid key is **Critical** — without it the core cannot
/// perform any model inference.
pub fn check_api_keys(china_models: Option<&ChinaModelsConfig>) -> GateCheckResult {
    let name = "GATE-2 API密钥".to_string();
    let gate = Some(GateCode::Gate2ApiKey);

    let key_valid = |var: &str| -> bool {
        std::env::var(var)
            .ok()
            .map_or(false, |k| k.len() >= MIN_API_KEY_LENGTH)
    };

    let has_ds = key_valid(ENV_DEEPSEEK_KEY);
    let has_db = key_valid(ENV_DOUBAO_KEY);

    let has_cn_key = china_models.map_or(false, |cm| {
        cm.deepseek.api_key.as_ref().map_or(false, |k| {
            k.len() >= MIN_API_KEY_LENGTH && cm.deepseek.enabled
        }) || cm.doubao.api_key.as_ref().map_or(false, |k| {
            k.len() >= MIN_API_KEY_LENGTH && cm.doubao.enabled
        }) || cm
            .qwen
            .api_key
            .as_ref()
            .map_or(false, |k| k.len() >= MIN_API_KEY_LENGTH && cm.qwen.enabled)
            || cm.moonshot.api_key.as_ref().map_or(false, |k| {
                k.len() >= MIN_API_KEY_LENGTH && cm.moonshot.enabled
            })
    });

    if has_ds || has_db || has_cn_key {
        let mut parts: Vec<&str> = Vec::new();
        if has_ds {
            parts.push("DEEPSEEK_API_KEY");
        }
        if has_db {
            parts.push("DOUBAO_API_KEY");
        }
        if has_cn_key {
            parts.push("config.toml [china_models]");
        }
        let sources = parts.join(" + ");
        make_result(
            &name,
            true,
            &format!("API密钥已配置 ({sources})"),
            GateSeverity::Critical,
            None,
            gate,
        )
    } else {
        make_result(
            &name,
            false,
            "请设置 DEEPSEEK_API_KEY 或 DOUBAO_API_KEY 环境变量，\
             或在 config.toml 中配置 [china_models] 段 (deepseek/doubao/qwen/moonshot API密钥，长度 ≥ 12)",
            GateSeverity::Critical,
            Some(GateErrorCode::Gate002NoApiKey),
            gate,
        )
    }
}
