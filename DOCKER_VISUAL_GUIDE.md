# 🐳 Docker Setup - Visual Quick Guide

**For:** Ressim Project (Rust + WASM + Svelte + Vite)
**Platform:** Windows 11 → Linux Container
**Time:** 5-10 minutes

---

## The Journey: Windows → Container

```
┌────────────────────────────────────────────────────────────┐
│ BEFORE (Windows Development - Your Old Setup)             │
├────────────────────────────────────────────────────────────┤
│                                                            │
│  Windows 11                                               │
│  ├── Node.js (global)          ──┐                        │
│  ├── Rust (global)             ──┼─→ Possible conflicts  │
│  ├── Dependencies scattered    ──┤   System pollution    │
│  ├── Dev environment           ──┘   Not reproducible    │
│  └── Your project files                                  │
│                                                            │
│  ❌ Problems:                                              │
│  - Package version conflicts                             │
│  - Works on my machine syndrome                          │
│  - Hard to clean up                                      │
│  - Team members struggle                                 │
│  - Production deployment risky                           │
│                                                            │
└────────────────────────────────────────────────────────────┘

                         ⬇️  MIGRATION  ⬇️

┌────────────────────────────────────────────────────────────┐
│ AFTER (Docker Container Development - New Setup)         │
├────────────────────────────────────────────────────────────┤
│                                                            │
│  Windows 11 (Your Machine)         Linux Container       │
│  ┌──────────────────────┐        ┌──────────────────────┐│
│  │ VS Code              │        │ /app (your project)  ││
│  │ ├─ src/             │◀─────→ │ ├─ src/              ││
│  │ │  ├─ App.svelte    │ volume │ │  ├─ App.svelte     ││
│  │ │  └─ lib/          │ mount  │ │  └─ lib/           ││
│  │ └─ package.json     │        │ └─ package.json      ││
│  │                      │        │                      ││
│  │ Browser at:5173     │◄───────┤ Vite dev server:5173 ││
│  │ (auto-refresh)      │  HMR   │ (Rust+WASM+Node)    ││
│  └──────────────────────┘        └──────────────────────┘│
│                                   Container runtime:    │
│                                   ├─ Rust toolchain     │
│                                   ├─ Node.js            │
│                                   ├─ wasm-pack          │
│                                   └─ All dependencies   │
│                                                            │
│  ✅ Benefits:                                              │
│  + Isolated environment                                  │
│  + No Windows conflicts                                 │
│  + Reproducible everywhere                             │
│  + Easy to reset (delete container)                    │
│  + Same as production                                  │
│  + Team members get instant setup                      │
│                                                            │
└────────────────────────────────────────────────────────────┘
```

---

## Setup Flow

```
START HERE
    │
    ├─→ 1️⃣  VERIFY PREREQUISITES
    │   • Docker Desktop running? (docker ps)
    │   • Project files present?
    │   • 10GB disk space?
    │   └─→ ✅ All OK? Continue
    │
    ├─→ 2️⃣  RUN SETUP SCRIPT
    │   • .\docker-setup.ps1
    │   • Wait 3-5 minutes
    │   • First build is slow (pulls large images)
    │   • Subsequent builds: 10-30 seconds
    │   └─→ ✅ Build complete? Continue
    │
    ├─→ 3️⃣  VERIFY CONTAINER RUNNING
    │   • docker ps | grep ressim-dev
    │   • Should show: STATUS = Up
    │   • Should show: PORTS = 5173:5173
    │   └─→ ✅ Container running? Continue
    │
    ├─→ 4️⃣  OPEN APPLICATION
    │   • Browser: http://localhost:5173
    │   • Should load: Ressim UI
    │   • Check: No console errors
    │   └─→ ✅ UI loads? Continue
    │
    ├─→ 5️⃣  TEST LIVE RELOAD
    │   • Edit: src/App.svelte (any change)
    │   • Save: Ctrl+S
    │   • Watch: Browser auto-refreshes
    │   • Verify: Change appears
    │   └─→ ✅ Live reload works? DONE!
    │
    └─→ 🎉 SETUP COMPLETE - START DEVELOPING!
```

---

## Development Workflow

```
┌─────────────────────────────────────────────────────────────┐
│ DAILY DEVELOPMENT CYCLE                                    │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  Morning:                                                  │
│  ┌─────────────────────────────────────┐                  │
│  │ docker-compose up -d ressim-dev     │                  │
│  │ (Start container)                   │                  │
│  └─────────────────────────────────────┘                  │
│           │                                               │
│           ↓                                               │
│  Browser: http://localhost:5173                           │
│           │                                               │
│           ↓                                               │
│  ┌─────────────────────────────────────┐                  │
│  │ Edit Code (VS Code on Windows)      │                  │
│  │ • src/App.svelte                    │                  │
│  │ • src/lib/ressim/src/lib.rs         │                  │
│  │ • src/lib/Counter.svelte            │                  │
│  └─────────────────────────────────────┘                  │
│           │ (save: Ctrl+S)                               │
│           ↓                                               │
│  ┌─────────────────────────────────────┐                  │
│  │ Container Detects Change            │                  │
│  │ • Volume mount watches files        │                  │
│  │ • Vite HMR triggered               │                  │
│  │ • Browser auto-refreshes           │                  │
│  └─────────────────────────────────────┘                  │
│           │ (1-2 seconds)                                │
│           ↓                                               │
│  Browser Shows Changes                                    │
│           │                                               │
│           ↓ (repeat for each change)                      │
│                                                             │
│  Evening:                                                  │
│  ┌─────────────────────────────────────┐                  │
│  │ docker-compose down                 │                  │
│  │ (Stop container, clean up)          │                  │
│  └─────────────────────────────────────┘                  │
│                                                             │
│  ✅ No manual rebuilds!                                     │
│  ✅ No package installation!                               │
│  ✅ Just edit → save → see changes!                        │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

---

## File Organization

```
📁 c:\Users\serge\Repos\ressim\

📄 DOCKER FILES (Infrastructure):
├── 🐳 Dockerfile                    ← Container image definition
├── 🐳 docker-compose.yml            ← Orchestration config
├── 🐳 .dockerignore                 ← Build optimization
└── 🐳 docker-setup.ps1              ← Setup automation

📚 DOCUMENTATION (Guides):
├── 📖 DOCKER_QUICK_START.md         ← Start here! (5 min read)
├── 📖 DOCKER_COMPLETE_SUMMARY.md    ← This visual guide
├── 📖 DOCKER_SETUP_GUIDE.md         ← Detailed (30 min read)
├── 📖 DOCKER_FILES_INDEX.md         ← Index & overview
└── 📖 DOCKER_MIGRATION_CHECKLIST.md ← Step-by-step checklist

💻 YOUR PROJECT (Unchanged):
├── 📁 src/
│   ├── 📄 App.svelte                (main UI)
│   ├── 📄 main.js                   (entry point)
│   └── 📁 lib/
│       ├── 📄 Counter.svelte
│       └── 📁 ressim/
│           ├── 📄 Cargo.toml        (Rust deps)
│           ├── 📁 src/
│           │   └── 📄 lib.rs        (Rust source)
│           └── 📁 pkg/              (Generated WASM)
│
├── 📄 package.json                  (Frontend deps)
├── 📄 vite.config.js                (Vite config)
├── 📄 svelte.config.js              (Svelte config)
└── 📄 index.html                    (HTML entry)
```

---

## Command Reference Card

```
╔══════════════════════════════════════════════════════════╗
║              DOCKER COMMANDS CHEAT SHEET                ║
╚══════════════════════════════════════════════════════════╝

🚀 STARTUP
  .\docker-setup.ps1                    Setup everything
  docker-compose up -d ressim-dev       Start container
  docker-compose down                   Stop container

📋 INFORMATION
  docker ps                             List running containers
  docker logs -f ressim-dev             View live logs
  docker stats ressim-dev               Resource usage

🐚 INTERACTIVE
  docker-compose exec ressim-dev bash   Shell access
  docker-compose exec ressim-dev cargo test      Run tests
  docker-compose exec ressim-dev npm run build   Build frontend

🔨 BUILDING
  docker-compose build --no-cache       Rebuild image
  docker-compose exec ressim-dev wasm-pack build src/lib/ressim

🧹 CLEANUP
  docker system prune -a                Remove unused
  docker volume prune                   Clean volumes
  docker image prune -a                 Remove old images

ℹ️ HELP
  .\docker-setup.ps1 -Status           Check status
  .\docker-setup.ps1 -Stop             Stop only
  .\docker-setup.ps1 -Rebuild          Force rebuild
```

---

## File Sync Visualization

```
┌─────────────────────────────────────────────────────────┐
│ HOW FILES SYNC BETWEEN WINDOWS AND CONTAINER           │
├─────────────────────────────────────────────────────────┤
│                                                         │
│ Windows Side (Host)          Container Side (Linux)    │
│ ─────────────────────────────────────────────────────  │
│                                                         │
│ 1. You edit file in VS Code                            │
│    c:\Users\serge\Repos\           │                  │
│    ressim\src\App.svelte           │                  │
│                                     │                  │
│ 2. File saved to Windows disk       │                  │
│    [actual bytes updated]           │                  │
│                                     ↓                  │
│ 3. Volume mount syncs               │ /app/src/App.svelte
│    (happens instantly!)             │ [same bytes]
│                                     │
│ 4. Vite watches /app/src/           │
│    HMR triggered                    │ Vite dev server
│                                     │ rebuilds module
│ 5. Browser connected to HMR         │
│    Receives update signal           │ Sends update
│                                     ↑
│ 6. Browser patches component       │ (WebSocket)
│    Live update in browser!          │
│                                     │
│ ✅ Result: You see changes in <1 second!              │
│                                                         │
└─────────────────────────────────────────────────────────┘
```

---

## Architecture Layers

```
┌─────────────────────────────────────────────────────┐
│ LAYER 1: PRESENTATION (Browser)                    │
├─────────────────────────────────────────────────────┤
│ • http://localhost:5173                            │
│ • Svelte UI components                             │
│ • WebGL 3D rendering                               │
└─────────────────────────────────────────────────────┘
                      ↕ (HTTP/WebSocket/HMR)
┌─────────────────────────────────────────────────────┐
│ LAYER 2: FRONTEND RUNTIME (Vite Dev Server)        │
├─────────────────────────────────────────────────────┤
│ • Node.js runtime                                  │
│ • Vite bundler                                     │
│ • HMR (Hot Module Replacement)                     │
│ • Serves JavaScript/CSS                            │
└─────────────────────────────────────────────────────┘
                      ↕ (WASM import)
┌─────────────────────────────────────────────────────┐
│ LAYER 3: WASM RUNTIME (WebAssembly Module)         │
├─────────────────────────────────────────────────────┤
│ • pkg/simulator.js (wasm bindings)                 │
│ • pkg/simulator_bg.wasm (compiled binary)          │
│ • Running physics simulator                        │
│ • Grid calculations, pressure solve, etc.          │
└─────────────────────────────────────────────────────┘
                      ↕ (Compilation)
┌─────────────────────────────────────────────────────┐
│ LAYER 4: RUST COMPILER (Build Stage)               │
├─────────────────────────────────────────────────────┤
│ • Rust source: src/lib/ressim/src/lib.rs           │
│ • Cargo compiler                                   │
│ • Dependencies from Cargo.toml                     │
│ • Output: WebAssembly binary                       │
└─────────────────────────────────────────────────────┘
                      ↕ (Docker build)
┌─────────────────────────────────────────────────────┐
│ LAYER 5: CONTAINER INFRASTRUCTURE (Docker)         │
├─────────────────────────────────────────────────────┤
│ • Linux operating system (Ubuntu base)             │
│ • Rust toolchain                                   │
│ • Node.js + npm                                    │
│ • wasm-pack compiler                               │
│ • Volume mounts for file sync                      │
│ • Port 5173 exposed to host                        │
└─────────────────────────────────────────────────────┘
                      ↕ (Docker Desktop)
┌─────────────────────────────────────────────────────┐
│ LAYER 6: HOST MACHINE (Windows 11)                 │
├─────────────────────────────────────────────────────┤
│ • Docker Desktop manages container                 │
│ • WSL 2 backend for container runtime              │
│ • File system shared via volume mounts             │
│ • Network access to :5173                          │
└─────────────────────────────────────────────────────┘

All layers communicate seamlessly!
Edit code → Browser shows changes → Physics updates
```

---

## Time Estimates

```
┌─────────────────────────────────────────────────────┐
│ ONE-TIME SETUP                                      │
├─────────────────────────────────────────────────────┤
│ Read docs ..................... 15-30 min          │
│ Run setup script ............... 5 min              │
│ First build .................... 3-5 min            │
│ Verify everything .............. 2 min              │
│                                 ─────────────       │
│ TOTAL FIRST TIME ............... 25-42 min         │
└─────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────┐
│ DAILY USAGE                                         │
├─────────────────────────────────────────────────────┤
│ Start container ................ <1 sec             │
│ Open browser ................... 1 sec              │
│ Edit & save file ............... <1 sec             │
│ Browser auto-refresh ........... 1-2 sec            │
│ Stop container ................. <1 sec             │
│                                                     │
│ NO MANUAL BUILDS! Just edit!                        │
└─────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────┐
│ REBUILD OPERATIONS                                  │
├─────────────────────────────────────────────────────┤
│ Container start ................ 10-15 sec          │
│ Subsequent builds (cached) ..... 10-30 sec          │
│ Full rebuild (no cache) ........ 2-3 min            │
│ First build (initial) .......... 3-5 min            │
└─────────────────────────────────────────────────────┘
```

---

## Decision Tree: When to Use What

```
Do I need to...?

├─ START DEVELOPING
│  └─→ .\docker-setup.ps1
│
├─ VIEW WHAT'S RUNNING
│  └─→ docker ps
│
├─ SEE ERRORS/LOGS
│  └─→ docker logs -f ressim-dev
│
├─ STOP EVERYTHING
│  └─→ docker-compose down
│
├─ RUN COMMANDS INSIDE CONTAINER
│  ├─→ Test: docker-compose exec ressim-dev cargo test
│  ├─→ Build: docker-compose exec ressim-dev wasm-pack build...
│  └─→ Shell: docker-compose exec ressim-dev bash
│
├─ CLEAN UP UNUSED IMAGES
│  └─→ docker system prune -a
│
└─ CHECK WHAT'S USING RESOURCES
   └─→ docker stats ressim-dev
```

---

## Success Indicators

✅ You're successful when:

```
☑ docker ps shows: ressim-dev Status=Up
☑ Browser loads: http://localhost:5173 (no errors)
☑ Page shows: Ressim simulator interface
☑ Edit file: Changes appear in browser instantly
☑ Logs show: "Vite v7.1.7 ready in XXms"
☑ Can type: docker-compose exec ressim-dev bash
☑ Terminal responds: root@...:/app#
```

---

## Next Steps (Recommended Order)

```
1️⃣  Read DOCKER_QUICK_START.md (5 minutes)
2️⃣  Run .\docker-setup.ps1 (wait for build)
3️⃣  Open http://localhost:5173 in browser
4️⃣  Edit src/App.svelte and save
5️⃣  Watch browser auto-refresh
6️⃣  Celebrate! 🎉
7️⃣  Read DOCKER_SETUP_GUIDE.md for details
8️⃣  Commit Docker files to git
9️⃣  Share with team members
```

---

## You're All Set! 🚀

Everything you need is ready:

✅ Docker configuration (Dockerfile + compose)
✅ Setup automation (PowerShell script)
✅ Comprehensive documentation (1000+ lines)
✅ Visual guides and checklists
✅ Troubleshooting solutions

**Just run:** `.\docker-setup.ps1`

**Then enjoy containerized development!** 🐳

---

Created: 2025-10-26
Status: ✅ Ready to Use
