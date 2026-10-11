# Install prebuilt LingShu CLI programs in the current user's profile.
param([Parameter(ValueFromRemainingArguments = $true)][string[]]$InstallArgs = @())
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
$prefix = if ($env:LINGSHU_INSTALL_PREFIX) { $env:LINGSHU_INSTALL_PREFIX } else { Join-Path $env:LOCALAPPDATA 'LingShu\cli' }
$binDir = if ($env:LINGSHU_BIN_DIR) { $env:LINGSHU_BIN_DIR } else { Join-Path $env:LOCALAPPDATA 'LingShu\bin' }
$version = 'latest'; $archive = ''; $uninstall = $false; $purgeData = ''; $waitPid = 0
for ($index = 0; $index -lt $InstallArgs.Count; $index++) {
    $option = $InstallArgs[$index]
    if ($option -eq '--uninstall') { $uninstall = $true; continue }
    if ($option -in @('--help', '-h')) { Write-Host 'Usage: install.ps1 [--version vX.Y.Z] [--archive FILE] [--prefix DIR] [--bin-dir DIR]'; exit 0 }
    if ($option -notin @('--prefix', '--bin-dir', '--version', '--archive', '--purge-data', '--wait-pid')) { throw "Unknown option: $option" }
    $index++; if ($index -ge $InstallArgs.Count) { throw "Missing value for $option" }
    $value = $InstallArgs[$index]
    switch ($option) {
        '--prefix' { $prefix = $value }; '--bin-dir' { $binDir = $value }; '--version' { $version = $value }
        '--archive' { $archive = $value }; '--purge-data' { $purgeData = $value }; '--wait-pid' { $waitPid = [int]$value }
    }
}
if (-not [IO.Path]::IsPathRooted($prefix) -or -not [IO.Path]::IsPathRooted($binDir)) { throw 'Install and bin directories must be absolute' }
$prefix = [IO.Path]::GetFullPath($prefix).TrimEnd('\')
$binDir = [IO.Path]::GetFullPath($binDir).TrimEnd('\')
if ($prefix -eq [IO.Path]::GetPathRoot($prefix).TrimEnd('\') -or $prefix -eq $env:USERPROFILE.TrimEnd('\')) { throw 'Unsafe install prefix' }
if ($version -notmatch '^[a-zA-Z0-9._+-]+$') { throw 'Invalid version' }
$marker = Join-Path $prefix '.lingshu-install'
if ($waitPid -gt 0) { Wait-Process -Id $waitPid -ErrorAction SilentlyContinue }
function Stop-LingShu {
    $cli = Join-Path $prefix 'current\lingshu.exe'
    if (Test-Path -LiteralPath $cli) { & $cli stop; if ($LASTEXITCODE -ne 0) { throw 'Could not stop managed LingShu service' } }
}
if ($uninstall) {
    if (-not (Test-Path -LiteralPath $marker) -or (Get-Content -LiteralPath $marker)[0] -ne 'lingshu-cli-v1') { throw 'Not a managed LingShu installation' }
    $binDir = (Get-Content -LiteralPath $marker)[1]
    Stop-LingShu
    $launcher = Join-Path $binDir 'lingshu.cmd'
    if ((Test-Path -LiteralPath $launcher) -and (Get-Content -LiteralPath $launcher -Raw).Contains((Join-Path $prefix 'current\lingshu.exe').Replace('%', '%%'))) { Remove-Item -LiteralPath $launcher }
    if ($purgeData) {
        $config = Join-Path $purgeData 'config.json'
        if (-not (Test-Path -LiteralPath $config) -or (Get-Content -LiteralPath $config -Raw | ConvertFrom-Json).application -ne 'lingshu-cli') { throw 'Refusing to purge an unrecognized data directory' }
        Remove-Item -LiteralPath $purgeData -Recurse -Force
    }
    # Remove the junction itself, then the owned release directories.
    $current = Join-Path $prefix 'current'
    if (Test-Path -LiteralPath $current) { [IO.Directory]::Delete($current) }
    Remove-Item -LiteralPath $prefix -Recurse -Force
    $userPath = [string][Environment]::GetEnvironmentVariable('Path', 'User')
    $updated = ($userPath -split ';' | Where-Object { $_ -and $_.TrimEnd('\') -ine $binDir }) -join ';'
    [Environment]::SetEnvironmentVariable('Path', $updated, 'User')
    Write-Host 'LingShu CLI uninstalled. Memories retained unless purge was requested.'
    exit 0
}
if (-not [Environment]::Is64BitOperatingSystem) { throw 'Windows x64 is required' }
$target = 'x86_64-pc-windows-msvc'
$work = Join-Path ([IO.Path]::GetTempPath()) ('lingshu-install-' + [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $work | Out-Null
try {
    if (-not $archive) {
        $archive = Join-Path $work "lingshu-$target.zip"
        $authenticatedGh = $false
        if (-not $env:LINGSHU_DOWNLOAD_BASE_URL -and (Get-Command gh -ErrorAction SilentlyContinue)) {
            try {
                & gh auth status *> $null
                $authenticatedGh = $LASTEXITCODE -eq 0
            } catch { $authenticatedGh = $false }
        }
        if ($authenticatedGh) {
            $downloadArgs = @('release', 'download')
            if ($version -ne 'latest') { if (-not $version.StartsWith('v')) { $version = 'v' + $version }; $downloadArgs += $version }
            $downloadArgs += @('--repo', 'LingeringDream/LingShu', '--pattern', "lingshu-$target.zip", '--pattern', "lingshu-$target.zip.sha256", '--dir', $work)
            & gh @downloadArgs
            if ($LASTEXITCODE -ne 0) { throw 'GitHub release download failed; check repository access and release availability' }
        } else {
            $base = if ($env:LINGSHU_DOWNLOAD_BASE_URL) { $env:LINGSHU_DOWNLOAD_BASE_URL.TrimEnd('/') } else { 'https://github.com/LingeringDream/LingShu/releases' }
            if ($version -eq 'latest') { $base += '/latest/download' } else { if (-not $version.StartsWith('v')) { $version = 'v' + $version }; $base += '/download/' + $version }
            $headers = @{}; if ($env:GH_TOKEN -and ($base.StartsWith('https://github.com/') -or $base.StartsWith('https://api.github.com/'))) { $headers['Authorization'] = 'Bearer ' + $env:GH_TOKEN }
            Invoke-WebRequest -Uri "$base/lingshu-$target.zip" -Headers $headers -OutFile $archive -UseBasicParsing
            Invoke-WebRequest -Uri "$base/lingshu-$target.zip.sha256" -Headers $headers -OutFile "$archive.sha256" -UseBasicParsing
        }
    }
    if (-not (Test-Path -LiteralPath $archive) -or -not (Test-Path -LiteralPath "$archive.sha256")) { throw 'Archive and its .sha256 file are required' }
    $expected = ((Get-Content -LiteralPath "$archive.sha256" -Raw).Trim() -split '\s+')[0].ToLowerInvariant()
    if ($expected -notmatch '^[0-9a-f]{64}$' -or (Get-FileHash -LiteralPath $archive -Algorithm SHA256).Hash.ToLowerInvariant() -ne $expected) { throw 'SHA-256 mismatch; existing installation was not changed' }
    $stage = Join-Path $work 'release'
    New-Item -ItemType Directory -Path $stage | Out-Null
    Add-Type -AssemblyName System.IO.Compression.FileSystem
    $zip = [IO.Compression.ZipFile]::OpenRead($archive)
    try {
        foreach ($entry in $zip.Entries) {
            $destination = [IO.Path]::GetFullPath((Join-Path $stage $entry.FullName))
            $kind = ($entry.ExternalAttributes -shr 16) -band 0xF000
            if (-not $destination.StartsWith($stage + '\', [StringComparison]::OrdinalIgnoreCase) -or $entry.FullName.Contains(':') -or $kind -eq 0xA000) { throw 'Unsafe archive entry' }
        }
    } finally { $zip.Dispose() }
    [IO.Compression.ZipFile]::ExtractToDirectory($archive, $stage)
    foreach ($file in @('lingshu.exe', 'lingshu-server.exe', 'lingshu-mcp.exe', 'web\index.html', 'VERSION', 'TARGET', 'install.ps1', 'release.json')) {
        if (-not (Test-Path -LiteralPath (Join-Path $stage $file) -PathType Leaf)) { throw "Bundle missing $file" }
    }
    $metadata = Get-Content -LiteralPath (Join-Path $stage 'release.json') -Raw | ConvertFrom-Json
    $bundleVersion = (Get-Content -LiteralPath (Join-Path $stage 'VERSION') -Raw).Trim()
    if ($bundleVersion -notmatch '^[a-zA-Z0-9._+-]+$' -or $metadata.target -ne $target -or $metadata.version -ne $bundleVersion) { throw 'Invalid bundle metadata' }
    $binaryVersion = & (Join-Path $stage 'lingshu.exe') --version
    if ($LASTEXITCODE -ne 0 -or $binaryVersion -ne "lingshu $bundleVersion") { throw 'Binary/bundle version mismatch' }
    if (Test-Path -LiteralPath $prefix) {
        if (@(Get-ChildItem -LiteralPath $prefix -Force).Count -gt 0) {
            if (-not (Test-Path -LiteralPath $marker) -or (Get-Content -LiteralPath $marker)[0] -ne 'lingshu-cli-v1') { throw 'Install prefix is nonempty and unmanaged' }
            $binDir = (Get-Content -LiteralPath $marker)[1]
        }
    }
    New-Item -ItemType Directory -Path $binDir -Force | Out-Null
    $launcher = Join-Path $binDir 'lingshu.cmd'
    $cliPath = Join-Path $prefix 'current\lingshu.exe'
    if ((Test-Path -LiteralPath $launcher) -and -not (Get-Content -LiteralPath $launcher -Raw).Contains($cliPath.Replace('%', '%%'))) { throw 'Existing lingshu command is unmanaged' }
    Stop-LingShu
    $releases = Join-Path $prefix 'releases'; New-Item -ItemType Directory -Path $releases -Force | Out-Null
    $release = Join-Path $releases ($bundleVersion + '-' + [guid]::NewGuid().ToString('N'))
    Move-Item -LiteralPath $stage -Destination $release
    Set-Content -LiteralPath $marker -Value @('lingshu-cli-v1', $binDir) -Encoding UTF8
    [IO.File]::WriteAllText((Join-Path $prefix 'install.json'), '{"application":"lingshu-cli"}', ([Text.UTF8Encoding]::new($false)))
    $current = Join-Path $prefix 'current'
    $previous = Join-Path $prefix ('.current-' + [guid]::NewGuid().ToString('N'))
    if (Test-Path -LiteralPath $current) { Move-Item -LiteralPath $current -Destination $previous }
    try { New-Item -ItemType Junction -Path $current -Target $release | Out-Null }
    catch { if (Test-Path -LiteralPath $previous) { Move-Item -LiteralPath $previous -Destination $current }; throw }
    if (Test-Path -LiteralPath $previous) { [IO.Directory]::Delete($previous) }
    $escapedCli = $cliPath.Replace('%', '%%')
    [IO.File]::WriteAllText($launcher, "@echo off`r`nchcp 65001 >nul`r`n`"$escapedCli`" %*`r`n", ([Text.UTF8Encoding]::new($false)))
    $userPath = [string][Environment]::GetEnvironmentVariable('Path', 'User')
    if ($binDir -notin ($userPath -split ';')) { [Environment]::SetEnvironmentVariable('Path', (($userPath.TrimEnd(';') + ';' + $binDir).TrimStart(';')), 'User') }
    $env:Path = $binDir + ';' + $env:Path
    Write-Host "Installed LingShu $bundleVersion. Open a new terminal, then: lingshu start; lingshu connect cursor; lingshu open"
} finally { Remove-Item -LiteralPath $work -Recurse -Force -ErrorAction SilentlyContinue }
