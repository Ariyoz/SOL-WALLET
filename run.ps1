#!/usr/bin/env pwsh
# run.ps1 — Start the API server in dev mode

$gitPerl = "C:\Program Files\Git\usr\bin"
if (Test-Path "$gitPerl\perl.exe") { $env:PATH = "$gitPerl;" + $env:PATH }

# Load .env
if (Test-Path ".env") {
    Get-Content ".env" | ForEach-Object {
        if ($_ -match "^\s*([^#][^=]+)=(.*)$") {
            [System.Environment]::SetEnvironmentVariable($Matches[1].Trim(), $Matches[2].Trim(), "Process")
        }
    }
    Write-Host "Loaded .env" -ForegroundColor Green
}

Write-Host "Starting Solana Low-Fee Wallet API..." -ForegroundColor Cyan
Write-Host "  Network : $env:SOLANA_NETWORK"
Write-Host "  RPC     : $env:SOLANA_RPC_URL"
Write-Host "  Database: $env:DATABASE_URL"
Write-Host "  API     : http://$env:API_HOST`:$env:API_PORT"
Write-Host ""

cargo run --bin solana-wallet-api
