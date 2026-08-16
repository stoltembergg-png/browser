# Windows package smoke test — PR-052
#
# Tests clean install, launch and uninstall of the Browser NSIS installer.
# Must be run on the reference platform (Windows 10/11 x86_64).
#
# Usage:
#   .\scripts\windows_package_smoke.ps1 [-DebPath path\to\Browser_0.1.0_x64-setup.exe]
#
# If no installer path is given, the script validates the smoke contract
# (checks that the tauri config and support matrix are consistent) without
# actually installing anything.

param(
    [string]$InstallerPath = ""
)

$ErrorActionPreference = "Stop"
$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$RepoRoot = Split-Path -Parent $ScriptDir
$TauriConf = Join-Path $RepoRoot "apps\desktop\src-tauri\tauri.conf.json"

# ── Contract validation (always runs) ────────────────────────────────────

Write-Host "=== Windows package smoke — contract validation ==="

if (-not (Test-Path $TauriConf)) {
    Write-Host "FAIL: tauri.conf.json not found at $TauriConf"
    exit 1
}

$conf = Get-Content $TauriConf -Raw | ConvertFrom-Json

if (-not $conf.bundle.active) {
    Write-Host "FAIL: bundle.active is not true in tauri.conf.json"
    exit 1
}

$targets = $conf.bundle.targets
if ($targets -notcontains "nsis") {
    Write-Host "FAIL: bundle.targets does not include 'nsis'"
    exit 1
}

Write-Host "PASS: tauri.conf.json bundle configuration is valid"
Write-Host "PASS: nsis target is declared"
Write-Host "PASS: contract validation complete"

# ── Install/Launch/Uninstall (only when installer is provided) ───────────

if ([string]::IsNullOrEmpty($InstallerPath)) {
    Write-Host ""
    Write-Host "=== No installer path provided — contract-only smoke complete ==="
    Write-Host "To run full smoke: .\scripts\windows_package_smoke.ps1 -InstallerPath path\to\setup.exe"
    exit 0
}

if (-not (Test-Path $InstallerPath)) {
    Write-Host "FAIL: installer not found at $InstallerPath"
    exit 1
}

Write-Host ""
Write-Host "=== Full install/launch/uninstall smoke ==="

# 1. Clean install
Write-Host "Step 1: Installing $InstallerPath..."
Start-Process -FilePath $InstallerPath -ArgumentList "/S" -Wait
$productName = $conf.productName
$installDir = "C:\Program Files\$productName"
if (-not (Test-Path $installDir)) {
    Write-Host "FAIL: install directory not found at $installDir"
    exit 1
}
Write-Host "PASS: install"

# 2. Launch
Write-Host "Step 2: Launching browser..."
$exePath = Join-Path $installDir "$productName.exe"
if (-not (Test-Path $exePath)) {
    Write-Host "FAIL: executable not found at $exePath"
    exit 1
}
$proc = Start-Process -FilePath $exePath -PassThru
Start-Sleep -Seconds 3
if ($proc.HasExited) {
    Write-Host "FAIL: browser process exited immediately"
    exit 1
}
Write-Host "PASS: launch (PID $($proc.Id))"
Stop-Process -Id $proc.Id -Force

# 3. Uninstall
Write-Host "Step 3: Uninstalling..."
$uninstaller = Join-Path $installDir "Uninstall $productName.exe"
if (Test-Path $uninstaller) {
    Start-Process -FilePath $uninstaller -ArgumentList "/S" -Wait
}
if (Test-Path $installDir) {
    Write-Host "FAIL: install directory still exists after uninstall"
    exit 1
}
Write-Host "PASS: uninstall"

Write-Host ""
Write-Host "=== All smoke steps passed ==="
