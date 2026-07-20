# ResSim OPM Flow Tools

Offline tooling for translating selected ResSim predefined cases into Eclipse-style decks, running the installed `flow` executable, and writing stable JSON artifacts for the frontend catalog.

Use `uv` for every command from this directory or the repo root:

```bash
uv run --directory tools/opm_flow python -m opm_flow_tool.cli generate-deck wf_bl1d --output tmp/opm-flow-runs/decks/wf_bl1d.DATA
uv run --directory tools/opm_flow python -m opm_flow_tool.cli run-flow wf_bl1d
uv run --directory tools/opm_flow python -m opm_flow_tool.cli build-artifacts all
```

Current scope is deliberately narrow while FIM is postponed: `wf_bl1d` and `spe1_gas_injection`. Generated artifacts include provenance and unit metadata even when no parsed OPM summary is available yet.

## Summary parsing (RUNSUM/SEPARATE → `.RSM`)

Both decks request `RUNSUM` + `SEPARATE` in their `SUMMARY` section, so a real `flow` run writes a text `.RSM` summary alongside the binary output. `build_artifact()` looks for `<run-root>/<case-key>/*.RSM`; if found, `opm_flow_tool/summary.py` parses it and the artifact status becomes `parsed` with real series data. If the run directory exists but no `.RSM` is found, status is `flow-run`; if the `.RSM` exists but fails to parse (or is missing an expected curve), status is `error` with the reason in `notes`. Never crashes `build-artifacts all` for one bad case.

The parser's column-layout assumptions (fixed-width fields, uniform gap between columns, header/data separator found by scanning forward from the `TIME` row rather than taking the first dashed line in the page) were reverse-engineered from and validated against **real** `flow 2026.04` output — see `tests/fixtures/*.RSM`, which are trimmed excerpts of actual runs, not hand-authored samples. A different Flow version could format differently; if a future run disagrees, fix `summary.py` and its fixtures together against a fresh real run.

Run the Python tests from this directory: `uv run pytest`.

## Known deck-physics caveat (2026-07-16)

The `wf_bl1d` OPM reference deck runs successfully and parses cleanly, but its oil-rate behavior is suspect: FOPR is negligible (~1e-4 sm3/day) and essentially flat for the entire 50-day run, while FWPR tracks FWIR almost exactly from the very first 0.25-day timestep — i.e. the deck shows near-instant water breakthrough rather than a Buckley-Leverett-style front arriving after significant PVI. This was **not** debugged further (out of scope for the pipeline work that surfaced it); treat the current `wf_bl1d.json` artifact's oil-rate/water-rate series as parsed-and-plumbed but **not yet validated as a meaningful reference** until this is investigated. See `TODO.md`.
