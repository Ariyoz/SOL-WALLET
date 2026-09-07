#!/usr/bin/env pwsh
# download-vendors.ps1
# Downloads all required JavaScript vendor libraries.
# Run ONCE before opening index.html.
# Requires PowerShell 5+ and internet access.

param([switch]$Force)

$dir = Join-Path $PSScriptRoot "vendor"
if (-not (Test-Path $dir)) { New-Item -ItemType Directory -Path $dir | Out-Null }

$vendors = [ordered]@{
    "solana-web3.js"   = "https://unpkg.com/@solana/web3.js@1.98.0/lib/index.iife.min.js"
    "nacl-fast.min.js" = "https://unpkg.com/tweetnacl@1.0.3/nacl-fast.min.js"
    "bs58.js"          = "https://unpkg.com/bs58@5.0.0/dist/bs58.umd.js"
    "bip39.js"         = "https://unpkg.com/bip39@3.1.0/dist/index.js"
}

foreach ($file in $vendors.Keys) {
    $dest = Join-Path $dir $file
    $url  = $vendors[$file]

    if ((Test-Path $dest) -and -not $Force) {
        Write-Host "  [skip] $file (already exists — use -Force to re-download)" -ForegroundColor DarkGray
        continue
    }

    Write-Host "  Downloading $file ..." -NoNewline
    try {
        Invoke-WebRequest -Uri $url -OutFile $dest -UseBasicParsing -TimeoutSec 30
        $size = [math]::Round((Get-Item $dest).Length / 1KB, 1)
        Write-Host " OK ($size KB)" -ForegroundColor Green
    } catch {
        Write-Host " FAILED: $_" -ForegroundColor Red
    }
}

# Wrap bip39 so it exposes window.bip39 (the dist/index.js is CommonJS-ish)
$bip39Path = Join-Path $dir "bip39.js"
if (Test-Path $bip39Path) {
    $content = Get-Content $bip39Path -Raw
    # Only wrap if not already wrapped
    if ($content -notmatch "window\.bip39") {
        $wrapped = @"
// Auto-wrapped by download-vendors.ps1 to expose window.bip39
(function(exports){
$content
if (typeof module === 'undefined') { window.bip39 = exports; }
})({});
"@
        Set-Content -Path $bip39Path -Value $wrapped -Encoding UTF8
        Write-Host "  Wrapped bip39.js to expose window.bip39" -ForegroundColor Cyan
    }
}

Write-Host ""
Write-Host "All vendor libraries ready." -ForegroundColor Green
Write-Host ""
Write-Host "Next steps:" -ForegroundColor Cyan
Write-Host "  1. Start the API:    cd ..\  && .\run.ps1"
Write-Host "  2. Open in browser:  frontend\index.html"
Write-Host "  3. Or serve locally: python -m http.server 8080 --directory frontend"
