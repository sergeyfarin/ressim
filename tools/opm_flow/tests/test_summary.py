from __future__ import annotations

from pathlib import Path

import pytest

from opm_flow_tool.summary import find_summary_file, parse_rsm

FIXTURES = Path(__file__).parent / "fixtures"


def test_parses_real_field_only_summary_including_integer_valued_time_row():
    """Fixture is a trimmed excerpt of a real `flow 2026.04` RUNSUM/SEPARATE
    run (tools/opm_flow, wf_bl1d case), not a hand-authored sample. Row 4
    (TIME=1) is the case that first exposed a real formatting quirk: Flow
    prints a whole-number TIME right-justified as a bare "1" with no
    decimal point, at a different start position within the field than the
    fractional rows — this only parses correctly with range-bounded
    fixed-width slicing, not exact token-position matching.
    """
    text = (FIXTURES / "wf_bl1d_sample.RSM").read_text(encoding="utf-8")
    summary = parse_rsm(text)

    assert summary.time_days == [0.25, 0.5, 0.75, 1.0, 1.25, 1.5]

    by_id = summary.by_curve_id()
    assert set(by_id) == {"YEARS", "FOPR", "FOPT", "FPR", "FWIR", "FWPR", "FWPT"}

    fopr = by_id["FOPR"]
    assert fopr.well_or_group is None
    assert fopr.unit == "SM3/DAY"
    assert fopr.values == [14.51805, 13.59252, 13.44682, 13.67147, 14.15780, 14.62355]

    fpr = by_id["FPR"]
    assert fpr.values == [283.3067, 273.1377, 271.7649, 274.8291, 281.1432, 286.9728]


def test_parses_real_summary_with_well_name_row_and_scale_factor_row():
    """Fixture is a trimmed excerpt of a real `flow 2026.04` run for the
    spe1_gas_injection case, exercising both the well-name header row (WBHP
    'INJ'/'PROD', WGOR 'PROD') and the scale-factor header row ("*10**3"
    under FGPT, since cumulative gas exceeds the column's plain display
    width) against genuine Flow column alignment. FGPT's raw displayed
    values (e.g. 6.629687) must come out multiplied by 1000.
    """
    text = (FIXTURES / "spe1_gas_injection_sample.RSM").read_text(encoding="utf-8")
    summary = parse_rsm(text)

    assert summary.time_days == [1.0, 4.0, 13.0, 30.0, 60.0, 90.0, 120.0]

    by_id = summary.by_curve_id()
    assert set(by_id) == {"YEARS", "FGIR", "FGPT", "FOPR", "FOPT", "FPR", "WBHP:INJ", "WBHP:PROD", "WGOR:PROD"}

    fgir = by_id["FGIR"]
    assert fgir.well_or_group is None
    assert fgir.values == [12598.74, 12605.69, 12603.79, 12596.99, 12582.26, 12674.31, 12886.47]

    fgpt = by_id["FGPT"]
    assert fgpt.values == pytest.approx(
        [6629.687, 26511.40, 86134.61, 198729.7, 397405.3, 596077.2, 794751.6]
    )

    wbhp_inj = by_id["WBHP:INJ"]
    assert wbhp_inj.mnemonic == "WBHP"
    assert wbhp_inj.well_or_group == "INJ"
    assert wbhp_inj.values == [621.0] * 7

    wbhp_prod = by_id["WBHP:PROD"]
    assert wbhp_prod.well_or_group == "PROD"
    assert wbhp_prod.values == [69.0] * 7

    wgor = by_id["WGOR:PROD"]
    assert wgor.well_or_group == "PROD"
    assert wgor.values == pytest.approx([274.9292] * 7)


def test_mismatched_time_axes_across_pages_raise():
    text = (
        "1\n"
        " -----------\n"
        " SUMMARY OF RUN TEST\n"
        " -----------\n"
        " TIME         FOPR\n"
        " DAYS         SM3/DAY\n"
        " -----------------------\n"
        " 0.000        1.000\n"
        " 1.000        2.000\n"
        "2\n"
        " -----------\n"
        " SUMMARY OF RUN TEST\n"
        " -----------\n"
        " TIME         FWPR\n"
        " DAYS         SM3/DAY\n"
        " -----------------------\n"
        " 0.000        3.000\n"
    )
    with pytest.raises(ValueError, match="mismatched TIME axes"):
        parse_rsm(text)


def test_missing_separator_after_time_row_raises():
    with pytest.raises(ValueError, match="data separator"):
        parse_rsm("1\n SUMMARY OF RUN TEST\n\n TIME         FOPR\n DAYS         SM3/DAY\n")


def test_no_time_row_at_all_raises():
    with pytest.raises(ValueError, match="Could not find a header row starting with TIME"):
        parse_rsm("1\n SUMMARY OF RUN TEST\n -----------\n")


def test_non_uniform_column_spacing_raises():
    text = (
        "1\n"
        " TIME    FOPR       FWPR\n"
        " DAYS    SM3/DAY    SM3/DAY\n"
        " -----------------------\n"
        " 0.000   1.000      2.000\n"
    )
    with pytest.raises(ValueError, match="not uniformly spaced"):
        parse_rsm(text)


def test_find_summary_file_prefers_rsm_over_lowercase(tmp_path):
    (tmp_path / "case.rsm").write_text("lower", encoding="utf-8")
    (tmp_path / "CASE.RSM").write_text("upper", encoding="utf-8")

    found = find_summary_file(tmp_path)
    assert found is not None
    assert found.read_text(encoding="utf-8") == "upper"


def test_find_summary_file_returns_none_when_absent(tmp_path):
    assert find_summary_file(tmp_path) is None
