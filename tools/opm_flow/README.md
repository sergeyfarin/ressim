# ResSim OPM Flow Tools

Offline tooling for translating selected ResSim predefined cases into Eclipse-style decks, running the installed `flow` executable, and writing stable JSON artifacts for the frontend catalog.

Use `uv` for every command from this directory or the repo root:

```bash
uv run --directory tools/opm_flow python -m opm_flow_tool.cli generate-deck wf_bl1d --output tmp/opm-flow-runs/decks/wf_bl1d.DATA
uv run --directory tools/opm_flow python -m opm_flow_tool.cli run-flow wf_bl1d
uv run --directory tools/opm_flow python -m opm_flow_tool.cli build-artifacts all
```

Current scope is deliberately narrow while FIM is postponed: `wf_bl1d` and `spe1_gas_injection`. Generated artifacts include provenance and unit metadata even when no parsed OPM summary is available yet.
