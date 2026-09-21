"""Run quantities for notebooks, computed from the same contract the application plots.

S6 of `docs/ARCHITECTURE_SPLIT_PLAN_2026-09-19.md` chose to **share the data contract, not the
renderer**. The reasoning is in §S6: chasing pixel-identical charts across two rendering stacks is
a large bill for a small benefit, while agreeing on *what a quantity is called, what it is measured
in, and what its values are* is the part that actually makes two plots comparable.

So this module does not draw anything. It reads `contracts/run-quantities.json` — which is
**generated** from the TypeScript registry by `runQuantities.contract.test.ts`, so the ids, labels
and units here cannot drift from the app's — and computes the series a notebook plots with
matplotlib, plotly, or whatever it likes.

**What is computed here, and what is not.** A run's rate history is self-contained, so the
quantities derived from it alone are computed here and checked against the TypeScript
implementation by `parity/compare_quantities.py`. The rest — recovery factors, p/z, PVI, well BHP
— need inputs the rate history does not carry (oil and gas in place, a gas PVT table and
reservoir temperature, per-well history). Those are reported by `missing_inputs()` rather than
guessed at: a recovery factor computed against an assumed STOIIP would be worse than no curve.
"""
from __future__ import annotations

import json
import math
from pathlib import Path
from typing import Any, Iterable, Sequence

__all__ = ["CONTRACT", "derive", "quantities", "missing_inputs", "quantity_ids"]

_CONTRACT_PATH = Path(__file__).resolve().parents[3] / "contracts" / "run-quantities.json"
CONTRACT: dict[str, Any] = json.loads(_CONTRACT_PATH.read_text())

#: Derived-series keys this module computes from a rate history alone.
_SUPPORTED_SERIES = {
    "time",
    "oilRate",
    "gasRate",
    "injectionRate",
    "gasCut",
    "avgWaterSat",
    "gor",
    "cumulativeOil",
    "cumulativeGas",
    "cumulativeLiquid",
    "cumulativeInjection",
}


def _finite(value: Any, fallback: float = 0.0) -> float:
    """Mirror of the TypeScript `toFiniteNumber`: a non-number is the fallback, not a crash."""
    if isinstance(value, bool) or value is None:
        return fallback
    try:
        out = float(value)
    except (TypeError, ValueError):
        return fallback
    return out if math.isfinite(out) else fallback


def _nullable(value: Any) -> float | None:
    """A field that reports `null` rather than a substitute when it is not a finite number."""
    if isinstance(value, bool) or value is None:
        return None
    try:
        out = float(value)
    except (TypeError, ValueError):
        return None
    return out if math.isfinite(out) else None


def derive(rate_history: Sequence[dict[str, Any]]) -> dict[str, list]:
    """Derived series for the quantities a rate history can support on its own.

    Formulas follow `buildDerivedRunSeries` and `integrateRunSeries` in the TypeScript, including
    their edge cases, because agreeing on the common case and diverging on empty runs or a
    zero-oil step is the kind of disagreement that shows up as a mysteriously different chart.

    Cumulatives use the **rectangle rule** deliberately: the engine reports a step-*average* rate
    over the interval ending at `point.time`, so `rate x dt` is exact and a trapezoid is wrong.
    `runSeries.ts` records the measurement that settled it — do not "improve" this loop.
    """
    time: list[float] = []
    oil_rate: list[float | None] = []
    gas_rate: list[float | None] = []
    injection_rate: list[float | None] = []
    gas_cut: list[float | None] = []
    avg_water_sat: list[float | None] = []
    gor: list[float | None] = []
    cum_oil: list[float] = []
    cum_gas: list[float] = []
    cum_liquid: list[float] = []
    cum_injection: list[float] = []

    oil = gas = liquid = injection = 0.0
    previous_time = 0.0

    for index, point in enumerate(rate_history):
        t = _finite(point.get("time"))
        dt = max(0.0, t - (previous_time if index > 0 else 0.0))
        previous_time = t

        o = max(0.0, abs(_finite(point.get("total_production_oil"))))
        g = max(0.0, abs(_finite(point.get("total_production_gas"))))
        liquid_rate = max(0.0, abs(_finite(point.get("total_production_liquid"))))
        inj = max(0.0, _finite(point.get("total_injection")))

        oil += o * dt
        gas += g * dt
        liquid += liquid_rate * dt
        injection += inj * dt

        total = g + o
        time.append(t)
        oil_rate.append(o)
        gas_rate.append(g)
        injection_rate.append(inj)
        gas_cut.append(g / total if total > 1e-12 else 0.0)
        avg_water_sat.append(_nullable(point.get("avg_water_saturation")))

        # GOR is reported by the engine, but only means anything while oil is being produced.
        if o > 0:
            reported = _finite(point.get("producing_gor"))
            gor.append(reported if reported > 0 else None)
        else:
            gor.append(None)

        cum_oil.append(oil)
        cum_gas.append(gas)
        cum_liquid.append(liquid)
        cum_injection.append(injection)

    return {
        "time": time,
        "oilRate": oil_rate,
        "gasRate": gas_rate,
        "injectionRate": injection_rate,
        "gasCut": gas_cut,
        "avgWaterSat": avg_water_sat,
        "gor": gor,
        "cumulativeOil": cum_oil,
        "cumulativeGas": cum_gas,
        "cumulativeLiquid": cum_liquid,
        "cumulativeInjection": cum_injection,
    }


def quantity_ids() -> list[str]:
    """Every quantity id in the contract, including ones this module cannot compute."""
    return [q["id"] for q in CONTRACT["quantities"]]


def missing_inputs() -> dict[str, str]:
    """Contract quantities a rate history alone cannot produce, and what each would need.

    Reported rather than silently omitted: a notebook asking for a recovery factor deserves to be
    told it needs the volume in place, not handed a blank curve.
    """
    needs = {
        "pressure": "reported per step, but the app reads its own pressure series",
        "waterCut": "the app's watercut series",
        "recovery": "stock-tank oil initially in place",
        "recoveryGas": "gas initially in place",
        "producerBhp": "per-well BHP history",
        "injectorBhp": "per-well BHP history",
        "p_z": "a gas PVT table and the reservoir temperature",
        "pvi": "pore volume",
        "pvp": "pore volume",
    }
    return {
        q["id"]: needs.get(q["series"], f"the {q['series']} series")
        for q in CONTRACT["quantities"]
        if q["series"] not in _SUPPORTED_SERIES
    }


def quantities(rate_history: Sequence[dict[str, Any]]) -> dict[str, dict[str, Any]]:
    """Plot-ready quantities keyed by contract id.

    Each entry carries the contract's own `label` and `unit`, so a notebook axis says exactly what
    the application's axis says without anyone retyping it.
    """
    derived = derive(rate_history)
    out: dict[str, dict[str, Any]] = {}
    for q in CONTRACT["quantities"]:
        series = q["series"]
        if series not in derived:
            continue
        out[q["id"]] = {
            "label": q["label"],
            "unit": q["unit"],
            "property": q["property"],
            "time": derived["time"],
            "values": derived[series],
        }
    return out
