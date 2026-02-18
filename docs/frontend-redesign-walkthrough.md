# Frontend Redesign — Walkthrough

## What Changed

The monolithic sidebar layout has been replaced with a **top-bar → run controls → tabbed content** design.

### New Layout Structure

```
┌─ HEADER ──────────────────────────── [☀/🌙 Theme] ─┐
├─ CATEGORY PILLS ─────────────────────────────────────┤
│  [Depletion vs. Analytical] [Waterflood vs. BL]      │
│  [Exploration Scenarios]    [⚙ Custom]               │
├─ CASE SELECTOR (sub-cases for active category) ──────┤
├─ RUN CONTROLS (▶ Run | Step | ⏹ Stop | ↻ Reinit) ──┤
├─ TABS ───────────────────────────────────────────────┤
│  [ 📊 Charts ]  [ 🧊 3D ]  [ ⚙ Inputs ]            │
│  ┌───────────────────────────────────────────────┐   │
│  │           Active tab content                  │   │
│  └───────────────────────────────────────────────┘   │
└──────────────────────────────────────────────────────┘
```

### Files Created

| File | Purpose |
|------|---------|
| [caseCatalog.js](file:///home/sergey/Repos/ressim/src/lib/caseCatalog.js) | 10 presets in 3 categories (depletion, waterflood, exploration) |
| [export-cases.mjs](file:///home/sergey/Repos/ressim/scripts/export-cases.mjs) | Pre-runs all cases via WASM, writes JSON to `public/cases/` |
| [TopBar.svelte](file:///home/sergey/Repos/ressim/src/lib/ui/TopBar.svelte) | Category pills + case selector buttons |
| [RunControls.svelte](file:///home/sergey/Repos/ressim/src/lib/ui/RunControls.svelte) | Horizontal run/stop/step/reinit bar |
| [TabContainer.svelte](file:///home/sergey/Repos/ressim/src/lib/ui/TabContainer.svelte) | 3-tab switcher (Charts, 3D, Inputs) |
| [InputsTab.svelte](file:///home/sergey/Repos/ressim/src/lib/ui/InputsTab.svelte) | 3-column grid of all parameter panels |

### Files Modified

| File | Change |
|------|--------|
| [App.svelte](file:///home/sergey/Repos/ressim/src/App.svelte) | Full rewrite — 1713→600 lines, new layout |
| [package.json](file:///home/sergey/Repos/ressim/package.json) | Added `cases:export` script |

---

## Verification Results

| Check | Result |
|-------|--------|
| `npm run build` | ✅ Success (6.3s, 137 modules) |
| `npm run cases:export` | ✅ 10/10 cases exported |
| Dev server (HTTP 200) | ✅ Serving at `localhost:5173/ressim/` |
| Browser visual test | ⚠ CDP port unavailable — please verify manually |

## Manual Testing Needed

Please open **http://localhost:5173/ressim/** and verify:

1. **Category navigation** — click each pill button, verify sub-cases appear
2. **Pre-run loading** — select a case (e.g., Depletion → Corner Producer), chart should populate instantly
3. **Custom mode** — click "⚙ Custom", verify Inputs tab activates with editable fields
4. **Tab switching** — Charts / 3D / Inputs tabs
5. **Run controls** — in Custom mode, click "▶ Run 20 Steps"
6. **Theme toggle** — light/dark switch in header
