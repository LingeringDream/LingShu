[CmdletBinding()]
param(
    [string]$OutputPath
)

# Keep this script ASCII-only for Windows PowerShell 5.1 compatibility.
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

# Resolve robocopy.exe. Some sandboxed shells run PowerShell with a stripped
# PATH that omits System32, so fall back to well-known absolute locations.
$robocopyExe = Get-Command robocopy -ErrorAction SilentlyContinue | Select-Object -ExpandProperty Source -ErrorAction SilentlyContinue
if (-not $robocopyExe) {
    $robocopyCandidates = @(
        'C:\Windows\System32\robocopy.exe'
        'C:\Windows\Sysnative\robocopy.exe'
    )
    $robocopyExe = $robocopyCandidates | Where-Object { Test-Path -LiteralPath $_ } | Select-Object -First 1
}
if (-not $robocopyExe) {
    throw 'robocopy.exe was not found on this system.'
}

$projectRoot = Split-Path -Parent $PSScriptRoot
$cargoManifest = Join-Path $projectRoot 'Cargo.toml'
$distDirectory = Join-Path $projectRoot 'dist'

if (-not (Test-Path -LiteralPath $cargoManifest -PathType Leaf)) {
    throw "Missing Cargo manifest: $cargoManifest"
}

$versionMatch = Select-String -LiteralPath $cargoManifest -Pattern '^version\s*=\s*"([^"]+)"' | Select-Object -First 1
if (-not $versionMatch) {
    throw 'Could not read the project version from Cargo.toml.'
}
$version = $versionMatch.Matches[0].Groups[1].Value

if (-not $OutputPath) {
    $OutputPath = Join-Path $distDirectory ("LingShu_{0}_Windows_Dev.zip" -f $version)
}

New-Item -ItemType Directory -Path $distDirectory -Force | Out-Null
$resolvedDist = [System.IO.Path]::GetFullPath($distDirectory).TrimEnd('\') + '\'
$resolvedOutput = [System.IO.Path]::GetFullPath($OutputPath)
if (-not $resolvedOutput.StartsWith($resolvedDist, [System.StringComparison]::OrdinalIgnoreCase)) {
    throw "The package output must stay inside: $distDirectory"
}

$stagingRoot = Join-Path ([System.IO.Path]::GetTempPath()) ("lingshu-dev-package-{0}" -f [guid]::NewGuid().ToString('N'))
$packageName = "LingShu-{0}-Windows-Dev" -f $version
$packageRoot = Join-Path $stagingRoot $packageName

try {
    New-Item -ItemType Directory -Path $packageRoot -Force | Out-Null
    Write-Host "Collecting development files..." -ForegroundColor Cyan

    $excludedDirectories = @('.git', 'dist', 'node_modules', 'target', 'logs', '.cache')
    $excludedFiles = @(
        '.env',
        'config.toml',
        '*.log',
        '*.tmp',
        '*.exe',
        '*.msi',
        '*.dmg',
        '*.deb',
        '*.AppImage'
    )
    $copyArguments = @(
        $projectRoot,
        $packageRoot,
        '/E',
        '/R:1',
        '/W:1',
        '/NFL',
        '/NDL',
        '/NJH',
        '/NJS',
        '/NP',
        '/XD'
    ) + $excludedDirectories + @('/XF') + $excludedFiles

    & $robocopyExe @copyArguments
    # Read LASTEXITCODE defensively: some restricted shells do not expose it
    # after an external call, which would otherwise fail under StrictMode.
    $robocopyExit = (Get-Variable -Name LASTEXITCODE -ErrorAction SilentlyContinue).Value
    if (-not $robocopyExit) { $robocopyExit = 0 }
    if ($robocopyExit -ge 8) {
        throw "File collection failed with robocopy exit code $robocopyExit."
    }

    if (Test-Path -LiteralPath $resolvedOutput -PathType Leaf) {
        Remove-Item -LiteralPath $resolvedOutput -Force
    }

    Write-Host "Creating archive..." -ForegroundColor Cyan
    Compress-Archive -LiteralPath $packageRoot -DestinationPath $resolvedOutput -CompressionLevel Optimal
    $archive = Get-Item -LiteralPath $resolvedOutput
    $hash = Get-FileHash -LiteralPath $resolvedOutput -Algorithm SHA256

    Write-Host "Package created successfully." -ForegroundColor Green
    Write-Host "Path   : $($archive.FullName)"
    Write-Host ("Size   : {0:N2} MB" -f ($archive.Length / 1MB))
    Write-Host "SHA256 : $($hash.Hash)"
}
finally {
    $resolvedTemp = [System.IO.Path]::GetFullPath([System.IO.Path]::GetTempPath()).TrimEnd('\') + '\'
    $resolvedStaging = [System.IO.Path]::GetFullPath($stagingRoot)
    if ((Test-Path -LiteralPath $resolvedStaging) -and $resolvedStaging.StartsWith($resolvedTemp, [System.StringComparison]::OrdinalIgnoreCase)) {
        Remove-Item -LiteralPath $resolvedStaging -Recurse -Force
    }
}
