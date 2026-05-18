# Phase-0 OPM Same-Dt Reference Runs

Date: 2026-05-18
Flow: 2025.10
Status: same-dt baseline only; not a converged physics reference.

These values were produced through `scripts/opm-ressim-compare.sh` using the
tracked decks in `opm/reference-decks/`. They are useful for checking that the
OPM harness runs and for comparing same-timestep behavior. Do not use them as
truth for solver promotion until matching dt/4 and dt/16 tables are recorded.

## Commands

```bash
scripts/opm-ressim-compare.sh --case water-medium-step1 --no-build-wasm --out-dir /tmp/ressim-opm-phase0-full --flow-bin /usr/bin/flow
scripts/opm-ressim-compare.sh --case water-medium-6step --no-build-wasm --out-dir /tmp/ressim-opm-phase0-full --flow-bin /usr/bin/flow
scripts/opm-ressim-compare.sh --case gas-rate-10x10x3 --no-build-wasm --out-dir /tmp/ressim-opm-phase0-full --flow-bin /usr/bin/flow
```

## water-medium-step1

| TIME | FPR | FOPT | FWIT | FWPT | WBHP:INJ | WBHP:PRO |
| ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 0.250000 | 308.299103 | 762.430054 | 764.658508 | 0.006550 | 500.000000 | 100.000000 |

## water-medium-6step

| TIME | FPR | FOPT | FWIT | FWPT | WBHP:INJ | WBHP:PRO |
| ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 0.250000 | 308.299103 | 762.430054 | 764.658508 | 0.006550 | 500.000000 | 100.000000 |
| 0.500000 | 327.590393 | 1588.115967 | 1595.049805 | 0.011042 | 500.000000 | 100.000000 |
| 0.750000 | 332.248108 | 2448.438721 | 2456.427002 | 0.013584 | 500.000000 | 100.000000 |
| 1.000000 | 334.827606 | 3321.796387 | 3330.290771 | 0.015144 | 500.000000 | 100.000000 |
| 1.250000 | 336.584076 | 4204.377441 | 4213.145508 | 0.016005 | 500.000000 | 100.000000 |
| 1.500000 | 337.959167 | 5093.812500 | 5102.734375 | 0.016403 | 500.000000 | 100.000000 |

## gas-rate-10x10x3

| TIME | FPR | FOPT | FGIT | FGPT | WBHP:INJ | WBHP:PRO | WGIT:INJ | WOPT:PRO | WGPT:PRO |
| ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 0.250000 | 206.701263 | 40.202415 | 20312.500000 | 3216.193359 | 216.787933 | 197.136124 | 20312.500000 | 40.202415 | 3216.193359 |
| 0.500000 | 215.469437 | 80.431961 | 41253.242188 | 6434.556641 | 222.854660 | 205.787186 | 41253.242188 | 80.431961 | 6434.556641 |
| 0.750000 | 224.682465 | 120.696999 | 63016.003906 | 9655.759766 | 231.013992 | 214.981857 | 63016.003906 | 120.696999 | 9655.759766 |
| 1.000000 | 233.269638 | 160.999344 | 85642.484375 | 12879.947266 | 238.762207 | 223.593735 | 85642.484375 | 160.999344 | 12879.947266 |
| 1.250000 | 242.163208 | 201.336441 | 109074.015625 | 16106.915039 | 247.230850 | 232.479095 | 109074.015625 | 201.336441 | 16106.915039 |
| 1.500000 | 251.108017 | 241.709549 | 133339.312500 | 19336.763672 | 255.917526 | 241.420273 | 133339.312500 | 241.709549 | 19336.763672 |

## Pending

- Add dt/4 and dt/16 deck variants for each promoted metric.
- Record ResSim-to-OPM comparison tables once the metric mapping is locked.
- Move or regenerate heavy-water same-dt/fine-dt references under
  `opm/reference-decks/`.
