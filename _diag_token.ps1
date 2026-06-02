# Token diagnostics — checks core health, token file, and tests an authenticated ping
param(
    [string]$CoreUrl = "http://127.0.0.1:7788"
)

Write-Host "=== Core Health Check ===" -ForegroundColor Cyan
try {
    $health = Invoke-RestMethod -Uri "$CoreUrl/health" -Method Get -TimeoutSec 5
    Write-Host "[OK] Core is reachable: $($health | ConvertTo-Json -Compress)" -ForegroundColor Green
} catch {
    Write-Host "[FAIL] Core NOT reachable at $CoreUrl — $($_.Exception.Message)" -ForegroundColor Red
    Write-Host "  -> Is openhuman-core running? Try: cargo run --bin openhuman-core -- serve"
    exit 1
}

Write-Host ""
Write-Host "=== Token Sources ===" -ForegroundColor Cyan

# 1. Env var (Tauri path)
$envToken = $env:OPENHUMAN_CORE_TOKEN
if ($envToken) {
    Write-Host "[ENV]  OPENHUMAN_CORE_TOKEN = $($envToken.Substring(0, [Math]::Min(16,$envToken.Length)))..." -ForegroundColor Yellow
} else {
    Write-Host "[ENV]  OPENHUMAN_CORE_TOKEN = (not set)" -ForegroundColor Gray
}

# 2. Token file (standalone path)
$workspaceDir = if ($env:OPENHUMAN_WORKSPACE) { $env:OPENHUMAN_WORKSPACE } else { "$env:USERPROFILE\.openhuman" }
$stagingDir   = "$env:USERPROFILE\.openhuman-staging"
$tokenPaths = @(
    "$workspaceDir\core.token",
    "$stagingDir\core.token"
)

foreach ($tp in $tokenPaths) {
    if (Test-Path $tp) {
        $ft = Get-Content $tp -Raw
        Write-Host "[FILE] $tp = $($ft.Trim().Substring(0, [Math]::Min(16, $ft.Trim().Length)))..." -ForegroundColor Yellow
    } else {
        Write-Host "[FILE] $tp = (not found)" -ForegroundColor Gray
    }
}

# 3. Vite env
$viteEnvPath = "app\.env.local"
if (Test-Path $viteEnvPath) {
    $viteToken = Select-String -Path $viteEnvPath -Pattern "VITE_OPENHUMAN_CORE_TOKEN" | ForEach-Object { $_.Line }
    if ($viteToken) {
        Write-Host "[VITE] $viteToken" -ForegroundColor Yellow
    } else {
        Write-Host "[VITE] VITE_OPENHUMAN_CORE_TOKEN not found in app/.env.local" -ForegroundColor Gray
    }
} else {
    Write-Host "[VITE] app/.env.local not found" -ForegroundColor Gray
}

Write-Host ""
Write-Host "=== Authenticated Ping Test ===" -ForegroundColor Cyan

# Try with each available token
$tokens = @()
if ($envToken) { $tokens += @{Source="ENV"; Value=$envToken} }
foreach ($tp in $tokenPaths) {
    if (Test-Path $tp) {
        $tokens += @{Source="FILE:$tp"; Value=(Get-Content $tp -Raw).Trim()}
    }
}

if ($tokens.Count -eq 0) {
    Write-Host "[WARN] No token found anywhere. Generate one manually or start the core first." -ForegroundColor Red
} else {
    foreach ($t in $tokens) {
        try {
            $body = @{ jsonrpc = "2.0"; id = 1; method = "core.ping"; params = @{} } | ConvertTo-Json -Compress
            $headers = @{
                "Content-Type" = "application/json"
                "Authorization" = "Bearer $($t.Value)"
            }
            $resp = Invoke-RestMethod -Uri "$CoreUrl/rpc" -Method Post -Body $body -Headers $headers -TimeoutSec 5
            Write-Host "[PASS] Token from $($t.Source) — core.ping OK: $($resp | ConvertTo-Json -Compress)" -ForegroundColor Green
        } catch {
            $statusCode = $_.Exception.Response.StatusCode.value__
            Write-Host "[FAIL] Token from $($t.Source) — HTTP $statusCode : $($_.Exception.Message)" -ForegroundColor Red
        }
    }
}

Write-Host ""
Write-Host "=== Fix Guide ===" -ForegroundColor Cyan
Write-Host "If you're running 'pnpm dev' (Vite-only):"
Write-Host "  1. Start core: cargo run --bin openhuman-core -- serve"
Write-Host "  2. Read token:  type %USERPROFILE%\.openhuman-staging\core.token"
Write-Host "  3. Write to:    app\.env.local  ->  VITE_OPENHUMAN_CORE_TOKEN=<paste-token>"
Write-Host "  4. Restart Vite: pnpm dev"
Write-Host ""
Write-Host "If you're running 'pnpm dev:app' (Tauri):"
Write-Host "  - Token is auto-managed; 401 usually means a stale listener on port 7788."
Write-Host "  - Kill any leftover openhuman-core processes and retry."
Write-Host "  - Or set OPENHUMAN_CORE_REUSE_EXISTING=1 then restart."
