# water-heavy-step1

Generated deck for the FIM heavy-water control: the wasm `water-pressure` preset at 12×12×3,
`TSTEP 1.0`, BHP injector 500 / producer 100, layer-0 completions and gravity on (`nz > 1`). The
deck is written by `src/lib/ressim/src/tests/opm_small_direct.rs` from the same simulator
configuration the wasm preset uses; it is not hand-mapped.

Regenerate it with the command in [`../README.md`](../README.md), then update
[`manifest.json`](manifest.json). Its current Flow/ResSim measurements are the
`water-heavy-step1` row in [`../small-direct/scorecard.json`](../small-direct/scorecard.json).

The historical FIM-DIAG-003 / WATER-001…017 narrative that used an earlier hand-edited version of
this deck remains in `docs/FIM_CONVERGENCE_WORKLOG.md` and `docs/FIM_EXPERIMENT_REGISTRY.md`; those
entries are dated evidence, not the current deck.
