param(
    [Alias("RustTarget")]
    [string]$Target = ""
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

$repoRoot = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot ".."))
Set-Location -LiteralPath $repoRoot

if ([string]::IsNullOrWhiteSpace($Target)) {
    $Target = (rustc -vV | Select-String '^host: (.+)$').Matches[0].Groups[1].Value
}
if ($Target -notlike '*-windows-*') {
    throw "build-sidecar.ps1 only packages the self-contained Windows sidecar (target: $Target)"
}

Write-Host "Building self-contained lingshu-server and lingshu-mcp for $Target"
cargo build -p lingshu-server --release --target $Target --features desktop-embedded-db
if ($LASTEXITCODE -ne 0) {
    throw "Server sidecar build failed with exit code $LASTEXITCODE"
}
cargo build -p lingshu-mcp --release --target $Target
if ($LASTEXITCODE -ne 0) {
    throw "MCP sidecar build failed with exit code $LASTEXITCODE"
}

$binaryDirectory = Join-Path $repoRoot "src-tauri\binaries"
New-Item -ItemType Directory -Path $binaryDirectory -Force | Out-Null
foreach ($binaryName in @("lingshu-server", "lingshu-mcp")) {
    $source = Join-Path $repoRoot "target\$Target\release\$binaryName.exe"
    $destination = Join-Path $binaryDirectory "$binaryName-$Target.exe"
    if (-not (Test-Path -LiteralPath $source -PathType Leaf)) {
        throw "Built sidecar is missing: $source"
    }
    Copy-Item -LiteralPath $source -Destination $destination -Force
    Write-Host "Sidecar binary placed at $destination"
}

# The bundled PostgreSQL executables and the Rust sidecar both use the
# Microsoft C runtime. Developer machines usually already have it through
# Visual Studio, but a clean end-user machine may not. Copy the x64 runtime
# into the Tauri resources so the installer remains self-contained.
$vcRuntimeCandidates = @()
if ($env:VCToolsRedistDir) {
    $vcRuntimeCandidates += Join-Path $env:VCToolsRedistDir "x64\Microsoft.VC143.CRT\vcruntime140.dll"
}

$programFilesX86 = ${env:ProgramFiles(x86)}
if ($programFilesX86) {
    $vcRedistRoot = Join-Path $programFilesX86 "Microsoft Visual Studio\2022\BuildTools\VC\Redist\MSVC"
    if (Test-Path -LiteralPath $vcRedistRoot -PathType Container) {
        $vcRuntimeCandidates += Get-ChildItem -Path $vcRedistRoot -Recurse -File -ErrorAction SilentlyContinue |
            Where-Object { $_.FullName -match "\\x64\\Microsoft\.VC143\.CRT\\vcruntime140\.dll$" } |
            Sort-Object LastWriteTime -Descending |
            Select-Object -ExpandProperty FullName
    }
}

if ($env:WINDIR) {
    $vcRuntimeCandidates += Join-Path $env:WINDIR "System32\vcruntime140.dll"
}

$vcRuntimeSource = $vcRuntimeCandidates |
    Where-Object { $_ -and (Test-Path -LiteralPath $_ -PathType Leaf) } |
    Select-Object -First 1
if (-not $vcRuntimeSource) {
    throw "Could not find x64 vcruntime140.dll. Install the Visual C++ build tools or set VCToolsRedistDir, then run this script again."
}

$resourceDirectory = Join-Path $repoRoot "src-tauri\resources"
$resourcePath = Join-Path $resourceDirectory "vcruntime140.dll"
New-Item -ItemType Directory -Path $resourceDirectory -Force | Out-Null
Copy-Item -LiteralPath $vcRuntimeSource -Destination $resourcePath -Force
Write-Host "Bundled Microsoft C runtime at $resourcePath"
Write-Host "Next: cd frontend; npm run tauri build -- --bundles nsis"
