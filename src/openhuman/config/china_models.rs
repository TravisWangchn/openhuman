// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OpenHuman-ZN Contributors
//! Chinese LLM model identifiers and provider configurations.
//!
//! China-hosted providers that don't require VPN access — critical for
//! OpenHuman-ZN deployments behind the Great Firewall.

/// DeepSeek (deepseek-chat) — primary CN model, best cost-performance.
pub const CN_MODEL_DEEPSEEK_CHAT: &str = "deepseek-chat";
pub const CN_MODEL_DEEPSEEK_REASONER: &str = "deepseek-reasoner";

/// Doubao (ByteDance/Volcengine) — backup via Ark platform.
pub const CN_MODEL_DOUBAO_PRO_32K: &str = "doubao-1.5-pro-32k";
pub const CN_MODEL_DOUBAO_LITE_32K: &str = "doubao-1.5-lite-32k";

/// Tongyi Qianwen (Alibaba) — tertiary CN model.
pub const CN_MODEL_QWEN_TURBO: &str = "qwen-turbo";
pub const CN_MODEL_QWEN_PLUS: &str = "qwen-plus";
pub const CN_MODEL_QWEN_MAX: &str = "qwen-max";

/// Moonshot / Kimi — optional fourth provider.
pub const CN_MODEL_KIMI_MOONSHOT_V1: &str = "moonshot-v1-8k";

/// China-hosted API endpoints (no VPN required).
pub const CN_API_BASE_DEEPSEEK: &str = "https://api.deepseek.com/v1";
pub const CN_API_BASE_DOUBAO: &str = "https://ark.cn-beijing.volces.com/api/v3";
pub const CN_API_BASE_QWEN: &str = "https://dashscope.aliyuncs.com/compatible-mode/v1";
pub const CN_API_BASE_MOONSHOT: &str = "https://api.moonshot.cn/v1";

/// Recommended model routing order for CN deployments.
pub const CN_MODEL_PRIORITY: &[&str] = &[
    CN_MODEL_DEEPSEEK_CHAT,
    CN_MODEL_DOUBAO_PRO_32K,
    CN_MODEL_QWEN_TURBO,
    CN_MODEL_KIMI_MOONSHOT_V1,
];

/// CN provider configuration block.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ChinaProviderConfig {
    pub api_key: Option<String>,
    pub base_url: Option<String>,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_concurrency")]
    pub max_concurrency: u32,
}

fn default_true() -> bool {
    true
}
fn default_concurrency() -> u32 {
    5
}

impl Default for ChinaProviderConfig {
    fn default() -> Self {
        Self {
            api_key: None,
            base_url: None,
            enabled: true,
            max_concurrency: 5,
        }
    }
}

/// Full CN model configuration (in config.toml or env vars).
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct ChinaModelsConfig {
    #[serde(default)]
    pub deepseek: ChinaProviderConfig,
    #[serde(default)]
    pub doubao: ChinaProviderConfig,
    #[serde(default)]
    pub qwen: ChinaProviderConfig,
    #[serde(default)]
    pub moonshot: ChinaProviderConfig,
    #[serde(default = "default_cn_primary")]
    pub primary_model: String,
    #[serde(default = "default_true")]
    pub auto_fallback: bool,
}

fn default_cn_primary() -> String {
    CN_MODEL_DEEPSEEK_CHAT.to_string()
}

impl ChinaModelsConfig {
    pub fn base_url_for(&self, model: &str) -> Option<&str> {
        match model {
            m if m.starts_with("deepseek") => self
                .deepseek
                .base_url
                .as_deref()
                .or(Some(CN_API_BASE_DEEPSEEK)),
            m if m.starts_with("doubao") => {
                self.doubao.base_url.as_deref().or(Some(CN_API_BASE_DOUBAO))
            }
            m if m.starts_with("qwen") => self.qwen.base_url.as_deref().or(Some(CN_API_BASE_QWEN)),
            m if m.starts_with("moonshot") => self
                .moonshot
                .base_url
                .as_deref()
                .or(Some(CN_API_BASE_MOONSHOT)),
            _ => None,
        }
    }

    pub fn api_key_for(&self, model: &str) -> Option<&str> {
        match model {
            m if m.starts_with("deepseek") => self.deepseek.api_key.as_deref(),
            m if m.starts_with("doubao") => self.doubao.api_key.as_deref(),
            m if m.starts_with("qwen") => self.qwen.api_key.as_deref(),
            m if m.starts_with("moonshot") => self.moonshot.api_key.as_deref(),
            _ => None,
        }
    }

    pub fn is_enabled(&self, model: &str) -> bool {
        match model {
            m if m.starts_with("deepseek") => self.deepseek.enabled,
            m if m.starts_with("doubao") => self.doubao.enabled,
            m if m.starts_with("qwen") => self.qwen.enabled,
            m if m.starts_with("moonshot") => self.moonshot.enabled,
            _ => false,
        }
    }
}
