# Cross-solver gate and benchmark records

Reference for the `ressim-validation` skill: the two gates that run for any change that can
move an answer.

Cross-solver gate — every ResSim solver against OPM Flow and against each other (~20 s with
Flow cached; skips with a note when `flow` is not installed):

```bash
bash scripts/validate-cross-solver.sh                   # check against the committed scorecard
bash scripts/validate-cross-solver.sh --update          # re-baseline: on a COMMITTED tree only
bash scripts/validate-cross-solver.sh --markdown        # README-ready tables, then check
bash scripts/validate-cross-solver.sh --refine 0.025 --case ow-2d-12x12   # time-refined referee
```

It runs FIM (sparse and dense LU) and IMPES natively on the eight small-direct decks
(`opm/reference-decks/small-direct/`), runs Flow on the same decks, and fails when an accuracy
metric against Flow (worst and final cell Δp/ΔSw/ΔSg, cumulatives) or a work metric (substeps,
Newton) regresses past its band, when a run raises a new solver warning, or when sparse and dense
stop agreeing. It prints improvements past the band too: re-baseline them deliberately with
`--update` on a committed tree and commit the scorecard with the change that earned it, so the
scorecard's provenance names that commit. **Do not hand-copy small-direct numbers into a doc**;
use `--markdown`.

Benchmark records — `docs/BENCHMARKS.md` is generated, never typed (#54):

```bash
bash scripts/benchmarks.sh check                 # measure every section, compare with docs/benchmarks/benchmarks.json
bash scripts/benchmarks.sh check --tier fast     # without Flow / flowexp_comp / JutulDarcy (what CI has)
bash scripts/benchmarks.sh update [--only spe1]  # re-record and re-render: on a COMMITTED tree only
bash scripts/benchmarks.sh render --check        # page and README summary in sync with records (no runs)
```

The full tier runs JutulDarcy (`tools/jutul`, needs `juliaup add 1.12`; ~4 min of it is Julia
compiling on first use of each model type) as a second simulator on the same decks.

`check` fails when a banded error grows by more than a tenth of its band or leaves it, or when a
recorded measurement disappears (a renamed test, a broken producer). Every other change is listed:
record it with `update` in a follow-up commit on the committed tree, so the section's stamp names
the commit that moved it. To add a benchmark, record it from its test with
`tests/bench_record.rs` (or append to `$RESSIM_BENCH_OUT/records.jsonl` from a script), then give
its section a renderer in `tools/benchmarks/benchmarks.py`.

The page's **Signals** and **Coverage** blocks are what should move priorities: same-model gaps to
a reference that solves the same discrete model, criteria near their band, loose bands, and
scenarios with no numerical reference. `python3 tools/benchmarks/benchmarks.py signals` prints them
plus stale sections (`--strict` exits 1 when anything needs attention). A gap stops being flagged
only through an entry in `docs/benchmarks/explained.json`: `tracked` with an open issue, or
`explained` with the mechanism written where `ref` points. The Coverage grid reads the scenario
catalog, so adding a scenario, a Flow artifact case or a refinement dimension needs
`benchmarks.sh render` (CI's `render --check` fails otherwise).

Agreement with Flow at the same time step is not accuracy: FIM and Flow share an implicit scheme
and its time-step error. When FIM and IMPES disagree, `--refine` is the referee (see the
small-direct README's "Referee" section, where refinement shows IMPES converging to Flow).
