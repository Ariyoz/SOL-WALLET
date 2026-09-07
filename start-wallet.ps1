#!/usr/bin/env pwsh
# start-wallet.ps1
# Builds (if needed) and starts the SOL Wallet API server + opens frontend

Write-Host "◎ SOL Wallet" -ForegroundColor Cyan
Write-Host "============" -ForegroundColor Cyan

# Add Git's Perl to PATH for OpenSSL build
$gitPerl = "C:\Program Files\Git\usr\bin"
if (Test-Path "$gitPerl\perl.exe") { $env:PATH = "$gitPerl;" + $env:PATH }

# Load .env
if (Test-Path ".env") {
    Get-Content ".env" | ForEach-Object {
        if ($_ -match "^\s*([^#][^=]+)=(.*)$") {
            [System.Environment]::SetEnvironmentVariable($Matches[1].Trim(), $Matches[2].Trim(), "Process")
        }
    }
}

$apiPort = if ($env:API_PORT) { $env:API_PORT } else { "3000" }
$network = if ($env:SOLANA_NETWORK) { $env:SOLANA_NETWORK } else { "devnet" }
$rpcUrl  = if ($env:SOLANA_RPC_URL) { $env:SOLANA_RPC_URL } else { "https://api.devnet.solana.com" }

Write-Host ""
Write-Host "Network : $network"  -ForegroundColor Yellow
Write-Host "RPC     : $rpcUrl"
Write-Host "API     : http://127.0.0.1:$apiPort"
Write-Host ""

# Build if binary doesn't exist
$binary = "target\debug\solana-wallet-api.exe"
if (-not (Test-Path $binary)) {
    Write-Host "Building backend (first time takes ~10 min)..." -ForegroundColor Yellow
    cargo build --bin solana-wallet-api
    if ($LASTEXITCODE -ne 0) { Write-Host "Build failed!" -ForegroundColor Red; exit 1 }
}

Write-Host "Starting API server..." -ForegroundColor Green
Write-Host "Open frontend\index.html in your browser" -ForegroundColor Cyan
Write-Host "Press Ctrl+C to stop" -ForegroundColor DarkGray
Write-Host ""

& ".\$binary"
