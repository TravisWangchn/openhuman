use std::time::Duration;

use serde::Serialize;

/// Network reachability classification for an endpoint.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum Reachability {
    /// Endpoint responded within the timeout.
    Reachable,
    /// Endpoint did not respond (timeout or connection refused).
    Unreachable,
    /// Probe was skipped.
    Skipped,
}

/// Aggregated network environment detected at startup.
#[derive(Debug, Clone, Serialize)]
pub struct NetworkEnvironment {
    pub deepseek: Reachability,
    pub doubao: Reachability,
    pub huggingface: Reachability,
    pub china_direct: bool,
    pub hf_available: bool,
}

impl NetworkEnvironment {
    const fn new(deepseek: Reachability, doubao: Reachability, huggingface: Reachability) -> Self {
        let china_direct = matches!(deepseek, Reachability::Reachable)
            || matches!(doubao, Reachability::Reachable);
        let hf_available = matches!(huggingface, Reachability::Reachable);
        Self {
            deepseek,
            doubao,
            huggingface,
            china_direct,
            hf_available,
        }
    }
}

/// Quick reachability probe via GET {url}/models with a short timeout.
async fn probe_endpoint(client: &reqwest::Client, url: &str) -> Reachability {
    match client
        .get(format!("{}/models", url.trim_end_matches('/')))
        .timeout(Duration::from_secs(4))
        .send()
        .await
    {
        Ok(resp) if resp.status().is_success() || resp.status().as_u16() == 401 => {
            Reachability::Reachable
        }
        Ok(resp) => {
            log::debug!("[network-probe] {} returned HTTP {}", url, resp.status());
            Reachability::Reachable
        }
        Err(e) => {
            log::debug!("[network-probe] {} unreachable: {}", url, e);
            Reachability::Unreachable
        }
    }
}

/// Probe China-hosted API endpoints and HuggingFace concurrently.
pub async fn detect_environment() -> NetworkEnvironment {
    let client = reqwest::Client::builder()
        .no_proxy()
        .timeout(Duration::from_secs(5))
        .build()
        .unwrap_or_default();

    let (ds, db, hf) = tokio::join!(
        probe_endpoint(&client, "https://api.deepseek.com/v1"),
        probe_endpoint(&client, "https://ark.cn-beijing.volces.com/api/v3"),
        probe_endpoint(&client, "https://huggingface.co"),
    );

    let env = NetworkEnvironment::new(ds, db, hf);

    log::info!(
        "[network-probe] 环境探测完成: china_direct={} hf_available={}",
        env.china_direct,
        env.hf_available,
    );

    if !env.china_direct {
        log::warn!("[network-probe] 国内API不可达 — DeepSeek和豆包均无法直连,模型推理将不可用");
    }
    if !env.hf_available {
        log::warn!("[network-probe] HuggingFace不可达 — 语音模型下载将不可用");
    }

    env
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn china_direct_when_deepseek_reachable() {
        let env = NetworkEnvironment::new(
            Reachability::Reachable,
            Reachability::Unreachable,
            Reachability::Unreachable,
        );
        assert!(env.china_direct);
        assert!(!env.hf_available);
    }

    #[test]
    fn china_direct_when_doubao_reachable() {
        let env = NetworkEnvironment::new(
            Reachability::Unreachable,
            Reachability::Reachable,
            Reachability::Unreachable,
        );
        assert!(env.china_direct);
    }

    #[test]
    fn all_unreachable() {
        let env = NetworkEnvironment::new(
            Reachability::Unreachable,
            Reachability::Unreachable,
            Reachability::Unreachable,
        );
        assert!(!env.china_direct);
        assert!(!env.hf_available);
    }

    #[test]
    fn all_reachable() {
        let env = NetworkEnvironment::new(
            Reachability::Reachable,
            Reachability::Reachable,
            Reachability::Reachable,
        );
        assert!(env.china_direct);
        assert!(env.hf_available);
    }
}
