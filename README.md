# ResSim

A reservoir simulator that runs entirely in the browser and draws every result against the
classical solution it is supposed to reproduce. Three-phase black-oil flow on a 3D Cartesian
grid — implicit-pressure (IMPES) or fully implicit — with Peaceman wells, gravity, capillarity
and correlation or tabular PVT, plotted live against Buckley–Leverett, Craig, Dykstra–Parsons,
Stiles, Dietz, Fetkovich, Arps, Havlena–Odeh and line-source well-test solutions, plus
precomputed OPM Flow runs where no closed form exists. Seventeen scenarios ship as
self-contained studies with sensitivity dimensions, scenario-appropriate charts and (where the
case is spatial) a 3D view and saturation profile. Each states what its reference assumes and
where the simulation is expected to leave it. Nothing is uploaded: the solver is WebAssembly running on the page, fonts are bundled
with the app, and there are no analytics or cookies. The selected theme is stored locally in the
browser.

https://farin.nl/ressim - No installation required

<img width="2463" height="1895" alt="image" src="https://github.com/user-attachments/assets/74e00ab8-6c5f-42fc-ae7b-5a85cb27089a" />


## What You Can Do With It

- **Watch a numerical solution meet — or leave — its analytical reference.** Every waterflood,
  sweep, depletion and gas-injection case carries the reference curve beside the simulation, on
  a shared pore-volume or time axis.
- **Ask why they differ.** Cases are built so that the departure has one cause at a time:
  grid resolution, timestep, solver formulation, capillarity, gravity, layering, or mobility
  ratio. Several ship a control run that lands back on the analytical curve, which is what makes
  the rest of the departure attributable.
- **Sweep a sensitivity.** Each scenario owns its dimensions — mobility ratio, Corey exponents,
  residual oil, gravity number, density contrast, vertical communication, drainage geometry,
  skin, PVT representation — and runs every selected variant as its own curve.
- **Check the simulator against another simulator.** Selected cases bundle a precomputed OPM
  Flow run of the identical deck, so the comparison is not only against theory.
- **See the field, not just the chart.** A 3D view of pressure, water/gas saturation,
  permeability and porosity, plus a saturation-versus-distance profile with the analytical
  front drawn on it, both stepping through the same replay timeline as the charts.
- **Read the caveats the product raises itself.** Enabled gravity, capillarity, a
  non-one-dimensional grid or an off-centre well each downgrade the analytical status and say
  so, rather than leaving a reference curve looking authoritative where it does not apply.

## How To Use It

1. **Pick a scenario** from the picker at the top. The groups say what kind of question each
   answers: 1D displacement, sweep efficiency, flow regimes and decline, material balance and
   drive, or published benchmark decks. The description below the picker states the case, its solver, and the
   reference solution it is judged against.
2. **Choose a sensitivity dimension**, then toggle the variants to include. The dimension
   description explains what the study is for; each variant chip carries what it changes.
3. **Run.** Progress and any solver warnings appear beside the controls; the run stays
   responsive because the solver executes in a Web Worker.
4. **Read the comparison.** Simulation curves are solid, analytical references dashed, and any
   external reference (published data or OPM Flow) dotted. Panels can be expanded individually,
   and the x-axis switches between time, pore volumes injected and cumulative injection.
5. **Inspect the field.** The 3D view and the profile plot below it replay the same timeline.
   For a flooded case the profile shows the simulated saturation against the Buckley–Leverett
   front at that moment.
6. **Change parameters** in the sections below the charts to depart from the shipped case; the
   analytical status updates to tell you when a reference has stopped being applicable.

## Scenario Inventory

| Catalog group | Scenario | Key | Primary reference / purpose |
|---|---|---|---|
| 1D Displacement — Buckley–Leverett | 1D Waterflood | `wf_bl1d` | Buckley–Leverett + Welge analytical reference across mobility ratio, oil Corey exponent and residual oil |
| 1D Displacement — Buckley–Leverett | 1D Waterflood — Capillary Effects | `wf_capillary` | Departure from the zero-capillary BL limit; physical versus numerical front spreading; OPM Flow run of the identical model, capillary pressure included |
| 1D Displacement — Buckley–Leverett | Gravity-Stable vs Unstable Displacement | `wf_gravity_stability` | 1D vertical column flooded upward or downward; gravity along the flow path brackets the viscous BL curve instead of bounding it; OPM Flow run of the identical model |
| 1D Displacement — Buckley–Leverett | Numerical Dispersion & Convergence | `wf_numerics` | The case where no BL assumption is broken, so the whole gap is the grid: first-order convergence over a 40x cell-size range, the IMPES stability limit made visible, IMPES vs FIM, and OPM Flow runs at two resolutions |
| 1D Displacement — Buckley–Leverett | Gas Injection | `gas_injection` | Gas-oil fractional-flow breakthrough; OPM Flow run of the identical model |
| Sweep Efficiency | Areal Sweep | `sweep_areal` | Craig confined five-spot correlation; OPM Flow run of the identical model |
| Sweep Efficiency | Vertical Sweep | `sweep_vertical` | Dykstra–Parsons / Stiles layered sweep; OPM Flow run of the identical model |
| Sweep Efficiency | Layer Crossflow — Do the Layers Talk? | `sweep_crossflow` | Dykstra–Parsons' non-communicating assumption tested directly: a k_v/k_h ladder the correlation cannot see, a crossflow benefit that reverses sign with mobility ratio, and capillary crossflow that needs a path; OPM Flow run of the base case |
| Sweep Efficiency | Combined Sweep | `sweep_combined` | Combined areal and vertical contact with selectable layered correlation; OPM Flow run of the base case |
| Sweep Efficiency | Gravity Override (Dietz Tongue) | `wf_gravity` | Vertical sweep lost to a gravity tongue; gravity-off control returns to BL, rate/density/k_z/completion ladders leave it; OPM Flow cross-check of the base case |
| Flow Regimes & Decline | Transient Radial Flow (Theis) | `dep_welltest` | Line-source drawdown before boundaries are felt; permeability, skin, and near-well grid bias |
| Flow Regimes & Decline | Drainage Geometry & Productivity (Dietz) | `dep_pss` | Equal-area drainage geometries and well positions; C_A recovered from the measured PSS drawdown |
| Flow Regimes & Decline | Boundary-Dominated Decline (Fetkovich) | `dep_decline` | Finite-slab transient, boundary arrival, and asymptotic decline |
| Flow Regimes & Decline | Layered Depletion (Arps) | `dep_arps` | Spatial layered depletion approaching a late-time Dietz/Fetkovich superposition, plus a crossflow limitation study |
| Material Balance & Drive Mechanism | Gas Reserves from p/z | `dep_gas_pz` | Dry-gas depletion against the p/z material-balance straight line; pore compressibility, compartmentalisation and how much history you have each inflate the reserves estimate; OPM Flow cross-check at both ends of the compressibility ladder |
| Material Balance & Drive Mechanism | Solution Gas Drive | `gas_drive` | Saturated black-oil depletion — liberation, free-gas build-up and the GOR rise; graded against an OPM Flow reference (`docs/THREE_PHASE_VALIDATION.md` §2) |
| Material Balance & Drive Mechanism | PVT Model Risk — One Calibration Point | `dep_pvt` | Constant-rate black-oil blowdown; every PVT table shares the one point a flash test pins. Above it, unmeasured undersaturated compressibility doubles the time to the bubble point; below it, the published Rs(p) correlation spreads producing GOR by ±20% at matched pressure. OPM Flow on every rung |
| Published Benchmark Decks | SPE1 Black-Oil Benchmark | `spe1_gas_injection` | Published Eclipse and OPM Flow comparative-solution references |
| Compositional *(defined and tested; withheld from picker)* | 1D Compositional CO₂ Flood | `comp_co2_1d` | Peng–Robinson compositional engine; withheld until the chart stack can plot compositional series ([issue #29](https://github.com/sergeyfarin/ressim/issues/29)) |

## Reading The Results — Model Validity Notes

What each reference is entitled to claim, and where it stops.

- Buckley-Leverett is a 1D immiscible displacement reference. Do not interpret it as a general areal or heterogeneous-field predictor.
- Craig areal sweep applies to confined five-spot style pattern assumptions. It is context, not a universal areal flood model.
- Dykstra-Parsons assumes layered, non-communicating flow. When the simulator allows vertical communication, analytical sweep penalties are intentionally conservative.
- Stiles-style combined sweep improves layered recovery interpretation, but it is still an analytical teaching aid rather than a substitute for full streamline or field-scale pattern modeling.
- Three-phase mode is graded quantitatively against numerical references (OPM Flow, SPE1), not promoted tank-model overlays. A Tarner–Tracy model was evaluated for Solution Gas Drive but rejected because its uniform tank assumptions do not represent the case's localized BHP drawdown and initially mobile free gas. Vaporized oil (Rv) is not modelled, so wet-gas and gas-condensate behavior is outside the envelope. See `docs/THREE_PHASE_VALIDATION.md` section 6.
- Material-balance closure is reported explicitly for all three phases. FIM and three-phase IMPES transport every component mass, so their oil balance is a genuine conservation check. Two-phase IMPES still keeps oil as the residual saturation (S_o = 1 - S_w) and moves water by volume on a fixed pore volume, so with rock or water compressibility it books their expansion as oil. See `docs/BLACK_OIL_VALIDATION.md` §3–4.
- The Brooks-Corey capillary model is numerically capped at `20 x P_entry`. That cap is a stability safeguard, not a physical plateau.
- Rock compressibility changes pore volume, not cell geometry. FIM and three-phase IMPES evaluate pore volume at the new pressure; two-phase IMPES transports on a fixed pore volume (see the material-balance item above).
- Water density and viscosity are pressure-independent. This is adequate for the reservoir pressure and temperature ranges targeted by this simulator.
- IMPES seeds its pressure solve with an oil compressibility: the scalar undersaturated `c_o` in two-phase mode, and in three-phase mode an effective value from the bubble-point curve, blended over the last 5 bar above the bubble point. In three-phase mode this is only the first guess. A volume-balance iteration then takes storage from the mass closure itself, so the approximation does not reach the conserved masses.
- A PVT table whose `Bo` rises faster than `Bg·dRs/dp` below the bubble point is thermodynamically unstable. The run is allowed, with a pre-run warning that names the pressure range.
- Numerical derivatives of PVT properties (effective gas compressibility, saturated Bo/Bg slopes used in three-phase accumulation) use a fixed 1-bar finite-difference step. Accuracy degrades below roughly 5 bar, which the pressure floor prevents from being reached in practice.

## Quick Start

### Prerequisites

- Node.js 24 (the CI version) with `pnpm`
- Rust toolchain
- `wasm-pack`
- `wasm32-unknown-unknown` target

### Install

```bash
pnpm install
rustup target add wasm32-unknown-unknown
cargo install wasm-pack
```

### Run

```bash
pnpm run dev
```

`src/lib/ressim/pkg/` (the wasm-bindgen output) is generated and not committed. `pnpm run dev`,
`build`, `typecheck` and the `test*` scripts all build it first; `scripts/build-wasm.sh` skips the
work when the output is already newer than the Rust sources, so the hook is close to free once
warm.

### Validate

```bash
pnpm run validate           # frontend: typecheck + lint + cycles + fast tests + build
pnpm run validate:product   # + full vitest + Rust IMPES solver bucket
pnpm run validate:full      # + all Rust solver buckets (shared + FIM + IMPES)
bash scripts/validate-solver-coverage.sh all   # Rust solver test buckets on their own
cargo test --manifest-path src/lib/ressim/Cargo.toml benchmark_buckley   # physics benchmark
```

Pull-request CI runs the same ground as `validate:full`, plus the Buckley-Leverett benchmarks, the
compositional thermodynamics gate and the two gates below. The `#[ignore]`d release replays and
the wasm control matrix stay out of PR CI and are run explicitly;
`.claude/skills/ressim-validation/SKILL.md` lists them and says when each applies.

Two gates sit outside `validate:*` because they need more than a Rust toolchain and Node:

```bash
bash scripts/validate-native-binding.sh   # native (PyO3) vs browser bindings on a case matrix
pnpm run test:deployed                    # Playwright, against `pnpm run preview`
```

Both run in PR CI. The first builds `crates/ressim-py` against the engine with its browser
bindings switched off and compares the two clients. They agree bit for bit: the engine's
transcendental math goes through one implementation on both targets (#62). The second is the only check that would catch an unstyled page, because a Tailwind
content-glob miss raises no error anywhere.

One gate is local only, because CI has no OPM Flow. Run it after any change that can move an
answer:

```bash
bash scripts/validate-cross-solver.sh     # FIM and IMPES vs OPM Flow on the small-direct decks
```

Full `cargo test` is not used as a gate, because FIM diagnostic tests can dominate its runtime
(see `.claude/skills/ressim-validation/SKILL.md`).

## Implemented Capabilities

### Status

- 18 scenarios are offered in the picker across five physics-question groups. One more,
  `comp_co2_1d`, is defined and tested but withheld from the picker (see the inventory above).
- Every scenario declares its solver and says why. Gas, black-oil and capillary cases run FIM by
  default (`gas_injection`, `gas_drive`, `spe1_gas_injection`, `dep_gas_pz`, `dep_pvt`,
  `wf_capillary`). The
  other oil/water cases run IMPES. `wf_numerics` runs the two side by side.
- OPM Flow references are precomputed offline and bundled as nineteen parsed artifacts. Every
  simulation in the browser runs in local WebAssembly.
- The engine also builds as a native Python module (`crates/ressim-py`, PyO3), with its browser
  bindings switched off.

### Flow Physics

- Two solvers on a 3D Cartesian grid with per-layer cell thickness: IMPES (implicit pressure,
  explicit transport) and a fully implicit (FIM) Newton solver with automatic differentiation, a
  sparse direct route for small systems and CPR-preconditioned GMRES for larger ones.
- Two-phase oil/water flow with Corey relative permeability.
- Optional Brooks-Corey oil-water and oil-gas capillary pressure.
- Optional gravity with density-weighted hydrostatic head.
- Three-phase oil/water/gas flow with Stone II oil relative permeability, gas Corey curves or tabular SWOF/SGOF. Three-phase IMPES transports all four black-oil masses and recovers the cell state with a flash, so it conserves oil and gas as well as water.
- Correlation-based or tabular black-oil PVT support with bubble-point tracking, Rs liberation/re-dissolution, pressure-dependent mobility, and producing GOR reporting.
- Peaceman-style well model with BHP or rate control, per-layer completion, dynamic PI updates, and injector / producer switching logic. Well PI uses per-layer cell thickness.
- Eclipse-style wellbore datum: a well's BHP is quoted at a datum depth (default the shallowest completion) and carried down to each completion by a wellbore column whose density is derived from the completion fluids, or fixed per well. Active only when gravity is enabled.
- Per-layer initial conditions: water saturation, gas saturation, and cell thickness can be specified per z-layer for scenarios with gas caps or non-uniform geology.
- Adaptive timestep checks based on saturation change, pressure change, and well-rate change
  limits, plus, in IMPES, a limit on the dissolved-gas change per substep.
- A separate isothermal compositional engine (Peng–Robinson EOS, two or three components, phase
  appearance and disappearance, compositional wells). It is validated natively against OPM's
  compositional simulator and not yet offered in the app.

### Analytical and Diagnostic Surfaces

- Buckley-Leverett fractional-flow reference curves with Welge shock construction, in time, pore-volume and spatial-profile form.
- Craig areal sweep, Dykstra-Parsons vertical sweep, and Stiles-style combined sweep interpretation.
- Dietz depletion, Fetkovich exponential decline, and Arps decline overlays.
- Line-source (exponential-integral) well-test drawdown with semilog slope fitting for permeability and skin.
- Havlena-Odeh material-balance terms and drive indices in depletion diagnostics.
- p/z-style gas diagnostics and producing GOR outputs for gas-oriented cases.

### UI and Workflow

- Scenario-first case selection through `ScenarioPicker.svelte`, with scenario-owned parameters,
  sensitivity dimensions, per-variant run sweeps and references.
- Worker-based execution to keep the UI responsive.
- 3D scalar visualization for pressure, water saturation, gas saturation, permeability, and porosity.
- Shared chart layout system for runtime and comparison views.

## Benchmarks

Current scorecard: per area, the banded criterion closest to its band, the largest gap to a
reference solving the same discrete model, and the commit it was measured on. The table is generated from `docs/benchmarks/benchmarks.json` by
`bash scripts/benchmarks.sh render`; every row, its band and its replay command are in
[`docs/BENCHMARKS.md`](docs/BENCHMARKS.md), which is the authoritative record.

<!-- GENERATED:summary -->
| Area | Reference | Tightest criterion | Band | Band used | Largest same-model gap | Measured on |
|---|---|---|---|---|---|---|
| 1D waterflood breakthrough | Buckley-Leverett + Welge | BL-Case-A nx=24: breakthrough_rel_err 9.44 % | 15.0 % | 63% | — | `ca747c1` |
| SPE1 Case 1, 10 years | Published SPE1 / Flow | 10x10x3: gor_rel_err 1.38 % | 12.0 % | 11% | 10x10x3: vs_flow.WGOR_rel_err -0.85 % (explained) | `ca747c1` |
| Three-phase gas drive and injection | OPM Flow | gas_drive: gor_rel_err 0.56 % | 1.0 % | 56% | gas_drive: vs_jutul.FOPR_rel_err +4.49 % (explained) | `ca747c1` |
| Black-oil depletion column | Grid self-convergence, Flow | FIM: sat_gas_finest_pair_gap 1.20 % | 1.5 % | 80% | — | `ca747c1` |
| Generated decks, three solvers | OPM Flow, generated decks | no banded criterion | — | — | sweep-combined: sparse.cum.FWPT +0.45 % | `ca747c1` |
| Native vs wasm bindings | Each other | buckley-impes (IMPES): cells_max_abs_diff 0 abs | 1e-09 abs | 0% | — | `ca747c1` |
| FIM convergence, long horizons | Substeps per report step | no banded criterion | — | — | — | `ca747c1` |
| Compositional, matched timestep | OPM flowexp_comp | 1D plain: worst_pressure_diff 1.68 bar | 2 bar | 84% | 1D plain: worst_pressure_diff 1.68 bar (explained) | `ca747c1` |
| Second simulator on the same decks | JutulDarcy vs Flow and FIM | no banded criterion | — | — | dep-pvt-lab-report: fim_vs_jutul.final_FGPR_rel_err +1.31 % (explained) | `ca747c1` |
<!-- /GENERATED:summary -->

Scenario tests (`pnpm run test:scenarios`) also grade each case in the picker against its own
reference, and analytical-contract tests check that every dimension marked `affectsAnalytical`
actually moves the analytical curve.

## Why The Roadmap Is Ordered This Way

The next priorities follow standard reservoir-engineering practice:

- Comparative-solution benchmarking should precede more physics expansion for black-oil and three-phase work.
- Analytical methods should only be exposed where their assumptions remain explicit and defensible.
- Relative permeability, PVT, and sweep-method interpretation dominate uncertainty more than UI breadth does.

That ordering aligns with the literature already used in the project: Buckley and Leverett, Welge, Craig, Dykstra and Parsons, Stiles, Dietz, Fetkovich, Arps, Havlena and Odeh, and the SPE comparative-solution tradition used for simulator validation.

## Project Layout

```text
src/
  App.svelte          # the application's page
  app.css
  main.ts
  pages/              # additional entry points, built from the packages below
    fractionalFlow/
  lib/
    analytical/       # @ressim/analytical  - reference solutions
    charts/           # @ressim/charts      - Chart.js panels and chart models
    primitives/       # @ressim/primitives  - presentational controls, no domain vocabulary
    quantities/       # @ressim/quantities  - derived run series and in-place volumes
    presets/          # @ressim/presets     - the preset/scenario editability contract
    catalog/          # scenario definitions and bundled OPM Flow artifacts
    compositional/    # compositional run payloads, series and quantities
    physics/
    ressim/           # the Rust/WASM engine
    scenario/
    stores/
    ui/
    visualization/
    workers/
crates/
  ressim-py/          # PyO3 bindings: the engine without its browser bindings
    parity/           # native/wasm cross-client gate
    ressim_quantities/# run quantities for notebooks, from the shared contract
contracts/
  run-quantities.json # GENERATED from the TypeScript registry; the shared data contract
docs/                 # authoritative + active working docs (see DOCUMENTATION_INDEX.md)
  BENCHMARKS.md       # current benchmark scorecard
  DOCUMENTATION_INDEX.md
  ...
opm/                  # OPM Flow reference decks, the small-direct scorecard, compositional fixtures
tools/                # offline OPM Flow pipeline (Python, run with uv)
.archive/             # superseded experiments, closed plans, historical snapshots
  docs/               # (git-tracked, reversible; see .archive/README.md)
ROADMAP.md
TODO.md
```

The five `@ressim/*` directories are pnpm workspace packages, imported by name rather than by
relative path. `src/lib/packageBoundaries.test.ts` enforces that: a relative import reaching into
a package from outside fails, because a package name without an enforced boundary is a naming
convention that decays the first time someone types a path out of habit.

`fractional-flow.html` is a second page built only from those packages — no store, no scenario
catalog, no worker, no WASM. It exists so that "the frontend can be composed into more than one
page" is demonstrated rather than asserted; see `docs/ARCHITECTURE_SPLIT_PLAN_2026-09-19.md`.

## Documentation Map

| Document | Purpose |
|----------|---------|
| [GitHub Issues](https://github.com/sergeyfarin/ressim/issues) | Actionable work, priorities, acceptance criteria, and status |
| `ROADMAP.md` | Strategic ordering and links to active issues |
| `docs/BENCHMARKS.md` | Current benchmark scorecard: every reference, band, measured error and replay command |
| `docs/P4_TWO_PHASE_BENCHMARKS.md` | Buckley-Leverett benchmark methodology and tolerance policy |
| `docs/BLACK_OIL_VALIDATION.md` | SPE1 acceptance criteria, black-oil grid convergence, solver safeguards |
| `docs/THREE_PHASE_VALIDATION.md` | Three-phase exit criteria, OPM Flow / SPE1 acceptance, phase-closure diagnostics |
| `docs/COMPOSITIONAL_VALIDATION.md` | Compositional fluid dataset, oracles, acceptance contract and gate status |
| `docs/FIM_STATUS.md` | FIM solver state, convergence history and source map |
| `docs/OPEN_ITEMS_2026-09-21.md` | Deliberate deferrals and known gaps, each with why and what would close it |
| `docs/SCIENTIFIC_LIMITATIONS.md` | What the software is not qualified for |
| `docs/THREE_PHASE_IMPLEMENTATION_NOTES.md` | Three-phase implementation details and parameter reference |
| `docs/UNIT_SYSTEM.md` | Unit conventions, equations, and PVT / solver notes |
| `docs/ARCHITECTURE_NOTES.md` | Current architecture direction and unresolved design decisions |
| `docs/DOCUMENTATION_INDEX.md` | Which documents are authoritative vs historical |

## Near-Term Focus

The limited public release is deployed. See `ROADMAP.md` for current ordering. The next
priorities are:

1. Close the remaining SPE1 and black-oil scenario validation gaps
   ([#12](https://github.com/sergeyfarin/ressim/issues/12)) and the chart presentation defects
   ([#15](https://github.com/sergeyfarin/ressim/issues/15)).
2. Bring the compositional engine into the app: chart sourcing for `comp_co2_1d`
   ([#29](https://github.com/sergeyfarin/ressim/issues/29)), then SPE5 and SPE3
   ([#52](https://github.com/sergeyfarin/ressim/issues/52)).
3. Add scenario enablers only with consuming cases and independent references.
4. Keep the FIM OPM-parity frontier parked behind product validation unless a user-visible defect
   requires it.

## License

ResSim is licensed under the [GNU Affero General Public License v3.0 only](LICENSE).
IBM Plex Sans and IBM Plex Mono are bundled through Fontsource and remain licensed under the
SIL Open Font License 1.1; their license texts are included in the installed font packages.
