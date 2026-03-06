# Frontend Review (Archived): Input Selection, Reactivity, and Event Logic

Date: 2026-03-05

## Status

This document is retained as a historical review that helped drive the current frontend direction.

It is not the authoritative execution tracker anymore. Use `TODO.md` under `Authoritative Recovery Plan — Mode-Specific Panels` for active work.

## Durable Conclusions

The following conclusions from the original review still matter:

1. the product direction remains Option B: unified preset plus customize
2. analytical overlays should stay permissive with visible caveats rather than hard-blocking
3. benchmark presets should support clone-to-custom workflows
4. warning policy, override visibility, and truthful customized-state UX need explicit handling rather than implicit store behavior

## Findings That Were Resolved or Superseded

The original file contained detailed, line-specific findings that no longer describe the live codebase. The following items were resolved or otherwise superseded by later work:

- validation gating is now wired through the run controls
- mode alias handling for custom sub-case switching was fixed
- one-pass constraint repair was replaced with deterministic stabilization
- per-layer permeability edits are preserved when `nz` changes
- the old pre-run loading path was removed entirely
- App/store wiring moved to explicit domain objects instead of the earlier flatter store shape
- the shell-era UI discussed in the old review was replaced by the current mode-panel architecture

## Why The Detailed Original Findings Were Purged

The old version of this document referenced components and code paths that no longer exist, including shell-era UI files and the removed pre-run pipeline. Leaving those details in place made the document read like live guidance when it was actually a historical review.

## Current Source Of Truth

- `README.md` for the current UI architecture summary
- `TODO.md` for the active execution plan
- `docs/status.md` for chronological slice history and validation notes
- `src/lib/ui/` for the live mode-panel and section-component structure

## Historical Value That Still Remains

The archived review remains useful as product rationale:

- keep scenario selection and customization in one mental model
- keep analytical assumptions visible to the user
- preserve reproducibility through benchmark provenance and override tracking
- keep constraint logic deterministic and code-defined

If a new frontend audit is needed, write it against the current mode-specific panel architecture instead of extending the stale shell-era review.