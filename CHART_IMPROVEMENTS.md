# Chart & Diagnostic Plot — Reference

## Architecture

```
┌─────────────────────────────────────────────────────┐
│  CHART CARD                                         │
│  ┌─────────────────────────────────────────────┐    │
│  │ X-axis: [Time ▾] [Log]                      │    │
│  └─────────────────────────────────────────────┘    │
│                                                     │
│  ▼ Rates ─────────────────────────────────────      │
│  [Oil ✕] [Water ✕] [Inj ✕] …                       │
│  ┌──────────────────── canvas ──────────────┐       │
│  └──────────────────────────────────────────┘       │
│                                                     │
│  ▼ Cumulative ────────────────────────────────      │
│  [Cum Oil ✕] [RF ✕] …                              │
│  ┌──────────────────── canvas ──────────────┐       │
│  └──────────────────────────────────────────┘       │
│                                                     │
│  ▸ Diagnostics (collapsed) ───────────────────      │
│                                                     │
│  Error: 42 pts · MAE/RMSE/MAPE                      │
└─────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────┐
│  Sw PROFILE CARD  (separate, always visible)        │
└─────────────────────────────────────────────────────┘
```

**Components:**

| File | Role | Lines |
|------|------|-------|
| `RateChart.svelte` | Container: data computation, x-axis state, 3 sub-panels | ~290 |
| `ChartSubPanel.svelte` | Reusable: collapsible card, Chart.js canvas, curve toggles | ~250 |
| `SwProfileChart.svelte` | Standalone Sw profile chart (cell-index x-axis) | ~220 |
| `FractionalFlow.svelte` | Waterflood analytical (computation only, no chart) | ~260 |
| `DepletionAnalytical.svelte` | Depletion analytical (computation only) | ~220 |

## Panels & Curves

### Rates Panel (7 curves, y-axis: Rate m³/d)
| Curve | Color | Default |
|-------|-------|---------|
| Oil Rate | green | ✅ |
| Oil Rate (Analytical) | dark green, dashed | ✅ |
| Water Rate | navy | ✅ |
| Water Rate (Analytical) | blue, dashed | ✅ |
| Injection Rate | cyan | ✅ |
| Liquid Rate | blue | ⬜ |
| Oil Rate Error | green, dotted | ⬜ |

### Cumulative Panel (5 curves, y: Cumulative m³ + y1: RF)
| Curve | Color | Default |
|-------|-------|---------|
| Cum Oil | dark green | ✅ |
| Cum Oil (Analytical) | dark green, dashed | ✅ |
| Cum Injection | cyan | ✅ |
| Cum Water | navy | ✅ |
| Recovery Factor | green (y1) | ✅ |

### Diagnostics Panel (9 curves, multiple axes)
| Curve | Axis | Default |
|-------|------|---------|
| Avg Pressure | y (Pressure) | ✅ |
| Avg Pressure (Analytical) | y | ✅ |
| VRR | y1 | ⬜ |
| WOR (Sim) | y2 | ⬜ |
| WOR (Analytical) | y2 | ⬜ |
| Avg Water Sat | y3 (Fraction) | ✅ |
| Water Cut (Sim) | y3 | ⬜ |
| Water Cut (Analytical) | y3 | ⬜ |
| MB Error | y4 | ⬜ |

## X-Axis Modes (5)
| Mode | Label |
|------|-------|
| `time` | Time (days) |
| `logTime` | Log Time (Fetkovich) |
| `pvi` | PV Injected |
| `cumLiquid` | Cumulative Liquid (m³) |
| `cumInjection` | Cumulative Injection (m³) |

## Scenario Defaults
| Scenario | Panels Open |
|----------|-------------|
| Depletion | Rates + Diagnostics |
| Waterflood | Rates + Cumulative |
| Exploration | All three |

---

## TODO

### High Priority
- [ ] **Sync x-axis range** across open panels (e.g., zoom one → zoom all)
- [ ] **SwProfileChart legend consistency** — make legend pills match ChartSubPanel style
- [ ] **Consider SwProfile as 3D viz subcard** — it shows spatial information similar to the 3D view

### Medium Priority
- [x] **C2** Dimensionless normalization (q/q₀, tD)
- [ ] **C3** Export chart data as CSV
- [ ] **C5** Relative error (%) curve
- [x] **D1** Dimensionless time tD = t/τ
- [x] **D2** Pore Volumes Produced (PVP)

### Low Priority / Future
- [ ] Multi-chart synchronized zoom/pan (requires Chart.js plugins)
- [ ] Areal sweep efficiency chart (2D waterflood)
- [ ] Fetkovich type curve overlay template (standard decline curves)
- [ ] Move remaining dead code audit (ensure no orphaned helpers)
