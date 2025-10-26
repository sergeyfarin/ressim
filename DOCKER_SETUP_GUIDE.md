# Moving Ressim to Docker Desktop - Complete Step-by-Step Guide

**Date:** 2025-10-26
**Platform:** Windows 11 with Docker Desktop
**Target:** Linux container development environment

---

## Prerequisites

### Step 0: Verify Requirements

Before starting, ensure you have:

1. **Docker Desktop for Windows** installed and running
   - Download from: https://www.docker.com/products/docker-desktop
   - Version: 4.25+ recommended
   - Verify: Run `docker --version` in PowerShell
   - Expected output: `Docker version 24.0.x, build ...`

2. **WSL 2 Backend** (Windows Subsystem for Linux)
   - Docker Desktop uses WSL 2 on Windows 11
   - Verify: `docker run hello-world` should succeed
   - Should see "Hello from Docker!" message

3. **Current project** on Windows 11 at `c:\Users\serge\Repos\ressim`
   - All source code present
   - package.json, Cargo.toml files intact
   - Node modules and target/ directory (will be removed)

4. **Git** (recommended for version control)
   - Not required but helpful for managing changes

---

## Phase 1: Prepare Your Windows Project

### Step 1.1: Clean Up Local Build Artifacts

These will be rebuilt in the container and bloat the Docker image.

```powershell
# Open PowerShell and navigate to project
cd c:\Users\serge\Repos\ressim

# Remove Node dependencies (will reinstall in container)
Remove-Item -Recurse -Force node_modules -ErrorAction SilentlyContinue
Write-Host "✓ Removed node_modules"

# Remove Rust build artifacts
Remove-Item -Recurse -Force src\lib\ressim\target -ErrorAction SilentlyContinue
Write-Host "✓ Removed Rust target directory"

# Remove generated WebAssembly package
Remove-Item -Recurse -Force src\lib\ressim\pkg -ErrorAction SilentlyContinue
Write-Host "✓ Removed generated pkg directory"

# Verify clean state
ls -la | Select-Object Name
```

**Expected output:**
```
✓ Removed node_modules
✓ Removed Rust target directory  
✓ Removed generated pkg directory

Name                           Type
----                           ----
Dockerfile                     File
docker-compose.yml             File
.dockerignore                  File
index.html                     File
jsconfig.json                  File
package.json                   File
README.md                      File
svelte.config.js               File
vite.config.js                 File
public/                        Folder
src/                           Folder
```

---

## Phase 2: Set Up Docker Files (Already Created)

### Step 2.1: Verify Docker Configuration Files

The following files have been created in your project root:

```powershell
# Check Dockerfile exists
Get-Content Dockerfile | Select-Object -First 10
# Should show: "# Multi-stage Dockerfile for Ressim Simulator"

# Check docker-compose.yml exists
Get-Content docker-compose.yml | Select-Object -First 10
# Should show: "version: '3.8'"

# Check .dockerignore exists
Get-Content .dockerignore | Select-Object -First 10
# Should show: "# Docker build ignore file"
```

**Files present:**
- ✅ `Dockerfile` - Defines the container image
- ✅ `docker-compose.yml` - Orchestrates development environment
- ✅ `.dockerignore` - Prevents unnecessary files in build

### Step 2.2: Verify Docker Daemon

```powershell
# Start Docker Desktop if not already running
# (Usually runs automatically on Windows 11)

# Verify Docker daemon is running
docker ps

# Expected: Either empty list or running containers
# If error: "Cannot connect to Docker daemon"
#   → Start Docker Desktop from Windows Start menu
#   → Wait 30 seconds for it to fully initialize
```

---

## Phase 3: Build Docker Image

### Step 3.1: Build the Container Image

This step compiles the image with all dependencies (Node, Rust, wasm-pack, etc.)

```powershell
# Navigate to project root
cd c:\Users\serge\Repos\ressim

# Build the image (first time takes 3-5 minutes)
docker-compose build --no-cache

# Wait for completion
# You'll see output like:
# [+] Building 45.2s (18/18) FINISHED
# => [internal] load build context
# => [builder 1/18] FROM rust:latest
# => [builder 2/18] WORKDIR /app
# ... (many more steps)
```

**What happens during build:**
1. Pulls base `rust:latest` image (~1 GB)
2. Installs system dependencies (curl, git, python3, etc.)
3. Installs `wasm-pack` for WebAssembly compilation
4. Installs Node.js LTS
5. Installs Rust wasm32 target
6. Copies your project into container
7. Compiles Rust → WebAssembly
8. Installs Node dependencies
9. Creates final production image

**Estimated time:** 3-5 minutes (slower on first run due to image pulls)

### Step 3.2: Verify Image Built Successfully

```powershell
# List built images
docker images | grep ressim

# Expected output:
# REPOSITORY          TAG       IMAGE ID      CREATED        SIZE
# ressim              latest    abc123def456  2 minutes ago   1.2GB

# Also check intermediate builder image
docker images | grep rust
```

---

## Phase 4: Start Development Container

### Step 4.1: Launch the Container

```powershell
# Start the development container
docker-compose up -d ressim-dev

# Wait for startup (should take 10-15 seconds)
# You'll see:
# [+] Running 1/1
# ✓ Container ressim-dev Started

# Verify container is running
docker ps

# Expected:
# CONTAINER ID  IMAGE     COMMAND              PORTS           NAMES
# abc123def456  ressim    npm run dev -- ...   5173->5173/tcp  ressim-dev
```

### Step 4.2: Watch Startup Logs

```powershell
# View container startup logs
docker logs -f ressim-dev

# Should see:
# > dev
# 
# VITE v7.1.7  ready in 123 ms
# ➜  Local:   http://localhost:5173/
# ➜  press h to show help

# Press Ctrl+C to exit logs (container still runs)
```

### Step 4.3: Access the Application

```powershell
# Open browser and navigate to:
# http://localhost:5173

# Should see:
# - Ressim simulator interface
# - 3D visualization (if grid shows)
# - All UI controls working
```

---

## Phase 5: Development Workflow

### Step 5.1: Edit Code from Windows (with Live Reload)

The container mounts your Windows project directory with live reload:

```powershell
# Edit files in VS Code on Windows (as you normally would)
# c:\Users\serge\Repos\ressim\src\*.svelte
# c:\Users\serge\Repos\ressim\src\App.svelte
# c:\Users\serge\Repos\ressim\src\lib\*.svelte

# Changes are automatically reflected in browser via Vite HMR
# No manual rebuild needed - just save and refresh browser
```

### Step 5.2: Modify Rust WebAssembly Code

```powershell
# If you modify Rust code (src/lib/ressim/src/lib.rs)
# The container automatically rebuilds the WASM module

# Or manually trigger rebuild:
docker-compose exec ressim-dev wasm-pack build src/lib/ressim --target bundler --release

# Then reload browser
```

### Step 5.3: View Console Logs from Container

```powershell
# See all container output (Vite logs, errors, warnings)
docker logs -f ressim-dev

# Follow new logs as they appear (Ctrl+C to exit)
```

### Step 5.4: Shell Access to Container

```powershell
# Get interactive shell access inside container
docker-compose exec ressim-dev bash

# Now you're inside Linux container with:
# - Rust toolchain available
# - Node.js and npm/bun available
# - Your project mounted at /app

# Common commands inside container:
cargo test                    # Run Rust tests
cargo build                   # Build Rust (without WASM)
cargo doc --open             # Generate docs
npm run build                 # Build production frontend
wasm-pack test               # Test WASM module

# Exit shell
exit
```

---

## Phase 6: Useful Commands

### Container Management

```powershell
# Start container
docker-compose up -d ressim-dev

# Stop container
docker-compose down ressim-dev

# Restart container
docker-compose restart ressim-dev

# View running containers
docker ps

# View all containers (including stopped)
docker ps -a

# Remove container
docker-compose down

# View container logs
docker logs -f ressim-dev

# View system resource usage
docker stats ressim-dev
```

### Build Management

```powershell
# Rebuild image (after Dockerfile changes)
docker-compose build --no-cache ressim-dev

# Build only without starting
docker-compose build

# View build cache
docker builder ls
```

### Debugging

```powershell
# Execute command in running container
docker-compose exec ressim-dev npm run build

# Check container health
docker inspect ressim-dev | Select-Object -ExpandProperty State

# View network connections
docker network ls

# Get container IP
docker inspect ressim-dev --format='{{.NetworkSettings.IPAddress}}'
```

---

## Phase 7: Common Issues & Solutions

### Issue 1: "Cannot connect to Docker daemon"

**Symptoms:** Error when running `docker ps`

**Solution:**
```powershell
# Start Docker Desktop
# - Click Windows Start menu
# - Type "Docker Desktop"
# - Click to open
# - Wait 30 seconds for startup
# - Verify: docker ps (should work now)

# Or check if it's already running
Get-Process docker -ErrorAction SilentlyContinue
```

### Issue 2: Port 5173 Already in Use

**Symptoms:** Error "Address already in use"

**Solution:**
```powershell
# Option 1: Stop existing container
docker-compose down

# Option 2: Use different port (edit docker-compose.yml)
# Change: 5173:5173
# To:     5174:5173
# Then:   docker-compose up -d

# Find what's using port 5173
Get-NetTCPConnection -LocalPort 5173 -ErrorAction SilentlyContinue
```

### Issue 3: Build Takes Very Long

**Symptoms:** First build takes 5+ minutes

**Solution:**
- This is normal! First build pulls large base images
- Subsequent builds are much faster (cached layers)
- Builds after Dockerfile changes take 1-2 minutes
- To speed up: use `--cache-from` flag

### Issue 4: Changes Not Reflecting in Browser

**Symptoms:** Edit Svelte file, browser doesn't update

**Solution:**
```powershell
# Ensure volume mount is working
docker inspect ressim-dev | Select-Object -ExpandProperty Mounts

# Should show /app mounted

# Manually restart container
docker-compose restart ressim-dev

# Check logs for errors
docker logs -f ressim-dev

# Refresh browser (Ctrl+Shift+R for hard refresh)
```

### Issue 5: Rust Compilation Error in Container

**Symptoms:** "error[E0433]: cannot find..." in container logs

**Solution:**
```powershell
# This often means dependencies need update
docker-compose down

# Rebuild without cache
docker-compose build --no-cache ressim-dev

# Or inside container
docker-compose exec ressim-dev cargo update
docker-compose exec ressim-dev cargo build
```

### Issue 6: Out of Disk Space

**Symptoms:** Build fails with "No space left on device"

**Solution:**
```powershell
# Clean up dangling images and volumes
docker image prune -a
docker volume prune
docker builder prune

# Check disk usage
docker system df

# Remove specific image
docker rmi ressim:latest
```

---

## Phase 8: Advanced Configuration

### Custom Environment Variables

Edit `docker-compose.yml`:

```yaml
environment:
  - NODE_ENV=development
  - RUST_BACKTRACE=full        # More detailed Rust errors
  - RUST_LOG=debug             # Rust logging level
  - VITE_DEBUG=true            # Vite debug mode
```

### Adjust Resource Limits

Edit `docker-compose.yml`:

```yaml
deploy:
  resources:
    limits:
      cpus: '8'           # Increase for faster builds
      memory: 8G
    reservations:
      cpus: '4'
      memory: 4G
```

### Add Additional Services

To add PostgreSQL, Redis, etc., extend `docker-compose.yml`:

```yaml
  postgres:
    image: postgres:15
    ports:
      - "5432:5432"
    environment:
      POSTGRES_PASSWORD: password
    volumes:
      - postgres-data:/var/lib/postgresql/data

volumes:
  postgres-data:
```

---

## Phase 9: Verify Complete Setup

### Checklist

```powershell
# 1. Verify Docker Desktop running
docker ps
# ✓ Shows ressim-dev container or empty list

# 2. Verify container status
docker ps | grep ressim-dev
# ✓ Shows RUNNING status

# 3. Verify application accessible
Start-Process "http://localhost:5173"
# ✓ Browser opens to simulator interface

# 4. Verify live reload working
# Edit src/App.svelte (add comment or change text)
# Save file
# Browser auto-refreshes
# ✓ Changes visible

# 5. Verify Rust code accessible
docker-compose exec ressim-dev ls -la src/lib/ressim/src/
# ✓ Shows lib.rs and other files

# 6. Verify logs accessible
docker logs ressim-dev | tail -20
# ✓ Shows recent container output
```

**All checks passing?** → Setup complete! ✅

---

## Phase 10: Development Best Practices

### Do's ✅

```
✓ Edit code on Windows using VS Code
✓ Let container handle compilation
✓ Use docker-compose exec for package installs
✓ Commit Docker files to version control
✓ Use .dockerignore to exclude build artifacts
✓ Keep Dockerfile updated with new dependencies
✓ Use named volumes for caches (cargo, npm)
```

### Don'ts ❌

```
✗ Don't edit files inside container (they'll be lost)
✗ Don't install packages on Windows (use container)
✗ Don't rebuild image frequently (rebuild only when needed)
✗ Don't commit build artifacts (node_modules, target/)
✗ Don't use bind mounts for performance-critical code
✗ Don't ignore Docker logs/errors
```

---

## Phase 11: Cleanup and Housekeeping

### When Done Developing

```powershell
# Stop container but keep it
docker-compose stop

# Stop and remove container
docker-compose down

# Clean up all Docker resources (careful!)
docker system prune -a
```

### Regular Maintenance

```powershell
# Weekly: Update base images
docker pull rust:latest
docker pull node:20-slim

# Monthly: Rebuild image with latest deps
docker-compose build --no-cache

# Quarterly: Clean up old images
docker image prune -a

# Check disk usage
docker system df
```

---

## Phase 12: Migrating Back to Windows (If Needed)

If you ever want to return to native Windows development:

```powershell
# Extract code from container
docker-compose cp ressim-dev:/app/src ./src-backup

# Install Node and Rust on Windows (from prerequisites)
# Copy code back
# Run: npm install
# Run: cargo build

# Note: This usually isn't necessary - keep using Docker!
```

---

## Quick Reference Commands

```powershell
# START
docker-compose up -d ressim-dev

# STOP
docker-compose down

# LOGS
docker logs -f ressim-dev

# SHELL
docker-compose exec ressim-dev bash

# BUILD
docker-compose build --no-cache

# REBUILD WASM
docker-compose exec ressim-dev wasm-pack build src/lib/ressim --target bundler --release

# STATUS
docker ps
docker stats ressim-dev

# CLEANUP
docker system prune -a

# VERIFY
curl http://localhost:5173
```

---

## Troubleshooting Summary

| Problem | Command |
|---------|---------|
| Container won't start | `docker logs -f ressim-dev` |
| Port in use | `Get-NetTCPConnection -LocalPort 5173` |
| Slow builds | `docker-compose build --no-cache` |
| Out of disk | `docker system prune -a` |
| Changes not showing | `docker-compose restart ressim-dev` |
| Need to debug | `docker-compose exec ressim-dev bash` |

---

## Next Steps

1. ✅ Follow Phase 1-4 above
2. ✅ Verify application loads at http://localhost:5173
3. ✅ Test live reload by editing a Svelte file
4. ✅ Try building Rust code inside container
5. ✅ Commit Docker files to git
6. ✅ Delete Windows development environment (optional)
7. ✅ Document setup for team members

---

## Summary

Your Ressim project is now containerized! 🐳

**Benefits:**
- ✅ Isolated Linux environment (matches production)
- ✅ No dependency conflicts with Windows
- ✅ Reproducible builds across machines
- ✅ Easy to onboard new developers
- ✅ Can develop on any OS (Mac, Linux, Windows)
- ✅ Faster deployment to production
- ✅ Can run multiple project versions simultaneously

**Directory structure maintained:**
```
c:\Users\serge\Repos\ressim\          (Windows)
├── src/                              (mounted to /app/src in container)
│   ├── App.svelte
│   ├── lib/
│   │   ├── Counter.svelte
│   │   └── ressim/
│   │       ├── src/                  (Rust source)
│   │       ├── Cargo.toml
│   │       └── pkg/                  (Generated WASM)
│   └── main.js
├── package.json
├── vite.config.js
├── Dockerfile                         ✅ Created
├── docker-compose.yml                ✅ Created
└── .dockerignore                     ✅ Created
```

**Status:** Ready to use! 🚀

