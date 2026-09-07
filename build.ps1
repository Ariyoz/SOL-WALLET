#!/usr/bin/env pwsh
# build.ps1 — Build the SOL Wallet on Windows
#
# Prerequisites:
#   - Rust (rustup.rs)
#   - Visual Studio C++ Build Tools
#   - Git (for Perl, needed by openssl-src)
#
# Usage: .\build.ps1 [release]

param([string]$Mode = "debug")

# Add Git's Perl to PATH — required for openssl-src vendored build.
$gitPerl = "C:\Program Files\Git\usr\bin"
if (Test-Path "$gitPerl\perl.exe") {
    $env:PATH = "$gitPerl;" + $env:PATH
    Write-Host "Found Perl at $gitPerl" -ForegroundColor Green
} else {
    Write-Warning "Perl not found. OpenSSL vendored build may fail."
    Write-Warning "Install Strawberry Perl: https://strawberryperl.com"
}

# Build
if ($Mode -eq "release") {
    Write-Host "Building release..." -ForegroundColor Cyan
    cargo build --release
} else {
    Write-Host "Building debug..." -ForegroundColor Cyan
    cargo build
}

if ($LASTEXITCODE -eq 0) {
    Write-Host ""
    Write-Host "Build successful!" -ForegroundColor Green
    Write-Host ""
    Write-Host "To run the API server:" -ForegroundColor Cyan
    if ($Mode -eq "release") {
        Write-Host "  .\target\release\solana-wallet-api.exe"
    } else {
        Write-Host "  cargo run --bin solana-wallet-api"
    }
    Write-Host ""
    Write-Host "To open the wallet:" -ForegroundColor Cyan
    Write-Host "  1. cd frontend && .\download-vendors.ps1"
    Write-Host "  2. Open frontend\index.html in your browser"
} else {
    Write-Host "Build FAILED. See errors above." -ForegroundColor Red
}
