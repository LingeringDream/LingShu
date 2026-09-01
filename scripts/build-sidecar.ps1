param(
    [string]$RustTarget = ""
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

$repoRoot = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot ".."))
Set-Location -LiteralPath $repoRoot

if ([string]::IsNullOrWhiteSpace($RustTarget)) {
    $RustTarget = (rustc -vV | Select-String '^host: (.+)$').Matches[0].Groups[1].Value
}
if ($RustTarget -notlike '*-windows-*') {
    throw "build-sidecar.ps1 only packages the self-contained Windows sidecar (target: $RustTarget)"
}

Write-Host "Building self-contained lingshu-server for $RustTarget"
cargo build -p lingshu-server --release --target $RustTarget --features desktop-embedded-db
if ($LASTEXITCODE -ne 0) {
    throw "Sidecar build failed with exit code $LASTEXITCODE"
}

$source = Join-Path $repoRoot "target\$RustTarget\release\lingshu-server.exe"
$binaryDirectory = Join-Path $repoRoot "src-tauri\binaries"
$destination = Join-Path $binaryDirectory "lingshu-server-$RustTarget.exe"
New-Item -ItemType Directory -Path $binaryDirectory -Force | Out-Null
Copy-Item -LiteralPath $source -Destination $destination -Force

Write-Host "Sidecar binary placed at $destination"
Write-Host "Next: cd frontend; npm run tauri build -- --bundles nsis"
