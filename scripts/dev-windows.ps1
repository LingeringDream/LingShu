[CmdletBinding()]
param(
    [switch]$Stop,
    [switch]$NoBrowser,
    [switch]$NoBuild
)

# Keep this file ASCII-only. Windows PowerShell 5.1 reads UTF-8 files without
# a BOM through the system code page, which can corrupt non-ASCII source text.
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$projectRoot = Split-Path -Parent $PSScriptRoot
$composeFile = Join-Path $projectRoot 'docker\docker-compose.dev.yml'
$consoleUrl = 'http://127.0.0.1:5173/?section=memory-lab'
$frontendUrl = 'http://127.0.0.1:5173/'
$healthUrl = 'http://127.0.0.1:8080/api/v1/system/health'
$script:dockerReady = $false

function Write-Step {
    param([string]$Message)
    Write-Host "`n==> $Message" -ForegroundColor Cyan
}

function Write-Check {
    param([string]$State, [string]$Message)

    $color = 'Green'
    if ($State -eq 'WARN') { $color = 'Yellow' }
    if ($State -eq 'FAIL') { $color = 'Red' }
    Write-Host ("[{0}] {1}" -f $State, $Message) -ForegroundColor $color
}

function Stop-WithMessage {
    param([string]$Missing, [string]$Fix)
    throw ("Missing or unavailable: {0}`nHow to fix: {1}" -f $Missing, $Fix)
}

function Invoke-Compose {
    param([Parameter(Mandatory = $true)][string[]]$ComposeArguments)

    & docker compose -f $composeFile --profile full @ComposeArguments
    if ($LASTEXITCODE -ne 0) {
        throw ("Docker Compose command failed: docker compose {0}`nReview the command output above for the exact build or startup error." -f ($ComposeArguments -join ' '))
    }
}

function Test-DockerEngine {
    & docker info *> $null
    return $LASTEXITCODE -eq 0
}

function Assert-RequiredFiles {
    Write-Step 'Checking required project files'

    $requiredFiles = @(
        @{ Path = $composeFile; Purpose = 'Docker Compose configuration' },
        @{ Path = (Join-Path $projectRoot 'Cargo.toml'); Purpose = 'Rust workspace manifest' },
        @{ Path = (Join-Path $projectRoot 'crates\lingshu-server\Dockerfile.dev'); Purpose = 'backend Dockerfile' },
        @{ Path = (Join-Path $projectRoot 'frontend\Dockerfile.dev'); Purpose = 'frontend Dockerfile' },
        @{ Path = (Join-Path $projectRoot 'frontend\package.json'); Purpose = 'frontend package manifest' },
        @{ Path = (Join-Path $projectRoot 'datasets\zte_memory_50turn_v1.json'); Purpose = 'benchmark dataset' }
    )

    foreach ($item in $requiredFiles) {
        if (-not (Test-Path -LiteralPath $item.Path -PathType Leaf)) {
            Stop-WithMessage -Missing ("{0}: {1}" -f $item.Purpose, $item.Path) -Fix 'Restore the missing file from the repository, then run the launcher again.'
        }
    }
    Write-Check -State 'OK' -Message 'All required project files are present.'
}

function Assert-DockerCli {
    Write-Step 'Checking Docker requirements'

    if (-not (Get-Command docker -ErrorAction SilentlyContinue)) {
        Stop-WithMessage -Missing 'Docker CLI (docker.exe)' -Fix 'Install Docker Desktop from https://www.docker.com/products/docker-desktop/ and reopen this launcher.'
    }
    Write-Check -State 'OK' -Message 'Docker CLI was found.'

    & docker compose version *> $null
    if ($LASTEXITCODE -ne 0) {
        Stop-WithMessage -Missing 'Docker Compose v2 plugin' -Fix 'Update or reinstall Docker Desktop. The launcher requires the command: docker compose.'
    }
    Write-Check -State 'OK' -Message 'Docker Compose v2 is available.'
}

function Start-DockerDesktopIfNeeded {
    if (Test-DockerEngine) {
        $script:dockerReady = $true
        Write-Check -State 'OK' -Message 'Docker engine is running.'
        return
    }

    $dockerDesktopCandidates = @(
        (Join-Path $env:ProgramFiles 'Docker\Docker\Docker Desktop.exe'),
        (Join-Path $env:LOCALAPPDATA 'Docker\Docker Desktop.exe')
    )
    $dockerDesktop = $dockerDesktopCandidates | Where-Object { Test-Path -LiteralPath $_ } | Select-Object -First 1
    if (-not $dockerDesktop) {
        Stop-WithMessage -Missing 'A running Docker engine and Docker Desktop installation' -Fix 'Install Docker Desktop, complete its WSL 2 setup, and run this launcher again.'
    }

    Write-Check -State 'WARN' -Message 'Docker engine is not running. Starting Docker Desktop now...'
    Start-Process -FilePath $dockerDesktop -WindowStyle Hidden

    $deadline = (Get-Date).AddMinutes(3)
    while ((Get-Date) -lt $deadline) {
        Start-Sleep -Seconds 3
        Write-Host '.' -NoNewline -ForegroundColor DarkGray
        if (Test-DockerEngine) {
            Write-Host ''
            $script:dockerReady = $true
            Write-Check -State 'OK' -Message 'Docker Desktop is ready.'
            return
        }
    }

    Write-Host ''
    Stop-WithMessage -Missing 'A responsive Docker engine' -Fix 'Open Docker Desktop and check its error screen. Verify that WSL 2 and hardware virtualization are enabled, then retry.'
}

function Test-TcpPort {
    param([int]$Port)
    try {
        $listeners = [System.Net.NetworkInformation.IPGlobalProperties]::GetIPGlobalProperties().GetActiveTcpListeners()
        return @($listeners | Where-Object { $_.Port -eq $Port }).Count -gt 0
    }
    catch {
        return $false
    }
}

function Get-PortOwnerHint {
    param([int]$Port)
    try {
        $connection = Get-NetTCPConnection -State Listen -LocalPort $Port -ErrorAction Stop | Select-Object -First 1
        $process = Get-Process -Id $connection.OwningProcess -ErrorAction Stop
        return ("{0} (PID {1})" -f $process.ProcessName, $process.Id)
    }
    catch {
        return 'another local process'
    }
}

function Assert-PortsAvailable {
    Write-Step 'Checking required ports'

    $runningServices = @(& docker compose -f $composeFile --profile full ps --services --status running 2>$null)
    $ports = @(
        @{ Port = 5432; Service = 'postgres' },
        @{ Port = 6379; Service = 'redis' },
        @{ Port = 6333; Service = 'qdrant' },
        @{ Port = 6334; Service = 'qdrant' },
        @{ Port = 8080; Service = 'backend' },
        @{ Port = 5173; Service = 'frontend' }
    )

    foreach ($item in $ports) {
        if (($runningServices -notcontains $item.Service) -and (Test-TcpPort -Port $item.Port)) {
            $owner = Get-PortOwnerHint -Port $item.Port
            Stop-WithMessage -Missing ("Free TCP port {0}; it is used by {1}" -f $item.Port, $owner) -Fix ("Stop the program using port {0}, or change that port in docker\docker-compose.dev.yml." -f $item.Port)
        }
    }
    Write-Check -State 'OK' -Message 'Required ports are available or already owned by this LingShu stack.'
}

function Assert-ComposeConfiguration {
    & docker compose -f $composeFile --profile full config --quiet
    if ($LASTEXITCODE -ne 0) {
        Stop-WithMessage -Missing 'A valid Docker Compose configuration' -Fix 'Review docker\docker-compose.dev.yml and the validation error printed above.'
    }
    Write-Check -State 'OK' -Message 'Docker Compose configuration is valid.'
}

function Write-OptionalDependencyWarnings {
    try {
        $ollamaResponse = Invoke-WebRequest -Uri 'http://127.0.0.1:11434/api/tags' -UseBasicParsing -TimeoutSec 2
        if ($ollamaResponse.StatusCode -ge 200 -and $ollamaResponse.StatusCode -lt 400) {
            Write-Check -State 'OK' -Message 'Optional Ollama service is available.'
            return
        }
    }
    catch {
        # Ollama is optional and must not prevent the console from starting.
    }
    Write-Check -State 'WARN' -Message 'Optional Ollama was not found. Memory demos still work; local chat and embeddings require Ollama or an API provider configured in Settings.'
}

function Show-ServiceDiagnostics {
    if (-not $script:dockerReady) { return }

    Write-Host "`nCurrent service status:" -ForegroundColor Yellow
    & docker compose -f $composeFile --profile full ps -a
    Write-Host "`nRecent backend and frontend logs:" -ForegroundColor Yellow
    & docker compose -f $composeFile --profile full logs --tail 80 backend frontend
}

function Wait-Endpoint {
    param([string]$Name, [string]$Service, [string]$Url, [int]$TimeoutSeconds)

    $deadline = (Get-Date).AddSeconds($TimeoutSeconds)
    $nextStatusAt = (Get-Date).AddSeconds(15)
    while ((Get-Date) -lt $deadline) {
        try {
            $response = Invoke-WebRequest -Uri $Url -UseBasicParsing -TimeoutSec 3
            if ($response.StatusCode -ge 200 -and $response.StatusCode -lt 400) {
                Write-Host ''
                Write-Check -State 'OK' -Message "$Name is ready."
                return
            }
        }
        catch {
            # The service is still compiling or starting; keep waiting.
        }

        $containerId = (& docker compose -f $composeFile --profile full ps -q $Service 2>$null | Select-Object -First 1)
        if (-not $containerId) {
            throw ("The {0} container was not created.`nHow to fix: Review the Docker Compose output and verify its Dockerfile and service configuration." -f $Service)
        }
        $containerState = (& docker inspect --format '{{.State.Status}}' $containerId 2>$null | Select-Object -First 1)
        if ($containerState -eq 'exited' -or $containerState -eq 'dead') {
            throw ("The {0} container stopped before becoming ready.`nHow to fix: Read the recent container logs printed below. They identify missing files, compile errors, or configuration errors." -f $Service)
        }

        if ((Get-Date) -ge $nextStatusAt) {
            Write-Host ("Still waiting for {0}; container state: {1}" -f $Name, $containerState) -ForegroundColor DarkGray
            $nextStatusAt = (Get-Date).AddSeconds(15)
        }
        else {
            Write-Host '.' -NoNewline -ForegroundColor DarkGray
        }
        Start-Sleep -Seconds 2
    }

    throw ("Timed out waiting for {0} at {1}.`nHow to fix: Read the status and logs below. On a slow first Rust build, rerun the launcher after compilation finishes." -f $Name, $Url)
}

function Open-WebConsole {
    param([string]$Url)
    try {
        Start-Process -FilePath 'explorer.exe' -ArgumentList $Url -ErrorAction Stop
    }
    catch {
        Write-Check -State 'WARN' -Message "Could not open the default browser. Open this URL manually: $Url"
    }
}

try {
    if (-not (Test-Path -LiteralPath $composeFile -PathType Leaf)) {
        Stop-WithMessage -Missing "Docker Compose configuration: $composeFile" -Fix 'Restore docker\docker-compose.dev.yml from the repository.'
    }
    Assert-DockerCli
    Start-DockerDesktopIfNeeded

    if ($Stop) {
        Write-Step 'Stopping the LingShu development environment'
        Invoke-Compose -ComposeArguments @('down', '--remove-orphans')
        Write-Host "`nLingShu stopped. Database volumes and memory data were preserved." -ForegroundColor Green
        exit 0
    }

    Assert-RequiredFiles

    Write-Host @'

  LingShu Dynamic Memory Engine
  Windows one-click development environment
'@ -ForegroundColor Magenta

    Assert-ComposeConfiguration
    Assert-PortsAvailable
    Write-OptionalDependencyWarnings

    Write-Step 'Starting PostgreSQL, Redis, Qdrant, backend, and frontend'
    if ($NoBuild) {
        Invoke-Compose -ComposeArguments @('up', '-d')
    }
    else {
        Invoke-Compose -ComposeArguments @('up', '-d', '--build')
    }

    Write-Step 'Waiting for services (the first build may take several minutes)'
    Wait-Endpoint -Name 'Backend API' -Service 'backend' -Url $healthUrl -TimeoutSeconds 300
    Wait-Endpoint -Name 'Web console' -Service 'frontend' -Url $frontendUrl -TimeoutSeconds 120

    Write-Step 'LingShu Dynamic Memory Engine is ready'
    Write-Host "Memory Console : $consoleUrl"
    Write-Host 'Swagger API   : http://127.0.0.1:8080/swagger-ui'
    Write-Host 'Qdrant        : http://127.0.0.1:6333/dashboard'
    Write-Host "`nServices keep running in the background. Use the stop launcher when finished." -ForegroundColor Yellow

    if (-not $NoBrowser) { Open-WebConsole -Url $consoleUrl }

    Write-Host ''
    [void](Read-Host 'The browser is open. Press Enter to close this window')
}
catch {
    Write-Host "`n============================================================" -ForegroundColor Red
    Write-Host 'LINGSHU STARTUP FAILED' -ForegroundColor Red
    Write-Host '============================================================' -ForegroundColor Red
    Write-Host $_.Exception.Message -ForegroundColor Red
    Show-ServiceDiagnostics
    Write-Host "`nAfter fixing the item above, run the launcher again." -ForegroundColor Yellow
    exit 1
}
