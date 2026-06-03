# LingShu Development Environment Setup
# Run this script from the project root: .\scripts\setup-dev.ps1

$ErrorActionPreference = "Stop"
$ProjectRoot = Split-Path -Parent $PSScriptRoot

Write-Host "=== LingShu Development Environment Setup ===" -ForegroundColor Cyan

# 1. Copy .env if not exists
$envFile = Join-Path $ProjectRoot ".env"
$envExample = Join-Path $ProjectRoot ".env.example"
if (-not (Test-Path $envFile)) {
    Copy-Item $envExample $envFile
    Write-Host "[OK] Created .env from .env.example" -ForegroundColor Green
} else {
    Write-Host "[SKIP] .env already exists" -ForegroundColor Yellow
}

# 2. Start Docker Compose
Write-Host "`nStarting Docker Compose services..." -ForegroundColor Cyan
$composeFile = Join-Path $ProjectRoot "docker\docker-compose.dev.yml"
docker compose -f $composeFile up --build -d

if ($LASTEXITCODE -ne 0) {
    Write-Host "[FAIL] Docker Compose failed to start" -ForegroundColor Red
    exit 1
}

# 3. Wait for services to be healthy
Write-Host "`nWaiting for services to be healthy..." -ForegroundColor Cyan
$maxWait = 120
$waited = 0
while ($waited -lt $maxWait) {
    $status = docker compose -f $composeFile ps --format json | ConvertFrom-Json
    $allHealthy = $true
    foreach ($svc in $status) {
        if ($svc.Health -and $svc.Health -ne "healthy") {
            $allHealthy = $false
            break
        }
    }
    if ($allHealthy) {
        Write-Host "[OK] All services healthy" -ForegroundColor Green
        break
    }
    Start-Sleep -Seconds 5
    $waited += 5
    Write-Host "  Waiting... ($waited/$max_wait s)" -ForegroundColor Gray
}

if ($waited -ge $maxWait) {
    Write-Host "[WARN] Some services may not be fully healthy yet" -ForegroundColor Yellow
}

# 4. Show service status
Write-Host "`n=== Service Status ===" -ForegroundColor Cyan
docker compose -f $composeFile ps

Write-Host "`n=== Endpoints ===" -ForegroundColor Cyan
Write-Host "  Backend API:    http://localhost:8080" -ForegroundColor White
Write-Host "  API Docs:       http://localhost:8080/swagger-ui/" -ForegroundColor White
Write-Host "  Frontend:       http://localhost:5173" -ForegroundColor White
Write-Host "  PostgreSQL:     localhost:5432 (user: lingshu)" -ForegroundColor White
Write-Host "  Redis:          localhost:6379" -ForegroundColor White
Write-Host "  Qdrant:         http://localhost:6333" -ForegroundColor White

Write-Host "`n=== Setup Complete ===" -ForegroundColor Green
