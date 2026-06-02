# start_core.ps1 — 启动 openhuman-core，自动同步令牌到 app/.env.local
#
# 逻辑：
#   1. 如果 app/.env.local 中已有 VITE_OPENHUMAN_CORE_TOKEN，将其作为
#      OPENHUMAN_CORE_TOKEN 环境变量传给核心 → 令牌不变，前端无需重启。
#   2. 如果没有预设令牌，核心会自己生成 → 启动后自动回写到 .env.local。
#
# 用法：
#   powershell -ExecutionPolicy Bypass -File start_core.ps1
#   或双击 start_core.bat

$ErrorActionPreference = "Stop"
$repoRoot = Split-Path -Parent $MyInvocation.MyCommand.Path
$envLocal = Join-Path $repoRoot "app\.env.local"
$tokenFile = "$env:USERPROFILE\.openhuman\core.token"

Write-Host "=== OpenHuman Core Launcher ===" -ForegroundColor Cyan

# ── 阶段 1：提取预设令牌 ──────────────────────────────────────────────
$presetToken = $null
if (Test-Path $envLocal) {
    $match = Select-String -Path $envLocal -Pattern '^VITE_OPENHUMAN_CORE_TOKEN=(.+)$'
    if ($match -and $match.Matches.Count -gt 0) {
        $presetToken = $match.Matches[0].Groups[1].Value.Trim()
        if ($presetToken.Length -eq 64) {
            Write-Host "[TOKEN] 使用 .env.local 中的预设令牌: $($presetToken.Substring(0,16))..." -ForegroundColor Green
            $env:OPENHUMAN_CORE_TOKEN = $presetToken
        } else {
            Write-Host "[TOKEN] .env.local 中令牌长度异常 ($($presetToken.Length) 字符)，将让核心自动生成" -ForegroundColor Yellow
            $presetToken = $null
        }
    }
}

if (-not $presetToken) {
    Write-Host "[TOKEN] 未找到预设令牌，核心将自动生成新令牌" -ForegroundColor Yellow
}

# ── 阶段 2：启动核心 ──────────────────────────────────────────────────
Write-Host "[CORE] 正在编译并启动..." -ForegroundColor Cyan
Push-Location $repoRoot
try {
    cargo run --bin openhuman-core -- serve
    if ($LASTEXITCODE -ne 0) {
        Write-Host "[CORE] 启动失败，退出码: $LASTEXITCODE" -ForegroundColor Red
        exit $LASTEXITCODE
    }
} finally {
    Pop-Location
}

Write-Host "[CORE] 核心已退出" -ForegroundColor Gray
