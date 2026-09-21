# Engine payload boundary — design note

Date: 2026-09-21. Base: `e24861b`. **This is a pre-implementation design note, not a record of
completed work.** It proposes how to make the engine's remaining 12 `JsValue`-bound functions
reachable from any target, and argues for one option over the alternatives.

Owner of the task sequence it feeds: `ARCHITECTURE_SPLIT_PLAN_2026-09-19.md` (S4 enumerated these
12; S5 demonstrated that they are what blocks a real native workflow).

## 1. The diagnosis: these are three problems, not twelve

Read as a group rather than a list, the 12 functions collapse:

| Kind | Count | What the body actually is | Functions |
|---|---|---|---|
| **A. Serialize already-`Serialize` state** | 7 | `serde_wasm_bindgen::to_value(&self.field).unwrap()` | `get_well_state`, `get_rate_history`, `get_rate_history_since`, `get_latest_rate_point`, `get_last_fim_step_stats`, `get_fim_step_stats_history`, `get_dimensions` |
| **B. Deserialize into already-`Deserialize` types** | 4 | `serde_wasm_bindgen::from_value(js)?` then apply | `set_pvt_table`, `set_three_phase_scal_tables`, `set_sweep_config`, `load_state` |
| **C. A zero-copy bundle** | 1 | `Float64Array::view` over five engine buffers, assembled with `Reflect::set` | `get_grid_state` |

**Eleven of the twelve are not wasm-specific at all.** The Rust data on both sides of the call is
already `Serialize`/`Deserialize`; `JsValue` is only the *encoding*. `get_dimensions` is the
clearest case — its entire body is `to_value(&[self.nx, self.ny, self.nz])`.

Only **C** is genuinely target-shaped, and even there the target-specific part is an
*optimization* (avoiding a copy), not the data.

### Two of the twelve are dead

`getLastFimStepStats` and `getFimStepStatsHistory` have **zero call sites** in the frontend, which
is their only consumer. Whatever else happens, these should be deleted or demoted to native
diagnostics rather than ported. Porting unused API is how a boundary grows without anyone
choosing to grow it.

## 2. The invariant worth committing to

> **The engine never names a target type.** No `JsValue` in `simulator`'s API, and equally no
> `PyObject` — the second rule matters more than the first, because the first is already nearly
> true and the second is the one that will be tempting next.

Everything below follows from this. It is the only property that makes the answer independent of
which targets exist, including targets nobody has proposed yet.

## 3. Recommendation: generalize the pattern this repository already chose

`compositional/api.rs` (838 lines) is plain Rust over `serde`, owns every decision about what a
payload may contain, and is tested natively by `comp_api_*`. `compositional/frontend.rs` (101
lines) converts `JsValue` ↔ those types and **does nothing else**. Its module docs say why:

> *"That split is what makes the plan's 'verify native/WASM results on the same tiny fixtures'
> mean something: both paths run this code, and the shell has nothing in it that could differ."*

An 8:1 ratio of decisions to conversion. The black-oil path never received this treatment:
`frontend.rs` is 1 349 lines mixing model API, serde conversion and JS-specific packing.

**So: give black-oil its own `api` module, mirroring `compositional/api.rs`, and reduce
`frontend.rs` to a shell.**

```
src/lib/ressim/src/api.rs          payload structs + engine methods returning them   (new, plain serde)
src/lib/ressim/src/frontend.rs     JsValue <-> api types. Nothing else.              (shrinks)
crates/ressim-py/src/lib.rs        PyObject <-> api types. Nothing else.             (grows a little)
```

The decisive argument is not elegance, it is that **this is already the house pattern**. One
pattern maintained in two places beats two patterns maintained in one place each, and a future
contributor reading either model finds the same shape.

### What each target then does

| Target | How it gets a rate history |
|---|---|
| Engine | `pub fn rate_history(&self) -> &[TimePointRates]` — borrowed, no copy |
| Browser | `serde_wasm_bindgen::to_value(sim.rate_history())` — as today |
| Python | `pythonize`/`serde` over the same slice, or a `numpy` view |
| A future target | its own one-line conversion |

Each shim is mechanical and testable. None of them contains a decision.

## 4. Alternatives, and why not

**Return JSON strings from the engine** (`rate_history_json() -> String`). One function, every
target, no shims. Rejected: it forces a JSON encode **and** decode on the browser's hot path —
`getRateHistorySince` is called per step by the worker — where `serde_wasm_bindgen` currently
produces JS values directly. It also throws away typing at exactly the boundary where a schema
mistake is most expensive, and Python would re-parse text the engine just formatted. A portable
encoding is not the same thing as a portable *interface*, and it is the interface we need.

**Add a `pyo3` shim inside `simulator` beside the wasm one.** Rejected in §9e of the split plan
and again here: two proc-macro attribute systems on one type, a feature matrix that grows
multiplicatively, and the engine forced to know its consumers. The separate-crate shape S5 built
is the alternative, and it works.

**Expose the internal fields as `pub`.** Rejected: it makes every internal representation a
compatibility promise, and the payload schema is precisely the thing that should be stable while
internals move.

**Leave it; let each consumer reach in.** This is the status quo by default, and it is how the
black-oil `frontend.rs` reached 1 349 lines. The cost is paid by whoever adds the third target.

## 5. The zero-copy question, honestly

`get_grid_state` exists because the worker posts grid state every step and `Float64Array::view`
avoids copying five arrays. That is a real optimization and should not be lost to tidiness.

It also does not conflict with the design, because **zero-copy is available on both sides; it is
just spelled differently**:

- Engine: `pub fn pressure(&self) -> &[f64]` — borrowed slices, no copy, no schema.
- Browser: keeps `getGridState` as a **wasm-only extra** built from those slices, exactly as now.
- Python: the same slices become a `memoryview`/`numpy` array without copying.

What becomes shared is the **schema** (`GridState { pressure, sat_water, sat_oil, sat_gas, rs }`),
not the representation. Add one test asserting the portable `grid_state()` and the zero-copy
`getGridState` carry identical values, and the optimization can never silently diverge from the
contract it is optimizing.

Note that `GridStatePayload` — the deserialize half of this schema — **already exists** inside
`frontend.rs`. It is the schema; it is simply in the wrong file.

## 6. Two robustness wins that come along

These are not the motivation, but they are free and worth stating:

1. **The `.unwrap()`s go.** All seven kind-A functions call `.unwrap()` on serialization. A
   failure is an abort across the FFI boundary with no diagnostic. Engine methods returning Rust
   values cannot fail; only the shims can, and each shim can return `Result` and say which payload
   failed.
2. **A schema version becomes possible.** `compositional/api.rs` declares *"The only schema this
   build accepts"* and refuses anything else. Black-oil `load_state` has no such guard, so a
   checkpoint saved by an older build is deserialized on a best-effort basis. Once the payload is
   a named type, a version field is a small, obvious addition.

## 7. Keeping it true

A design decays unless something fails when it is violated. Two cheap gates:

1. **No target types in the engine's API.** An architecture test asserting that no file under
   `src/lib/ressim/src/` outside `*/frontend.rs` mentions `JsValue`, `wasm_bindgen`, `PyObject` or
   `pyo3`. The repo already has this class of test (`packageBoundaries.test.ts`,
   `chartAgnosticArchitecture.test.ts`), and it is the same shape.
2. **Shell thinness.** Assert that each `frontend.rs` contains no arithmetic and no engine field
   access beyond calling `api` methods — or, more simply and more robustly, assert a line-count
   ratio between `api.rs` and its shell, which fails loudly when logic migrates into a shim.

Gate 1 is the important one. Gate 2 is a smell detector; do not over-engineer it.

## 8. Sequencing

Three phases, each independently shippable, each keeping the byte-identical-wasm check and
`validate:product` green. Ordered by payoff, not by file:

**Phase 1 — the seven serializers.** Move the payload types to `api.rs`, add native methods
returning them, reduce the wasm functions to conversions. Delete the two dead ones (§1) rather
than porting them. *Unlocks the rate history natively, which is the single thing standing between
`ressim-py` and real work.*

**Phase 2 — the four deserializers.** `set_pvt_table`, `set_three_phase_scal_tables`,
`set_sweep_config`, `load_state`. Unlocks configuration and checkpoint/restore from any target,
and is where the schema version (§6.2) should land.

**Phase 3 — `get_grid_state`.** Portable `grid_state()` plus the retained zero-copy path and the
equality test between them (§5).

**Phase 4, optional — one schema, generated bindings.** `src/lib/simulator-types.ts` is today a
hand-maintained mirror of these Rust types, and nothing prevents drift. With the payloads named
in one place, `ts-rs` or `schemars` could generate the TypeScript from the Rust. Genuinely
valuable, genuinely a new dependency, and explicitly **not** required by phases 1–3 — raise it as
its own decision rather than smuggling it in.

## 9. Cost and risk

Phases 1–2 are mechanical: the types exist, the traits are already derived, and the conversions
are one line each. The risk is not correctness but **blast radius** — `frontend.rs` is the shipped
wasm API surface, so every phase must show the generated `simulator.d.ts` unchanged, exactly as
S4 did. Where a signature must change, it changes for the frontend too, and
`pnpm run validate:product` plus the 14 scenario tests that drive WASM directly are the net.

Phase 3 carries the only judgement call: whether the portable grid-state path is ever allowed to
become the browser's path. It should not be, unless a measurement says the copy is free.

## 10. What this note does not decide

It does not choose a Python object mapping (`pythonize` vs manual vs `numpy`), because that is the
shim's business and the shim is the cheap part. It does not propose changing any physics,
tolerance or solver behaviour. And it does not commit to phase 4, which is a dependency decision
rather than an architectural one.
