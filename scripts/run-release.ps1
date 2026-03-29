$ErrorActionPreference = "Stop"

$repoRoot = Split-Path -Parent $PSScriptRoot
$manifest = Join-Path $repoRoot "src-tauri\Cargo.toml"

Write-Host "Building release binary..."
cargo build --release --manifest-path $manifest

$exe = Join-Path $repoRoot "src-tauri\target\release\fast-avatar-ai.exe"
if (-not (Test-Path $exe)) {
  throw "release executable not found: $exe"
}

Write-Host "Starting release binary..."
Start-Process -FilePath $exe
