# Quick Start - Docker Setup for Ressim

**Last Updated:** 2025-10-26
**Platform:** Windows 11
**Container OS:** Linux (Ubuntu)

---

## 🚀 Quick Start (5 Minutes)

### Step 1: Verify Docker Desktop is Running

```powershell
# Open PowerShell and run:
docker --version

# Should output: Docker version 24.0.x, build ...
# If error: Start Docker Desktop from Windows Start menu
```

### Step 2: Navigate to Project

```powershell
cd c:\Users\serge\Repos\ressim
```

### Step 3: Build and Start Container (First Time Only)

```powershell
# Option A: Use PowerShell script (recommended)
.\docker-setup.ps1

# Option B: Manual commands
docker-compose build --no-cache ressim-dev
docker-compose up -d ressim-dev
```

**Wait 3-5 minutes for first build** (pulls large base images)

### Step 4: Verify It's Running

```powershell
# Check container status
docker ps | grep ressim-dev

# Should see: ressim-dev   Up  (status)

# Or open browser:
Start-Process "http://localhost:5173"
```

### Step 5: Start Developing

✅ Edit files in `c:\Users\serge\Repos\ressim\src\` using VS Code
✅ Browser automatically reloads (Vite HMR)
✅ Changes visible within 1-2 seconds
✅ No manual rebuild needed!

---

## 📋 Common Commands

### Daily Development

```powershell
# Start container
docker-compose up -d ressim-dev

# View logs
docker logs -f ressim-dev

# Stop container
docker-compose down

# Get shell access
docker-compose exec ressim-dev bash
```

### Build & Deployment

```powershell
# Rebuild Rust/WASM
docker-compose exec ressim-dev wasm-pack build src/lib/ressim --target bundler --release

# Build for production
docker-compose exec ressim-dev npm run build

# Run tests
docker-compose exec ressim-dev cargo test
```

### Troubleshooting

```powershell
# Check status
.\docker-setup.ps1 -Status

# Clean artifacts and rebuild
.\docker-setup.ps1 -Clean -Rebuild

# Stop all
.\docker-setup.ps1 -Stop

# Full cleanup
docker system prune -a
```

---

## 🐳 What Gets Installed in Container?

When you run the first `docker-compose build`:

✅ **Rust** - Latest stable toolchain
✅ **wasm-pack** - WebAssembly compiler
✅ **Node.js** - LTS (v20) for frontend
✅ **npm/bun** - Package managers
✅ **wasm32 target** - Rust compilation to WebAssembly
✅ **All dependencies** from package.json and Cargo.toml

**Container size:** ~1.2 GB (includes all compilers and tools)

---

## 🔄 File Synchronization

**Windows (Host)** ← → **Linux (Container)**

```
c:\Users\serge\Repos\ressim\    (Windows edit location)
         ↕ (volume mount)
    /app/                         (Container sees files)
```

When you save a file in Windows:
1. File updated on Windows disk
2. Container sees change immediately
3. Vite detects change
4. Browser auto-refreshes

**No manual sync needed!**

---

## ⚠️ Important: Don't Edit in Container

```powershell
# DON'T do this:
docker-compose exec ressim-dev nano src/App.svelte
# Changes will be lost when container stops!

# DO this instead:
# Edit on Windows: c:\Users\serge\Repos\ressim\src\App.svelte
# Container picks up changes automatically
```

---

## 📊 Check System Resources

```powershell
# View container resource usage
docker stats ressim-dev

# View disk usage
docker system df

# Check available disk space
Get-Volume
```

---

## 🚨 Troubleshooting

### Container won't start
```powershell
docker logs -f ressim-dev
# Look for error message, then check DOCKER_SETUP_GUIDE.md
```

### Port 5173 in use
```powershell
# Find what's using the port
Get-NetTCPConnection -LocalPort 5173

# Free up the port
docker-compose down
docker-compose up -d ressim-dev
```

### Changes not showing in browser
```powershell
# Restart container
docker-compose restart ressim-dev

# Hard refresh browser (Ctrl+Shift+R)
```

### Build takes forever
```powershell
# First build takes 3-5 minutes (normal!)
# Subsequent builds are faster (cached layers)
# Check progress: docker logs -f ressim-dev
```

### Out of disk space
```powershell
# Clean up Docker
docker system prune -a

# This removes: stopped containers, dangling images, unused volumes
```

---

## 📚 Full Documentation

For detailed instructions, see:
- **`DOCKER_SETUP_GUIDE.md`** - 12 phases with detailed explanations
- **`Dockerfile`** - Container image definition
- **`docker-compose.yml`** - Container orchestration
- **`.dockerignore`** - Files excluded from Docker build

---

## ✅ Verification Checklist

```
☐ Docker Desktop installed and running
☐ Ran .\docker-setup.ps1 successfully
☐ No build errors
☐ Container shows "Up" status (docker ps)
☐ Browser loads http://localhost:5173
☐ Page displays simulator interface
☐ Can edit .svelte file and see changes
☐ Console shows Vite HMR logs
```

All checked? → Setup complete! 🎉

---

## 🎯 Next Steps

1. ✅ Run `.\docker-setup.ps1` to start
2. ✅ Open http://localhost:5173 in browser
3. ✅ Edit `src/App.svelte` to test live reload
4. ✅ Check `docker logs -f ressim-dev` to see logs
5. ✅ Read `DOCKER_SETUP_GUIDE.md` for advanced topics
6. ✅ Commit Docker files to git: `git add Dockerfile docker-compose.yml`

---

## 💡 Pro Tips

### Enable Bash Completion
```powershell
# Inside container shell
source <(docker ps --format '{{.Names}}' | grep ressim)
```

### Use WSL 2 Path
```powershell
# Access container files from WSL terminal
wsl docker exec ressim-dev ls /app
```

### Monitor Multiple Terminals
```powershell
# Terminal 1: Logs
docker logs -f ressim-dev

# Terminal 2: Shell
docker-compose exec ressim-dev bash

# Terminal 3: Stats
docker stats ressim-dev
```

### Backup Container
```powershell
# Save image for deployment
docker save ressim:latest -o ressim.tar

# Load on another machine
docker load -i ressim.tar
```

---

## 📞 Still Having Issues?

1. Check error message: `docker logs -f ressim-dev`
2. Verify Docker is running: `docker ps`
3. Check network: `docker network ls`
4. Restart everything: `docker-compose down && docker-compose up -d`
5. Nuclear option: `docker system prune -a` then rebuild

---

## 🎓 Learning Resources

- Docker Docs: https://docs.docker.com/
- Compose Reference: https://docs.docker.com/compose/compose-file/
- WSL Documentation: https://docs.microsoft.com/en-us/windows/wsl/
- Rust & WASM: https://rustwasm.org/

---

**Status:** Ready to develop! 🚀

Your Ressim project is now containerized and ready for isolated development.
