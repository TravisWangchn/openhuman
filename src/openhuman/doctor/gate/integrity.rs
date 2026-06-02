// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OpenHuman-ZN Contributors

//! GATE-1: Binary file integrity check.

use super::{
    make_result, GateCheckResult, GateCode, GateErrorCode, GateSeverity, ENV_EXPECTED_HASH,
};
use sha2::{Digest, Sha256};

/// GATE-1: Computes the SHA-256 hash of the running `openhuman-core` binary
/// and compares it against the expected value.
///
/// The expected hash is read from the `OPENHUMAN_CORE_EXPECTED_HASH`
/// environment variable. When the variable is unset the check passes with
/// an informational message — integrity verification is optional.
pub fn check_binary_integrity() -> GateCheckResult {
    let name = "GATE-1 文件完整性".to_string();
    let gate = Some(GateCode::Gate1FileIntegrity);

    let expected = match std::env::var(ENV_EXPECTED_HASH) {
        Ok(h) => h,
        Err(_) => {
            return make_result(
                &name,
                true,
                "未配置哈希校验 (设置 OPENHUMAN_CORE_EXPECTED_HASH 可启用)",
                GateSeverity::Info,
                None,
                gate,
            );
        }
    };

    let exe_path = match std::env::current_exe() {
        Ok(p) => p,
        Err(e) => {
            return make_result(
                &name,
                false,
                &format!("无法获取可执行文件路径: {e}"),
                GateSeverity::Critical,
                Some(GateErrorCode::Gate001FileHash),
                gate,
            );
        }
    };

    let data = match std::fs::read(&exe_path) {
        Ok(d) => d,
        Err(e) => {
            return make_result(
                &name,
                false,
                &format!("无法读取可执行文件 ({exe_path:?}): {e}"),
                GateSeverity::Critical,
                Some(GateErrorCode::Gate001FileHash),
                gate,
            );
        }
    };

    let mut hasher = Sha256::new();
    hasher.update(&data);
    let actual_hash = hex::encode(hasher.finalize());

    // Case-insensitive hex comparison.
    if expected.to_uppercase() == actual_hash.to_uppercase() {
        make_result(
            &name,
            true,
            &format!(
                "SHA-256 校验通过: {}...{}",
                &actual_hash[..8],
                &actual_hash[actual_hash.len().saturating_sub(8)..]
            ),
            GateSeverity::Critical,
            None,
            gate,
        )
    } else {
        let trunc = |s: &str| -> String {
            let s = s.trim_end_matches('=');
            if s.len() > 8 {
                format!("{}...", &s[..8])
            } else {
                s.to_string()
            }
        };
        make_result(
            &name,
            false,
            &format!(
                "SHA-256 不匹配 (期望={}, 实际={})",
                trunc(&expected),
                trunc(&actual_hash),
            ),
            GateSeverity::Critical,
            Some(GateErrorCode::Gate001FileHash),
            gate,
        )
    }
}
