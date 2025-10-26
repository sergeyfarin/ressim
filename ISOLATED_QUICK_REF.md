# Isolated Container - Quick Reference Card

## ⚡ 60-Second Summary

**What changed?** No more volume mounts. Container is fully self-contained.

**How to use?** Two options:

### Option A: VS Code Remote (Fastest)
```powershell
docker-compose build ressim-dev
# F1 → "Remote-Containers: Open Folder in Container"
# Done! Edit files, auto-refreshes
```

### Option B: Docker + Git
```powershell
docker-compose build --no-cache ressim-dev
docker-compose up -d ressim-dev
# Browser: http://localhost:5173
# Edit locally, commit to Git, rebuild
```

---

## Quick Commands

```powershell
# BUILD
docker-compose build ressim-dev                # First time
docker-compose build --no-cache ressim-dev    # Rebuild

# RUN
docker-compose up -d ressim-dev       # Start
docker-compose down                   # Stop

# INTERACT
docker-compose exec ressim-dev bash   # Shell
docker-compose exec ressim-dev npm run build

# INFO
docker ps                      # See running containers
docker logs -f ressim-dev      # View logs
docker-compose ps              # Docker Compose status

# CLEAN
docker-compose down            # Remove container
docker image rm ressim-dev     # Remove image
docker system prune -a         # Remove all unused
```

---

## VS Code Remote Setup

```
1. Install Extension
   VS Code → Extensions → "Remote - Containers"

2. Build Image
   docker-compose build ressim-dev

3. Open in Container
   F1 → "Remote-Containers: Open Folder in Container"
   Select: c:\Users\serge\Repos\ressim

4. Start Development
   npm run dev
   (Browser auto-opens to localhost:5173)

5. Edit, Save, Auto-Refresh
   Done!
```

---

## Git-Based Workflow

```
1. Build Image
   docker-compose build --no-cache ressim-dev

2. Start Container
   docker-compose up -d ressim-dev

3. Edit on Windows
   Edit files in VS Code

4. Commit Changes
   git add .
   git commit -m "Feature description"

5. Rebuild (when needed)
   docker-compose build --no-cache ressim-dev
   docker-compose up -d ressim-dev
```

---

## Data Exchange

| Method | Speed | Best For |
|--------|-------|----------|
| **VS Code Remote** | Instant | Active development |
| **Git** | ~1 min rebuild | Team collaboration |
| **Docker Exec** | Seconds | Quick tasks |
| **APIs** | Network speed | Service integration |

---

## File Locations

```
Project Root (c:\Users\serge\Repos\ressim\)
├─ docker-compose.yml          # No volumes!
├─ Dockerfile                  # Multi-stage build
├─ .dockerignore               # Build optimization
│
├─ ISOLATED_QUICK_START.md              # This approach (5 min)
├─ ISOLATED_CONTAINER_GUIDE.md          # Full guide
├─ ISOLATED_CONFIGURATION_SUMMARY.md    # Technical details
│
├─ src/                         # Your project code
│  ├─ App.svelte
│  ├─ main.js
│  └─ lib/ressim/               # Rust WebAssembly
│
└─ package.json                # Node.js config
```

---

## Status Check

```powershell
# Everything working?
docker ps | grep ressim-dev
# Shows: ressim-dev | Up | 0.0.0.0:5173->5173/tcp

# Application loaded?
curl http://localhost:5173
# Shows: HTML page (not error)

# View logs
docker logs -f ressim-dev
# Shows: "Vite dev server running..."
```

---

## Troubleshooting

| Problem | Solution |
|---------|----------|
| **Port 5173 already in use** | `taskkill /PID <PID> /F` or use 5174 |
| **Can't connect to container** | `docker ps` - ensure running, then `docker-compose up -d` |
| **VS Code can't connect** | Restart VS Code, ensure container running, try again |
| **Changes not showing** | Commit + rebuild, or use VS Code Remote |
| **Build failed** | Check logs: `docker logs ressim-dev` |
| **Out of disk space** | `docker system prune -a` to clean |

---

## Daily Checklist (VS Code Remote)

```
☐ Morning: Open container in VS Code
☐ Edit files normally
☐ Save → Auto-refresh browser
☐ Commit regularly: git add . && git commit
☐ Evening: Exit and stop container
```

---

## Daily Checklist (Git-Based)

```
☐ Morning: docker-compose up -d ressim-dev
☐ Edit files on Windows
☐ Use docker-compose exec for builds
☐ Commit when ready: git add . && git commit
☐ Rebuild if major changes: docker-compose build
☐ Evening: docker-compose down
```

---

## Key Differences from Volume Approach

| Old (Volumes) | New (Isolated) |
|---------------|---|
| Live file sync | Git-based sync |
| Host ↔ Container linked | Container independent |
| Potential conflicts | No conflicts |
| Machine-dependent | Reproducible |

---

## Benefits

✅ **Security** - Isolated container, no host access
✅ **Reproducibility** - Same image everywhere
✅ **Simplicity** - No volume management
✅ **Collaboration** - Easy team sharing
✅ **Production-Ready** - Dev = Prod image

---

## Recommended Setup (Start Here)

```powershell
# 1. Build image (one-time)
cd c:\Users\serge\Repos\ressim
docker-compose build ressim-dev
# Wait 3-5 minutes

# 2a. OPTION A: Use VS Code Remote (Recommended)
# F1 → "Remote-Containers: Open Folder in Container"
# Then: npm run dev
# Done! Instant development

# 2b. OPTION B: Use Git-based approach
# docker-compose up -d ressim-dev
# Browser: http://localhost:5173
# Then: Edit, commit, rebuild as needed
```

---

## Getting Help

- **5-minute start:** Read `ISOLATED_QUICK_START.md`
- **Complete guide:** Read `ISOLATED_CONTAINER_GUIDE.md`
- **Technical details:** Read `ISOLATED_CONFIGURATION_SUMMARY.md`
- **Docker help:** https://docs.docker.com/compose/
- **VS Code Remote:** https://code.visualstudio.com/docs/remote/containers

---

## One-Liner Commands

```powershell
# Build and run
docker-compose build ressim-dev && docker-compose up -d ressim-dev

# Stop all
docker-compose down

# Full reset
docker-compose down && docker image rm ressim-dev && docker system prune -a

# Emergency rebuild
docker-compose build --no-cache ressim-dev && docker-compose restart

# View everything
docker ps && docker images | grep ressim && docker logs -f ressim-dev
```

---

## Remember

✓ Container is fully **isolated**
✓ No volume mounts = **no file conflicts**
✓ Use **Git or VS Code** to exchange data
✓ Same image for **dev and production**
✓ Easy to **rebuild or reset**

---

**Status:** ✅ Ready to use
**Setup Time:** 5 minutes
**Questions?** Check the full guides above

Last Updated: 2025-10-26
