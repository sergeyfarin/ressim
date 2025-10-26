# Quick Docker Setup Script for Windows PowerShell
# Run: .\docker-setup.ps1
# This automates the setup process

param(
    [switch]$Clean = $false,
    [switch]$Rebuild = $false,
    [switch]$Shell = $false,
    [switch]$Stop = $false,
    [switch]$Status = $false
)

# Colors for output
$Success = @{ ForegroundColor = "Green" }
$ErrorColor = @{ ForegroundColor = "Red" }
$Info = @{ ForegroundColor = "Cyan" }
$Warning = @{ ForegroundColor = "Yellow" }

Write-Host "╔════════════════════════════════════════╗" @Info
Write-Host "║  Ressim Docker Setup Script            ║" @Info
Write-Host "╚════════════════════════════════════════╝" @Info
Write-Host ""

# Function to check Docker daemon
function Test-Docker {
    try {
        docker ps > $null 2>&1
        return $true
    } catch {
        return $false
    }
}

# Function to show status
function Show-Status {
    Write-Host "🔍 Checking Docker status..." @Info
    
    # Check Docker daemon
    if (-not (Test-Docker)) {
        Write-Host "❌ Docker daemon not running!" @ErrorColor
        Write-Host "   → Start Docker Desktop from Windows Start menu" @Warning
        Write-Host "   → Wait 30 seconds for startup" @Warning
        exit 1
    }
    Write-Host "✅ Docker daemon running" @Success
    
    # Check running containers
    $containers = docker ps --format "{{.Names}}" | Select-String "ressim"
    if ($containers) {
        Write-Host "✅ Container running: $containers" @Success
    } else {
        Write-Host "⏹️  Container not running (use 'up' command)" @Warning
    }
    
    # Show image info
    $images = docker images --format "{{.Repository}}" | Select-String "ressim"
    if ($images) {
        Write-Host "✅ Docker image built" @Success
    } else {
        Write-Host "❌ Docker image not built (use 'build' command)" @ErrorColor
    }
    
    Write-Host ""
}

# Function to remove build artifacts
function Remove-BuildArtifacts {
    Write-Host "🧹 Cleaning build artifacts..." @Info
    
    if (Test-Path "node_modules") {
        Remove-Item -Recurse -Force "node_modules" -ErrorAction SilentlyContinue
        Write-Host "   ✓ Removed node_modules" @Success
    }
    
    if (Test-Path "src\lib\ressim\target") {
        Remove-Item -Recurse -Force "src\lib\ressim\target" -ErrorAction SilentlyContinue
        Write-Host "   ✓ Removed Rust target directory" @Success
    }
    
    if (Test-Path "src\lib\ressim\pkg") {
        Remove-Item -Recurse -Force "src\lib\ressim\pkg" -ErrorAction SilentlyContinue
        Write-Host "   ✓ Removed generated WASM pkg" @Success
    }
    
    Write-Host ""
}

# Main command routing
if ($Status) {
    Show-Status
    exit 0
}

if ($Stop) {
    Write-Host "🛑 Stopping container..." @Info
    docker-compose down
    Write-Host "✅ Container stopped" @Success
    exit 0
}

if ($Clean) {
    Remove-BuildArtifacts
}

# Check prerequisites
Write-Host "✅ Prerequisites check:" @Info

if (-not (Test-Docker)) {
    Write-Host "❌ Docker daemon not running" @ErrorColor
    Write-Host "   → Start Docker Desktop from Windows Start menu" @Warning
    exit 1
}
Write-Host "   ✓ Docker daemon running" @Success

if (-not (Test-Path "package.json")) {
    Write-Host "❌ package.json not found!" @ErrorColor
    Write-Host "   → Run this script from project root directory" @Warning
    exit 1
}
Write-Host "   ✓ Project files present" @Success

if (-not (Test-Path "Dockerfile")) {
    Write-Host "❌ Dockerfile not found!" @ErrorColor
    Write-Host "   → Dockerfile should be in project root" @Warning
    exit 1
}
Write-Host "   ✓ Docker files present" @Success

Write-Host ""

# Build or rebuild image
if ($Rebuild -or -not (docker images --format "{{.Repository}}" | Select-String "ressim")) {
    Write-Host "🔨 Building Docker image..." @Info
    Write-Host "   (This takes 3-5 minutes on first build)" @Warning
    
    docker-compose build --no-cache ressim-dev
    
    if ($LASTEXITCODE -ne 0) {
        Write-Host "❌ Build failed!" @ErrorColor
        exit 1
    }
    
    Write-Host "✅ Docker image built successfully" @Success
} else {
    Write-Host "✅ Docker image already built" @Success
}

Write-Host ""

# Start container
Write-Host "🚀 Starting container..." @Info

docker-compose up -d ressim-dev

if ($LASTEXITCODE -ne 0) {
    Write-Host "❌ Failed to start container!" @ErrorColor
    exit 1
}

Write-Host "✅ Container started" @Success

# Wait for startup
Write-Host "⏳ Waiting for application startup..." @Info
Start-Sleep -Seconds 3

# Show logs
Write-Host "📋 Recent logs:" @Info
docker logs --tail 15 ressim-dev

Write-Host ""
Write-Host "✅ Setup complete!" @Success
Write-Host ""
Write-Host "🌐 Application running at: http://localhost:5173" @Info
Write-Host ""
Write-Host "📝 Useful commands:" @Info
Write-Host "   .\docker-setup.ps1 -Status      # Show container status" @Info
Write-Host "   .\docker-setup.ps1 -Stop        # Stop container" @Info
Write-Host "   .\docker-setup.ps1 -Rebuild     # Rebuild image" @Info
Write-Host "   .\docker-setup.ps1 -Shell       # Open shell in container" @Info
Write-Host "   docker-compose exec ressim-dev bash  # Direct shell access" @Info
Write-Host "   docker logs -f ressim-dev            # View live logs" @Info
Write-Host ""

# Open browser if requested
$response = Read-Host "Open browser? (y/n)"
if ($response -eq "y" -or $response -eq "Y") {
    Start-Process "http://localhost:5173"
    Write-Host "🌐 Opening browser..." @Success
}

Write-Host ""
Write-Host "👉 Next steps:" @Info
Write-Host "   1. Edit code in VS Code (c:\Users\serge\Repos\ressim\src)" @Info
Write-Host "   2. Browser auto-refreshes with changes (Vite HMR)" @Info
Write-Host "   3. See DOCKER_SETUP_GUIDE.md for more details" @Info
Write-Host ""

if ($Shell) {
    Write-Host "🐚 Opening shell in container..." @Info
    docker-compose exec ressim-dev bash
}
