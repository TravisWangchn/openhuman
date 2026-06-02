// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OpenHuman-ZN Contributors

//! GATE-5: Disk space quota check.

use super::{
    make_result, GateCheckResult, GateCode, GateErrorCode, GateSeverity, ENV_DATA_DIR,
    MIN_DISK_SPACE_BYTES,
};
use log::info;
use std::path::{Path, PathBuf};

/// GATE-5: Checks that storage directories have at least
/// [`MIN_DISK_SPACE_BYTES`] (100 MiB) of free space.
///
/// The following paths are examined (when discoverable):
///
/// * Data directory — from `OPENHUMAN_DATA_DIR` or the binary's parent dir.
/// * Obsidian vault — from `OPENHUMAN_VAULT_DIR` (optional).
pub fn check_disk_quota() -> GateCheckResult {
    let name = "GATE-5 磁盘配额".to_string();
    let gate = Some(GateCode::Gate5DiskQuota);
    let paths = collect_check_paths();

    let mut failures: Vec<String> = Vec::new();

    for (label, path) in &paths {
        match get_available_space(path) {
            Ok(bytes) if bytes >= MIN_DISK_SPACE_BYTES => {
                let mb = bytes as f64 / (1024.0 * 1024.0);
                info!("[闸机] {label} ({path:?}): 剩余 {mb:.1} MiB — 充足");
            }
            Ok(bytes) => {
                let mb = bytes as f64 / (1024.0 * 1024.0);
                failures.push(format!("{label} ({path:?}): 仅剩 {mb:.1} MiB，需 ≥100 MiB"));
            }
            Err(e) => {
                failures.push(format!("{label} ({path:?}): 无法检查磁盘空间 ({e})"));
            }
        }
    }

    if failures.is_empty() {
        make_result(
            &name,
            true,
            &format!("磁盘空间充足 (已检查 {} 个路径)", paths.len()),
            GateSeverity::Warning,
            None,
            gate,
        )
    } else {
        make_result(
            &name,
            false,
            &failures.join("; "),
            GateSeverity::Warning,
            Some(GateErrorCode::Gate005DiskQuota),
            gate,
        )
    }
}

/// Returns `(label, path)` pairs for every storage path that needs disk
/// space checking.
///
/// Resolution order:
/// 1. `OPENHUMAN_DATA_DIR` env var → data directory for SQLite / config.
/// 2. Parent of the current executable (fallback when the env var is unset).
/// 3. `OPENHUMAN_VAULT_DIR` env var → Obsidian vault directory (optional).
fn collect_check_paths() -> Vec<(&'static str, PathBuf)> {
    let mut paths: Vec<(&'static str, PathBuf)> = Vec::with_capacity(2);

    let data_dir = std::env::var(ENV_DATA_DIR)
        .ok()
        .map(PathBuf::from)
        .or_else(|| {
            std::env::current_exe()
                .ok()
                .and_then(|p| p.parent().map(|p| p.to_path_buf()))
        });

    if let Some(ref path) = data_dir {
        paths.push(("数据目录", path.clone()));
    }

    if let Ok(vault) = std::env::var("OPENHUMAN_VAULT_DIR") {
        paths.push(("Obsidian 仓库", PathBuf::from(vault)));
    }

    paths
}

/// Returns the available free space (in bytes) on the filesystem containing
/// `path`.
///
/// Opens a file handle on the given path, then queries the underlying volume
/// for free space via the OS (equivalent to `statvfs` on Unix and
/// `GetDiskFreeSpaceExW` on Windows).
fn get_available_space(path: &Path) -> std::io::Result<u64> {
    fs2::available_space(path).or_else(|_| {
        path.parent()
            .ok_or_else(|| {
                std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    "no parent path available for disk space query",
                )
            })
            .and_then(fs2::available_space)
    })
}
