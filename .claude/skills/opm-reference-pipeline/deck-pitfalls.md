# Deck pitfalls

Reference for the `opm-reference-pipeline` skill: the deck defects this repository shipped, as
recorded when they were fixed.

## Deck-physics caveat — RESOLVED 2026-07-24 (COMPDAT item shift)

Both decks put the wellbore *radius* in `COMPDAT` **item 8** (connection transmissibility factor)
instead of item 9 (wellbore *diameter*). Flow then used that number as the CF verbatim, choking every
connection by ~2 orders of magnitude. Symptoms: `wf_bl1d` FOPR ~1e-4 sm3/day with FWPR tracking FWIR
from the first timestep; `spe1_gas_injection` FOPR ~24 sm3/day with **both** wells pinned to their
BHP limits from day 1. Fixed by defaulting items 7-8 (`2*`) and passing the diameter in item 9
(`0.2` m for `wf_bl1d`, `0.1524` m for SPE1). **When adding a deck, remember COMPDAT item 9 is a
diameter, and sanity-check that a rate-controlled well actually holds its target rate.**

Fixed in the same pass for SPE1 (it was depth-degenerate and Case-2-flavoured): `TOPS` 0 → 2537.46 m
with matching `EQUIL`/`RSVD`/`WELSPECS` datum depths, added `EQLDIMS`, and added `DRSDT 0` (Case 1
has no re-dissolution, matching the scenario's `gasRedissolutionEnabled: false`).

Post-fix validation — the generated deck now tracks the canonical `OPM/opm-common/tests/SPE1CASE1.DATA`
(run with `flow SPE1CASE1.DATA --output-dir=.`, WELLDIMS raised to 4 so the RFT wells load) within
~4 % on FOPR and ~8 % on late-time GOR, with the same ~day-1000 BHP-floor decline onset. `wf_bl1d`
now shows a proper Buckley-Leverett front: FOPR flat ~70 sm3/day to breakthrough at ~14.5 d
(≈0.55 PV, as expected for a 1920 m³ PV at ~70 m³/day), then declining as FWPR rises.

Also fixed in the 2026-07-16 pass in the same pass, two pre-existing deck bugs unrelated to parsing that were silently blocking any real `flow` run of these two cases: `wf_bl1d`'s `PVDO` table had non-monotonic (flat) Bo values, which Flow rejects — now uses `c_o = 1e-5/bar` matching the ResSim `wf_bl1d` scenario's own declared compressibility; `spe1_gas_injection`'s `TABDIMS` declared `NTSFUN=2, NTPVT=15` while every PVT/SCAL keyword only supplied one region's table — corrected to `1 1`.
