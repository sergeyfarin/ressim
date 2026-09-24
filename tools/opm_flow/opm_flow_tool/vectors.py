"""What each summary vector is, in what unit, and where the frontend may draw it.

Flow prints a unit under every column of its text summary. The artifact builder
checks that row against this table rather than trusting a case's say-so: a deck
that is not METRIC, a vector whose unit differs from what the chart axis assumes,
or a mnemonic nobody has classified fails generation instead of shipping curves
on the wrong scale (#20). Before this table existed, four cases declared their
rates as reservoir ``m3/day`` while Flow reported them as ``SM3/DAY``.

``panels`` is the other half of the contract. A cumulative-oil vector must land
on the cumulative-oil panel and a water rate may not land on a water-cut panel:
the frontend panel keys are shared across quantities of different phases, so a
mapping is only as honest as the phase it names. ``token`` is the word both the
curve key and its legend label must contain, which keeps the phase (or the
quantity) readable wherever the curve is shown.

Units are the strings Flow 2026.04 writes for a METRIC deck, confirmed against
real runs (``tests/fixtures/*.RSM`` and every committed artifact).
"""

from __future__ import annotations

from dataclasses import dataclass


@dataclass(frozen=True)
class VectorSpec:
    unit: str
    panels: frozenset[str]
    token: str
    description: str


def _spec(unit: str, panels: tuple[str, ...], token: str, description: str) -> VectorSpec:
    return VectorSpec(unit=unit, panels=frozenset(panels), token=token, description=description)


# The unit of the TIME column every summary starts with.
TIME_UNIT = "DAYS"

METRIC_VECTORS: dict[str, VectorSpec] = {
    # Surface rates. `rates` is a multi-phase rate panel in the solution-gas-drive
    # layouts; in the Buckley-Leverett layouts it is the water/gas cut panel, which
    # is why the waterflood cases map FOPR to `oil_rate` and only FWCT to `rates`.
    "FOPR": _spec("SM3/DAY", ("rates", "oil_rate"), "oil", "field oil production rate, stock tank"),
    "FWPR": _spec("SM3/DAY", ("rates", "water_rate"), "water", "field water production rate, surface"),
    "FGPR": _spec("SM3/DAY", ("gas_rate",), "gas", "field gas production rate, surface"),
    "FWIR": _spec("SM3/DAY", ("rates", "injection_rate"), "injection", "field water injection rate, surface"),
    "FGIR": _spec("SM3/DAY", ("injection_rate",), "gas", "field gas injection rate, surface"),
    # Surface cumulatives. Oil and water share the `cumulative` panel; gas has its own.
    "FOPT": _spec("SM3", ("cumulative",), "oil", "field cumulative oil production, stock tank"),
    "FWPT": _spec("SM3", ("cumulative",), "water", "field cumulative water production, surface"),
    "FGPT": _spec("SM3", ("cumulative_gas",), "gas", "field cumulative gas production, surface"),
    # Cumulative injection at surface conditions: what ResSim's `cum-injection`
    # curve and cumulative-injection axis report.
    "FWIT": _spec("SM3", ("volumes",), "injection", "field cumulative water injection, surface"),
    "FGIT": _spec("SM3", ("volumes",), "injection", "field cumulative gas injection, surface"),
    # Reservoir-volume injection: the numerator of pore volumes injected. An axis
    # source only; it has no panel of its own.
    "FVIT": _spec("RM3", (), "injection", "field cumulative injection, reservoir volume"),
    "FGIP": _spec("SM3", (), "gas", "field gas in place, surface"),
    "FWCT": _spec("", ("rates",), "water", "field water cut, surface-rate fraction"),
    "FPR": _spec("BARSA", ("diagnostics",), "pressure", "field average pressure, pore-volume weighted"),
    "FGOR": _spec("SM3/SM3", ("gor",), "gor", "field producing gas-oil ratio"),
    "WGOR": _spec("SM3/SM3", ("gor",), "gor", "well producing gas-oil ratio"),
    "WBHP": _spec("BARSA", ("injector_bhp", "producer_bhp"), "bhp", "well bottom-hole pressure"),
    "YEARS": _spec("YEARS", (), "", "elapsed time in years"),
}


def mnemonic_of(curve_id: str) -> str:
    """``WBHP:PROD`` -> ``WBHP``."""
    return curve_id.split(":", 1)[0]
