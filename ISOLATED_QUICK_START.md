# Isolated Container: Quick Start (5 Minutes)

## What Changed?

**Before:** Volumes mapped host → container (live sync)
**Now:** Isolated container with Git or VS Code Remote

---

## Path A: VS Code Remote Containers (Recommended - Easiest)

### 1. Install Extension
```
VS Code → Extensions
Search: "Remote - Containers"
Install: ms-vscode-remote.remote-containers
```

### 2. Build Image
```powershell
cd c:\Users\serge\Repos\ressim
docker-compose build ressim-dev
# Wait 3-5 minutes for first build
```

### 3. Open in Container
```
F1 → "Remote-Containers: Open Folder in Container"
Select: c:\Users\serge\Repos\ressim
Click: "Reopen in Container"
```

### 4. Start Development
```bash
# Terminal appears inside container
npm run dev

# Access: http://localhost:5173
```

### 5. Develop Normally
- Edit files in VS Code
- Files are inside container
- Changes sync in real-time
- Commit to Git when done

**Total setup time: 5 minutes**

---

## Path B: Git-Based Workflow (Most Isolated)

### 1. Commit Current Work
```powershell
cd c:\Users\serge\Repos\ressim
git add .
git commit -m "Current work before isolation"
```

### 2. Build Image
```powershell
docker-compose build --no-cache ressim-dev
# Wait 3-5 minutes
```

### 3. Start Container
```powershell
docker-compose up -d ressim-dev

# Verify running
docker ps | grep ressim-dev
```

### 4. Access Application
```
Browser: http://localhost:5173
```

### 5. When You Need to Edit
```bash
# Option A: Open container shell
docker-compose exec ressim-dev bash

# Option B: Run command directly
docker-compose exec ressim-dev npm run build
```

### 6. Sync Changes
```powershell
# Git-based workflow
docker-compose down
# Edit locally on Windows
git add .
git commit -m "Feature: Description"

# Rebuild with new code
docker-compose build --no-cache ressim-dev
docker-compose up -d ressim-dev
```

**Total setup time: 5 minutes**

---

## Path C: Hybrid (Flexible)

1. Use VS Code Remote for development
2. Commit regularly to Git
3. Share via Git repository
4. Teammates pull and use same image

**Combines benefits of both approaches**

---

## Verify Setup

```powershell
# Check container running
docker ps | grep ressim-dev
# Should show: STATUS = Up

# Check application
curl http://localhost:5173
# Should return HTML page

# Check logs
docker logs -f ressim-dev
# Should show: Vite dev server running
```

---

## Daily Workflow (VS Code Remote)

### Morning
```
F1 → "Remote-Containers: Open Folder in Container"
Or: docker-compose up -d ressim-dev
```

### Work
```
1. Edit files normally in VS Code
2. Save → Auto-refresh browser
3. Commit as needed: git commit -m "..."
```

### End of Day
```bash
exit  # Exit container terminal

# Or Windows terminal:
docker-compose down
```

---

## Daily Workflow (Git-Based)

### Morning
```powershell
docker-compose up -d ressim-dev
# Or if image changed:
docker-compose build && docker-compose up -d ressim-dev
```

### Work
```
1. Edit files on Windows in VS Code
2. Use `docker-compose exec` for builds
3. Commit to Git
```

### End of Day
```powershell
docker-compose down
```

---

## Quick Commands

### Start
```powershell
docker-compose build ressim-dev    # First time only
docker-compose up -d ressim-dev    # Start
```

### Access
```powershell
# Shell access
docker-compose exec ressim-dev bash

# Run command
docker-compose exec ressim-dev npm run build
```

### Stop
```powershell
docker-compose down
```

### Rebuild
```powershell
docker-compose build --no-cache ressim-dev
docker-compose up -d ressim-dev
```

### View Logs
```powershell
docker logs -f ressim-dev
```

---

## Troubleshooting

### "Cannot connect to container"
```powershell
# Check if running
docker ps

# If not running, start it
docker-compose up -d ressim-dev

# Check logs
docker logs ressim-dev
```

### "Port 5173 already in use"
```powershell
# Find what's using it
netstat -ano | findstr :5173

# Kill process
taskkill /PID <PID> /F

# Or use alternate port
# Edit docker-compose.yml: 5174:5173
```

### "Need to rebuild"
```powershell
docker-compose build --no-cache ressim-dev
docker-compose up -d ressim-dev
```

### "Lost changes"
```
Solution: Commit to Git before major changes
git add .
git commit -m "Backup"
```

---

## Key Differences

| Feature | Old | New |
|---------|-----|-----|
| **Volume Mounts** | Yes | No |
| **Live Sync** | Automatic | Git-based |
| **Isolation** | Partial | Complete |
| **Build Context** | Host + Container | Container only |
| **Dev Approach** | Mount files | Copy in Dockerfile |
| **Update Method** | File sync | Git commit + rebuild |
| **Security** | Host accessible | Isolated |
| **Reproducibility** | Machine-dependent | Image-based |

---

## Getting Started Now

### Choose one:

**Option 1: VS Code Remote (Recommended)**
```powershell
# In PowerShell
cd c:\Users\serge\Repos\ressim
docker-compose build ressim-dev
# Then in VS Code: F1 → "Remote-Containers: Open Folder in Container"
```

**Option 2: Git-Based**
```powershell
cd c:\Users\serge\Repos\ressim
docker-compose build --no-cache ressim-dev
docker-compose up -d ressim-dev
# Browser: http://localhost:5173
```

**Then:**
1. Open http://localhost:5173
2. Application loads
3. Ready to develop!

---

## Questions?

- Full details: Read `ISOLATED_CONTAINER_GUIDE.md`
- VS Code Remote: https://code.visualstudio.com/docs/remote/containers
- Docker: https://docs.docker.com/compose/

---

**Status:** ✅ Ready to use
**Setup Time:** 5 minutes
**Next:** Choose Path A, B, or C above and follow steps
