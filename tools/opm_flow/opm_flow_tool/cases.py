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
    # Maps a parsed summary vector's curve_id (mnemonic, or "MNEMONIC:NAME"
    # for well/group vectors) to how the frontend should display it. Keys
    # must be a subset of what the deck's SUMMARY section actually requests;
    # `build_artifact()` treats an unmapped-but-present vector as ignored,
    # not an error, so this can be extended incrementally.
    curve_display: dict[str, dict[str, str]]


def _clean_deck(text: str) -> str:
    return dedent(text).strip() + "\n"


WF_BL1D = OpmCase(
    key="wf_bl1d",
    scenario_key="wf_bl1d",
    label="1D Waterflood IMPES Reference-Compatible Case",
    deck_name="WF_BL1D.DATA",
    supported_curves=("FOPR", "FWPR", "FWIR", "FOPT", "FWPT", "FPR"),
    units={"system": "METRIC", "time": "days", "pressure": "bar", "rate": "m3/day"},
    curve_display={
        "FOPR": {"panelKey": "rates", "curveKey": "opm-oil-rate", "label": "OPM Flow — Oil Rate"},
        "FWPR": {"panelKey": "rates", "curveKey": "opm-water-rate", "label": "OPM Flow — Water Rate"},
        "FWIR": {"panelKey": "rates", "curveKey": "opm-injection-rate", "label": "OPM Flow — Injection Rate"},
        "FOPT": {"panelKey": "cumulative", "curveKey": "opm-cum-oil", "label": "OPM Flow — Cum Oil"},
        "FWPT": {"panelKey": "cumulative", "curveKey": "opm-cum-water", "label": "OPM Flow — Cum Water"},
        "FPR": {"panelKey": "diagnostics", "curveKey": "opm-avg-pressure", "label": "OPM Flow — Avg Pressure"},
    },
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
          100 1.002 1.0
          300 1.000 1.0
          500 0.998 1.0 /
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
          0 300 10000 0 0 0 0 0 0 /
        SUMMARY
        FOPR
        FWPR
        FWIR
        FOPT
        FWPT
        FPR
        RUNSUM
        SEPARATE
        SCHEDULE
        RPTRST
          BASIC=2 /
        WELSPECS
          'INJ' 'G' 1 1 0 'WATER' /
          'PROD' 'G' 96 1 0 'OIL' /
        /
        COMPDAT
        -- Items 7-8 (SATNUM, connection transmissibility factor) are defaulted so
        -- Flow computes the Peaceman factor itself; item 9 is the wellbore
        -- DIAMETER (2 x the ResSim scenario's 0.1 m well_radius).
          'INJ' 1 1 1 1 'OPEN' 2* 0.2 /
          'PROD' 96 1 1 1 'OPEN' 2* 0.2 /
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
    curve_display={
        "FOPR": {"panelKey": "oil_rate", "curveKey": "opm-oil-rate", "label": "OPM Flow — Oil Rate"},
        "FGIR": {"panelKey": "injection_rate", "curveKey": "opm-gas-injection-rate", "label": "OPM Flow — Gas Injection Rate"},
        "FOPT": {"panelKey": "cumulative", "curveKey": "opm-cum-oil", "label": "OPM Flow — Cum Oil"},
        "FGPT": {"panelKey": "cumulative", "curveKey": "opm-cum-gas", "label": "OPM Flow — Cum Gas"},
        "FPR": {"panelKey": "diagnostics", "curveKey": "opm-avg-pressure", "label": "OPM Flow — Avg Pressure"},
        "WBHP:INJ": {"panelKey": "injector_bhp", "curveKey": "opm-injector-bhp", "label": "OPM Flow — Injector BHP"},
        "WBHP:PROD": {"panelKey": "producer_bhp", "curveKey": "opm-producer-bhp", "label": "OPM Flow — Producer BHP"},
        "WGOR:PROD": {"panelKey": "gor", "curveKey": "opm-gor", "label": "OPM Flow — GOR"},
    },
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
          1 1 20 20 1 20 /
        EQLDIMS
        /
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
        -- 8325 ft: the true SPE1 reservoir top. Needed so the WELSPECS datum
        -- depths below sit inside the grid and BHP is reported at the same
        -- datum the ResSim scenario uses (depth_reference 2560 m).
        TOPS
          100*2537.46 /
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
        -- Datum 8400 ft / 4800 psia, WOC just below (8450 ft) and GOC just
        -- above (8300 ft) the reservoir, as in Odeh's Table 1.
        EQUIL
          2560.32 331 2575.56 0 2529.84 0 1 0 0 /
        -- Undersaturated: Rs is constant with depth (bubble point 4014.7 psia
        -- < 4800 psia initial), so both bracketing depths carry the same Rs.
        RSVD
          2529.84 226.197
          2575.56 226.197 /
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
        RUNSUM
        SEPARATE
        SCHEDULE
        -- Case 1: no re-dissolution of free gas, matching the ResSim
        -- scenario's gasRedissolutionEnabled: false.
        DRSDT
          0 /
        -- Datum depths are the perforated layer centres: 8335 ft (layer 1) for
        -- the injector, 8400 ft (layer 3) for the producer.
        WELSPECS
          'INJ' 'G' 1 1 2540.51 'GAS' /
          'PROD' 'G' 10 10 2560.32 'OIL' /
        /
        COMPDAT
        -- Items 7-8 (SATNUM, connection transmissibility factor) are defaulted so
        -- Flow computes the Peaceman factor itself; item 9 is the wellbore
        -- DIAMETER (0.5 ft = 2 x the deck's 0.25 ft well radius).
          'INJ' 1 1 1 1 'OPEN' 2* 0.1524 /
          'PROD' 10 10 3 3 'OPEN' 2* 0.1524 /
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


GAS_DRIVE = OpmCase(
    key="gas_drive",
    scenario_key="gas_drive",
    label="Solution Gas Drive Black-Oil Reference",
    deck_name="GAS_DRIVE.DATA",
    supported_curves=("FOPR", "FGPR", "FOPT", "FGPT", "FPR", "FGOR"),
    units={"system": "METRIC", "time": "days", "pressure": "bar", "rate": "sm3/day"},
    # FGPR/FGPT are requested in SUMMARY and parsed, but deliberately left
    # unmapped: gas rate and cumulative gas are ~3 orders of magnitude larger
    # than their oil counterparts, so overlaying them on the shared `rates` /
    # `cumulative` axes would flatten the simulated curves. They stay available
    # in the .RSM for grading (docs/THREE_PHASE_VALIDATION.md), which is done in
    # the Rust acceptance test rather than through the chart overlay.
    curve_display={
        "FOPR": {"panelKey": "rates", "curveKey": "opm-oil-rate", "label": "OPM Flow — Oil Rate"},
        "FOPT": {"panelKey": "cumulative", "curveKey": "opm-cum-oil", "label": "OPM Flow — Cum Oil"},
        "FPR": {"panelKey": "diagnostics", "curveKey": "opm-avg-pressure", "label": "OPM Flow — Avg Pressure"},
        "FGOR": {"panelKey": "gor", "curveKey": "opm-gor", "label": "OPM Flow — GOR"},
    },
    deck=_clean_deck(
        """
        RUNSPEC
        TITLE
          RESSIM GAS_DRIVE OPM FLOW REFERENCE /
        DIMENS
          20 1 1 /
        OIL
        WATER
        GAS
        DISGAS
        METRIC
        TABDIMS
          1 1 20 20 1 20 /
        WELLDIMS
          1 1 1 1 /
        START
          1 JAN 2026 /
        GRID
        DXV
          20*50 /
        DYV
          1*50 /
        DZV
          1*10 /
        -- The ResSim scenario runs with gravity disabled on a single flat
        -- layer. A non-zero, uniform TOPS keeps the deck out of the
        -- depth-degenerate configuration without introducing any within-layer
        -- gravity contrast: every cell centre sits at the same 2005 m.
        TOPS
          20*2000 /
        PORO
          20*0.2 /
        PERMX
          20*100 /
        PERMY
          20*100 /
        PERMZ
          20*10 /
        PROPS
        -- ResSim computes three-phase k_ro with Stone's Model II
        -- (relperm.rs::k_ro_stone2), so the deck must select it explicitly:
        -- Flow's default three-phase oil model is not Stone II.
        STONE2
        -- Surface gas density for 0.75 gas gravity (0.75 x 1.2232 kg/m3).
        -- This is the surface density the scenario's PVT table was generated
        -- with; ResSim's rho_g parameter carries the same value.
        DENSITY
          800 1000 0.9172 /
        PVTW
          200 1.0 3E-6 0.5 0 /
        ROCK
          200 1E-6 /
        -- PVTO/PVDG are the scenario's own generateBlackOilTable(35 API, 0.75
        -- gas gravity, 80 C, Pb = 200 bar, Pmax = 300 bar, 20 points,
        -- c_o = 1e-5/bar) table, emitted verbatim. Bubble point equals the
        -- initial pressure, so the reservoir starts saturated and the
        -- undersaturated branch exists only for the maximum Rs.
        PVTO
            0.04771    1.0000  1.05365  2.27850 /
            1.32560   15.7895  1.05651  2.17064 /
            3.05563   31.5789  1.06041  2.04252 /
            4.98034   47.3684  1.06479  1.91951 /
            7.04348   63.1579  1.06953  1.80582 /
            9.21608   78.9474  1.07456  1.70224 /
           11.48009   94.7368  1.07984  1.60842 /
           13.82306  110.5263  1.08536  1.52356 /
           16.23581  126.3158  1.09109  1.44676 /
           18.71128  142.1053  1.09702  1.37711 /
           21.24384  157.8947  1.10314  1.31381 /
           23.82889  173.6842  1.10944  1.25610 /
           26.46258  189.4737  1.11590  1.20334 /
           28.24377  200.0000  1.12030  1.17063
                     221.0526  1.12007  1.20424
                     236.8421  1.11989  1.23168
                     252.6316  1.11971  1.26092
                     268.4211  1.11953  1.29182
                     284.2105  1.11936  1.32430
                     300.0000  1.11918  1.35827 /
        /
        PVDG
             1.0000   1.239768  0.01254
            15.7895   0.076466  0.01271
            31.5789   0.037254  0.01299
            47.3684   0.024222  0.01337
            63.1579   0.017742  0.01382
            78.9474   0.013889  0.01436
            94.7368   0.011355  0.01498
           110.5263   0.009579  0.01568
           126.3158   0.008280  0.01646
           142.1053   0.007300  0.01731
           157.8947   0.006546  0.01821
           173.6842   0.005954  0.01916
           189.4737   0.005483  0.02014
           200.0000   0.005220  0.02081
           221.0526   0.004791  0.02214
           236.8421   0.004534  0.02313
           252.6316   0.004318  0.02412
           268.4211   0.004135  0.02508
           284.2105   0.003978  0.02603
           300.0000   0.003842  0.02695 /
        -- Corey curves evaluated from the scenario's own parameters
        -- (s_wc 0.2, s_or 0.15, s_gc 0.05, s_gr 0.05, s_org 0.20,
        --  n_w 2, n_o 2, n_g 1.5, k_rw_max 0.4, k_ro_max 1.0, k_rg_max 0.8),
        -- using the same effective-saturation definitions as relperm.rs.
        SWOF
          0.2000 0.000000 1.000000 0
          0.2812 0.006250 0.765625 0
          0.3625 0.025000 0.562500 0
          0.4437 0.056250 0.390625 0
          0.5250 0.100000 0.250000 0
          0.6062 0.156250 0.140625 0
          0.6875 0.225000 0.062500 0
          0.7687 0.306250 0.015625 0
          0.8500 0.400000 0.000000 0
          1.0000 0.400000 0.000000 0 /
        SGOF
          0.0000 0.000000 1.000000 0
          0.0500 0.000000 0.840278 0
          0.1286 0.030084 0.617347 0
          0.2071 0.085091 0.428713 0
          0.2857 0.156323 0.274376 0
          0.3643 0.240674 0.154337 0
          0.4429 0.336353 0.068594 0
          0.5214 0.442147 0.017149 0
          0.6000 0.557169 0.000000 0
          0.8000 0.800000 0.000000 0 /
        SOLUTION
        -- Explicit (non-equilibrium) initialisation, because the ResSim
        -- scenario starts from uniform saturations and pressure rather than
        -- from gravity-capillary equilibrium. Rs is the saturated value at
        -- 200 bar, so the whole field starts exactly at its bubble point.
        PRESSURE
          20*200 /
        SWAT
          20*0.2 /
        SGAS
          20*0.08 /
        RS
          20*28.24377 /
        SUMMARY
        FOPR
        FGPR
        FOPT
        FGPT
        FPR
        FGOR
        RUNSUM
        SEPARATE
        SCHEDULE
        -- The scenario enables gas redissolution, i.e. no DRSDT cap.
        WELSPECS
          'PROD' 'G' 20 1 2005 'OIL' /
        /
        COMPDAT
        -- Items 7-8 (SATNUM, connection transmissibility factor) are defaulted so
        -- Flow computes the Peaceman factor itself; item 9 is the wellbore
        -- DIAMETER (2 x the ResSim scenario's 0.1 m well_radius).
          'PROD' 20 1 1 1 'OPEN' 2* 0.2 /
        /
        WCONPROD
          'PROD' 'OPEN' 'BHP' 5* 100 /
        /
        TSTEP
          60*10 /
        END
        """
    ),
)


WF_GRAVITY = OpmCase(
    key="wf_gravity",
    scenario_key="wf_gravity",
    label="Gravity Override Cross-Section (base case)",
    deck_name="WF_GRAVITY.DATA",
    supported_curves=("FWCT", "FOPR", "FWPR", "FWIR", "FOPT", "FWPT", "FWIT", "FPR"),
    units={"system": "METRIC", "time": "days", "pressure": "bar", "rate": "m3/day"},
    curve_display={
        # Panels carry one property each, so only the vectors with a ResSim
        # counterpart in the waterflood layout are displayed: water cut, oil
        # rate, cumulative oil and average pressure. The rest stay in the
        # summary for provenance.
        "FWCT": {"panelKey": "rates", "curveKey": "opm-water-cut", "label": "OPM Flow — Water Cut"},
        "FOPR": {"panelKey": "oil_rate", "curveKey": "opm-oil-rate", "label": "OPM Flow — Oil Rate"},
        "FOPT": {"panelKey": "cumulative", "curveKey": "opm-cum-oil", "label": "OPM Flow — Cum Oil"},
        "FPR": {"panelKey": "diagnostics", "curveKey": "opm-avg-pressure", "label": "OPM Flow — Avg Pressure"},
    },
    deck=_clean_deck(
        """
        RUNSPEC
        TITLE
          RESSIM WF_GRAVITY OPM FLOW REFERENCE /
        DIMENS
          30 1 20 /
        OIL
        WATER
        METRIC
        TABDIMS
          1 1 20 20 1 20 /
        EQLDIMS
        /
        WELLDIMS
          2 2 1 2 /
        START
          1 JAN 2026 /
        GRID
        DXV
          30*10 /
        DYV
          1*20 /
        DZV
          20*2 /
        TOPS
          30*0 /
        PORO
          600*0.2 /
        PERMX
          600*5000 /
        PERMY
          600*5000 /
        PERMZ
          600*5000 /
        PROPS
        PVTW
          300 1.0 3E-6 0.5 0 /
        PVDO
          100 1.002 1.0
          300 1.000 1.0
          500 0.998 1.0 /
        ROCK
          300 1E-6 /
        DENSITY
          800 1000 1 /
        -- Corey n_w = n_o = 2, S_wc = S_or = 0.1, end points 1.0, zero capillary
        -- pressure: the same rock curves as the ResSim wf_gravity scenario.
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
        -- Datum at the mid-depth of the 40 m section so the initial hydrostatic
        -- field averages the ResSim scenario's uniform 300 bar. Water contact is
        -- placed far below the model, leaving every cell at connate water.
        EQUIL
          20 300 10000 0 0 0 0 0 0 /
        SUMMARY
        FWCT
        FOPR
        FWPR
        FWIR
        FOPT
        FWPT
        FWIT
        FPR
        RUNSUM
        SEPARATE
        SCHEDULE
        RPTRST
          BASIC=2 /
        -- One perforation per well, both in the bottom layer: the ResSim engine
        -- applies no wellbore hydrostatic correction across completions, so the
        -- comparison is only well-model-neutral with a single connection each.
        WELSPECS
          'INJ' 'G' 1 1 39 'WATER' /
          'PROD' 'G' 30 1 39 'OIL' /
        /
        COMPDAT
          'INJ' 1 1 20 20 'OPEN' 2* 0.2 /
          'PROD' 30 1 20 20 'OPEN' 2* 0.2 /
        /
        WCONINJE
          'INJ' 'WATER' 'OPEN' 'RATE' 160 1* 600 /
        /
        WCONPROD
          'PROD' 'OPEN' 'BHP' 5* 200 /
        /
        TSTEP
          210*2 /
        END
        """
    ),
)


CASES = {case.key: case for case in (WF_BL1D, SPE1_GAS_INJECTION, GAS_DRIVE, WF_GRAVITY)}
