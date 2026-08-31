param(
    [string]$Target = "x86_64-pc-windows-msvc"
)

$ErrorActionPreference = "Stop"
$RepoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$BinaryName = "lingshu-server.exe"

Push-Location $RepoRoot
try {
    Write-Host "Building lingshu-server for $Target"
    cargo build -p lingshu-server --release --target $Target
    if ($LASTEXITCODE -ne 0) {
        throw "cargo build failed with exit code $LASTEXITCODE"
    }

    $Source = Join-Path $RepoRoot "target\$Target\release\$BinaryName"
    if (-not (Test-Path $Source -PathType Leaf)) {
        throw "Sidecar binary not found: $Source"
    }

    $BinaryDir = Join-Path $RepoRoot "src-tauri\binaries"
    New-Item -ItemType Directory -Force -Path $BinaryDir | Out-Null
    $Destination = Join-Path $BinaryDir "lingshu-server-$Target.exe"
    Copy-Item -Force $Source $Destination

    Write-Host "Sidecar binary placed at $Destination"
    Write-Host "Next: cd frontend; npm run tauri build -- --bundles nsis"
}
finally {
    Pop-Location
}
