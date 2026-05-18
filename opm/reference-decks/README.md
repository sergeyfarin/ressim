# OPM Reference Decks

These decks are Phase-0 validation fixtures for the OPM-compatible FIM
reimplementation plan. They are tracked inputs, not generated output.

Run them through `scripts/opm-ressim-compare.sh`; the harness copies each deck
into the selected comparison output directory before invoking Flow so OPM
restart, summary, and report files do not dirty this tree.

The decks intentionally mirror the current `scripts/fim-wasm-diagnostic.mjs`
presets as closely as Eclipse/Flow syntax allows. Treat same-dt OPM output as a
performance reference until matching dt/4 and dt/16 refinement tables are
recorded for the metric being promoted.

Initial Flow parse/run validation was performed for:

- `water-medium-step1`
- `water-medium-6step`
- `gas-rate-10x10x3`
