from __future__ import annotations

from dataclasses import dataclass
from textwrap import dedent


@dataclass(frozen=True)
class OpmCase:
    key: str
    scenario_key: str
    label: str
    deck_name: str
    supported_curves: tuple[str, ...]
    units: dict[str, str]
    deck: str


def _clean_deck(text: str) -> str:
    return dedent(text).strip() + "\n"


WF_BL1D = OpmCase(
    key="wf_bl1d",
    scenario_key="wf_bl1d",
    label="1D Waterflood IMPES Reference-Compatible Case",
    deck_name="WF_BL1D.DATA",
    supported_curves=("FOPR", "FWPR", "FWIR", "FOPT", "FWPT", "FPR"),
    units={"system": "METRIC", "time": "days", "pressure": "bar", "rate": "m3/day"},
    deck=_clean_deck(
        """
        RUNSPEC
        TITLE
          RESSIM WF_BL1D OPM FLOW REFERENCE /
        DIMENS
          96 1 1 /
        OIL
        WATER
        METRIC
        TABDIMS
          1 1 20 20 1 20 /
        WELLDIMS
          2 2 1 2 /
        START
          1 JAN 2026 /
        GRID
        DXV
          96*10 /
        DYV
          1*10 /
        DZV
          1*1 /
        TOPS
          96*0 /
        PORO
          96*0.2 /
        PERMX
          96*2000 /
        PERMY
          96*2000 /
        PERMZ
          96*200 /
        PROPS
        PVTW
          300 1.0 3E-6 0.5 0 /
        PVDO
          100 1.0 1.0
          300 1.0 1.0
          500 1.0 1.0 /
        DENSITY
          800 1000 1 /
        SWOF
          0.10 0.0 1.0 0
          0.20 0.015625 0.765625 0
          0.30 0.0625 0.5625 0
          0.40 0.140625 0.390625 0
          0.50 0.25 0.25 0
          0.60 0.390625 0.140625 0
          0.70 0.5625 0.0625 0
          0.80 0.765625 0.015625 0
          0.90 1.0 0.0 0 /
        REGIONS
        SOLUTION
        EQUIL
          0 300 0 0 0 0 0 0 0 /
        SUMMARY
        FOPR
        FWPR
        FWIR
        FOPT
        FWPT
        FPR
        SCHEDULE
        RPTRST
          BASIC=2 /
        WELSPECS
          'INJ' 'G' 1 1 0 'WATER' /
          'PROD' 'G' 96 1 0 'OIL' /
        /
        COMPDAT
          'INJ' 1 1 1 1 'OPEN' 1* 0.1 /
          'PROD' 96 1 1 1 'OPEN' 1* 0.1 /
        /
        WCONINJE
          'INJ' 'WATER' 'OPEN' 'BHP' 1* 1* 500 /
        /
        WCONPROD
          'PROD' 'OPEN' 'BHP' 5* 100 /
        /
        TSTEP
          200*0.25 /
        END
        """
    ),
)


SPE1_GAS_INJECTION = OpmCase(
    key="spe1_gas_injection",
    scenario_key="spe1_gas_injection",
    label="SPE1 Black-Oil Benchmark",
    deck_name="SPE1_GAS_INJECTION.DATA",
    supported_curves=("FOPR", "FGIR", "FOPT", "FGPT", "FPR", "WBHP", "WGOR"),
    units={"system": "METRIC", "time": "days", "pressure": "bar", "rate": "sm3/day"},
    deck=_clean_deck(
        """
        RUNSPEC
        TITLE
          RESSIM SPE1 GAS INJECTION OPM FLOW REFERENCE SKELETON /
        DIMENS
          10 10 3 /
        OIL
        WATER
        GAS
        DISGAS
        METRIC
        TABDIMS
          2 15 20 20 1 20 /
        WELLDIMS
          2 2 1 2 /
        START
          1 JAN 2026 /
        GRID
        DXV
          10*304.8 /
        DYV
          10*304.8 /
        DZ
          100*6.096 100*9.144 100*15.24 /
        TOPS
          100*0 /
        PORO
          300*0.3 /
        PERMX
          100*500 100*50 100*200 /
        PERMY
          100*500 100*50 100*200 /
        PERMZ
          100*500 100*50 100*200 /
        PROPS
        -- This skeleton is generated from the ResSim SPE1 scenario. Keep the
        -- authoritative SPE1 deck-matching work in this tool rather than the UI.
        DENSITY
          860 1033 0.854 /
        PVTW
          331 1.038 4.67E-5 0.318 0 /
        PVTO
          0.18 1.01 1.062 1.040 /
          16.12 18.25 1.150 0.975 /
          32.06 35.49 1.207 0.910 /
          66.08 69.96 1.295 0.830 /
          113.29 138.91 1.435 0.695 /
          138.03 173.38 1.500 0.641 /
          165.64 207.85 1.565 0.594 /
          226.20 276.79 1.695 0.510
                 621.54 1.579 0.740 /
          288.17 345.73 1.827 0.449
                 621.54 1.737 0.631 /
        /
        PVDG
          1.01 0.9361 0.0080
          18.25 0.0679 0.0096
          35.49 0.0352 0.0112
          69.96 0.0179 0.0140
          138.91 0.00906 0.0189
          173.38 0.00727 0.0208
          207.85 0.00607 0.0228
          276.79 0.00455 0.0268
          345.73 0.00364 0.0309 /
        SWOF
          0.12 0 1 0
          0.24 1.86E-7 0.997 0
          0.36 7.438E-7 0.7 0
          0.48 1.674E-6 0.2 0
          0.60 2.975E-6 0.021 0
          0.72 4.649E-6 0.001 0
          0.84 6.694E-6 0 0
          1.00 1E-5 0 0 /
        SGOF
          0 0 1 0
          0.02 0 0.997 0
          0.05 0.005 0.98 0
          0.12 0.025 0.7 0
          0.20 0.075 0.35 0
          0.30 0.19 0.09 0
          0.40 0.41 0.021 0
          0.50 0.72 0.001 0
          0.70 0.94 0 0
          0.88 0.984 0 0 /
        SOLUTION
        EQUIL
          0 331 0 0 0 0 0 0 0 /
        RSVD
          0 226.197 /
        SUMMARY
        FOPR
        FGIR
        FOPT
        FGPT
        FPR
        WBHP
          'INJ' 'PROD' /
        WGOR
          'PROD' /
        SCHEDULE
        WELSPECS
          'INJ' 'G' 1 1 0 'GAS' /
          'PROD' 'G' 10 10 0 'OIL' /
        /
        COMPDAT
          'INJ' 1 1 1 1 'OPEN' 1* 0.0762 /
          'PROD' 10 10 3 3 'OPEN' 1* 0.0762 /
        /
        WCONINJE
          'INJ' 'GAS' 'OPEN' 'RATE' 2831680 1* 621 /
        /
        WCONPROD
          'PROD' 'OPEN' 'ORAT' 3179.74 4* 69 /
        /
        TSTEP
          120*30 /
        END
        """
    ),
)


CASES = {case.key: case for case in (WF_BL1D, SPE1_GAS_INJECTION)}
