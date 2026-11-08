"""Compare ResSim's SPE5 monthly reports with the JutulDarcy reference (#52, #116).

    python3 -I tools/jutul/spe5_compare.py RESSIM_STATES [REFERENCE]
    python3 -I tools/jutul/spe5_compare.py --ministeps JUTUL_MINISTEPS
    python3 -I tools/jutul/spe5_compare.py --references LABEL=REFERENCE LABEL=REFERENCE ...
    python3 -I tools/jutul/spe5_compare.py --validate-run REFERENCE VOLUME MAX_DT [NOT_BEFORE]
    python3 -I tools/jutul/spe5_compare.py --timestep-convergence R_REPORT R_5 R_05 J_REPORT J_5 J_05 [LAST]

The third form is the JutulDarcy well-volume/timestep sensitivity (tools/jutul/run-spe5-sensitivity.sh):
each reference's work, and its worst differences from the first one over months 1-217 and over all
240 months. A `REFERENCE.config.json` beside a reference, when present, supplies its configuration.

`--timestep-convergence` compares ResSim with itself and with JutulDarcy at matched timestep caps
(#116). R_* are outputs of the ignored ResSim test `comp_spe5_replay_with_timestep_options`
(schema `ressim-spe5-replay/1`) with no cap, a 5 d cap and a 0.5 d cap; J_* are JutulDarcy
references with the same caps (tools/jutul/run-spe5-sensitivity.sh: `default`, `default-dt5`,
`default-dt0.5`). It prints each ResSim run's work, conservation and stop, then the worst
differences of every pair over months 1..LAST (default: the last month all three ResSim runs
reached). "Σ oil" compares the sum of monthly producer oil rates, the same rule on both sides;
the ResSim-only rows also compare true cumulative surface oil and gas at LAST.

`--validate-run` exits non-zero unless REFERENCE is a complete 240-report, 7305 d run whose
`REFERENCE.config.json` records the requested SPE5_WELL_VOLUME_M3 and SPE5_MAX_DT_DAYS settings
(`default` or a number) and, with NOT_BEFORE (a file), unless both files are newer than it.

The second form prints the "What JutulDarcy accepts" table of docs/SPE5_VALIDATION.md from the
output of tools/jutul/spe5_ministep_diagnostics.jl. Jutul's MB is summed over hydrocarbon
components only: Jutul's water "MB" is not a mass balance (it sums per-cell volume-scaled residuals
and multiplies by dt twice), so it is reported in its own column and never mixed in.

RESSIM_STATES is the output of the ignored test `comp_spe5_dump_report_states` run from month 1:

    SPE5_STATE_OUT=ressim_months.json SPE5_STATE_FIRST=1 SPE5_STATE_LAST=217 \
        cargo test --release --lib comp_spe5_dump_report_states -- --ignored

REFERENCE defaults to opm/compositional/spe5/reference.json. The script prints the markdown used in
docs/SPE5_VALIDATION.md "Comparison with ResSim": the month-24 table, the worst differences over
the shared months, and the monthly table. Nothing in that section is typed by hand.

Conventions, matching tools/jutul/spe5_reference.jl: mean pressure is the arithmetic cell mean;
cell 33 is zero-based; GOR and water cut are at surface conditions from the producer's separator
rates; relative rate differences are |R - J| / max(|R|, |J|).
"""
import json
import statistics
import sys

CELL = 33
BHP_LIMIT_BAR = 68.9476
MONTHS = (12, 16, 24, 48, 96, 144, 192, 204, 205, 206, 207, 208, 209, 210, 211, 212, 213, 214,
          215, 216, 217, 218, 219)


def ressim_reports(path):
    return reports_from_states(json.load(open(path))["states"])


def reports_from_states(states):
    out = {}
    for state in states:
        s = state["snapshot"]
        p = s["pressure"]
        prod = s["well_surface_rates"][0]
        oil = -prod["liquid_m3_per_day"]
        gas = -prod["vapour_m3_per_day"]
        water = -prod["water_m3_per_day"]
        out[state["month"]] = {
            "day": s["time_days"],
            "mean_p_bar": sum(p) / len(p),
            "cell33_p_bar": p[CELL],
            "cell33_sw": s["water"]["saturation"][CELL],
            "cell33_sg": s["vapour_saturation"][CELL],
            "producer_oil_m3_day": oil,
            "producer_gas_m3_day": gas,
            "producer_water_m3_day": water,
            "producer_gor_m3_m3": gas / oil if oil > 0 else None,
            "producer_watercut": water / (oil + water) if oil + water > 0 else None,
            "bhp_controlled": abs(s["well_bhp_bar"][0] - BHP_LIMIT_BAR) < 1e-9,
        }
    return out


def rel(a, b):
    m = max(abs(a), abs(b))
    return abs(a - b) / m if m > 0 else 0.0


def main(ressim_path, reference_path="opm/compositional/spe5/reference.json"):
    r = ressim_reports(ressim_path)
    ref = json.load(open(reference_path))
    j = {i + 1: rep for i, rep in enumerate(ref["reports"])}
    shared = sorted(set(r) & set(j))
    print(f"ResSim months {min(r)}-{max(r)}; reference {len(j)} reports; shared {len(shared)}\n")

    m = 24
    rows = [("Mean pressure, bar", "mean_p_bar", "{:.2f}"),
            ("Cell-33 pressure, bar", "cell33_p_bar", "{:.2f}"),
            ("Cell-33 Sw", "cell33_sw", "{:.4f}"), ("Cell-33 Sg", "cell33_sg", "{:.4f}"),
            ("Producer oil, m³/d", "producer_oil_m3_day", "{:.1f}"),
            ("Producer gas, m³/d", "producer_gas_m3_day", "{:,.0f}"),
            ("Producer water, m³/d", "producer_water_m3_day", "{:.5f}"),
            ("GOR, m³/m³", "producer_gor_m3_m3", "{:.1f}"),
            ("Water cut", "producer_watercut", "{:.3e}")]
    print(f"Month {m} ({j[m]['day']:.1f} d):\n")
    print("| Quantity | ResSim | JutulDarcy |\n|---|---:|---:|")
    for label, key, fmt in rows:
        print(f"| {label} | {fmt.format(r[m][key])} | {fmt.format(j[m][key])} |")

    r_switch = next((k for k in shared if r[k]["bhp_controlled"]), None)
    j_switch = next((k for k in shared if j[k]["producer_control"] != "orat"), None)
    print(f"\nProducer rate-to-BHP switch: ResSim month {r_switch}, JutulDarcy month {j_switch}\n")

    worst = {}
    for k in shared:
        for name, val in (
            ("mean pressure, bar", abs(r[k]["mean_p_bar"] - j[k]["mean_p_bar"])),
            ("cell-33 pressure, bar", abs(r[k]["cell33_p_bar"] - j[k]["cell33_p_bar"])),
            ("producer oil, relative", rel(r[k]["producer_oil_m3_day"], j[k]["producer_oil_m3_day"])),
            ("GOR, relative", rel(r[k]["producer_gor_m3_m3"] or 0, j[k]["producer_gor_m3_m3"] or 0)),
            ("water cut, absolute", abs((r[k]["producer_watercut"] or 0) - (j[k]["producer_watercut"] or 0))),
            ("cell-33 Sg, absolute", abs(r[k]["cell33_sg"] - j[k]["cell33_sg"])),
        ):
            if val > worst.get(name, (-1, 0))[0]:
                worst[name] = (val, k)
    print("| Worst difference over the shared months | Value | Month |\n|---|---:|---:|")
    for name, (val, k) in worst.items():
        shown = f"{100 * val:.1f}%" if "relative" in name else f"{val:.4g}"
        print(f"| {name} | {shown} | {k} |")

    print("\n| month | day | mean p R / J (bar) | cell 33 p R / J (bar) | Sw 33 R / J | Sg 33 R / J "
          "| oil R / J (m³/d) | GOR R / J (m³/m³) | WC R / J | Jutul ministeps / Newton |")
    print("|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|")
    for k in MONTHS:
        if k not in j:
            continue
        jj, d = j[k], j[k]["diagnostics"]
        if k in r:
            rr = r[k]
            cells = (f"{rr['mean_p_bar']:.2f} / {jj['mean_p_bar']:.2f}",
                     f"{rr['cell33_p_bar']:.2f} / {jj['cell33_p_bar']:.2f}",
                     f"{rr['cell33_sw']:.4f} / {jj['cell33_sw']:.4f}",
                     f"{rr['cell33_sg']:.4f} / {jj['cell33_sg']:.4f}",
                     f"{rr['producer_oil_m3_day']:.1f} / {jj['producer_oil_m3_day']:.1f}",
                     f"{rr['producer_gor_m3_m3']:.1f} / {jj['producer_gor_m3_m3']:.1f}",
                     f"{rr['producer_watercut']:.5f} / {jj['producer_watercut']:.5f}")
        else:
            cells = (f"— / {jj['mean_p_bar']:.2f}", f"— / {jj['cell33_p_bar']:.2f}",
                     f"— / {jj['cell33_sw']:.4f}", f"— / {jj['cell33_sg']:.4f}",
                     f"— / {jj['producer_oil_m3_day']:.1f}", f"— / {jj['producer_gor_m3_m3']:.1f}",
                     f"— / {jj['producer_watercut']:.5f}")
        print(f"| {k} | {jj['day']:.2f} | " + " | ".join(cells)
              + f" | {d['ministeps']} / {d['newton_iterations']} |")


def ministeps(path):
    data = json.load(open(path))
    config = data.get("configuration") or {"well_volume_m3": data.get("well_volume_m3")}
    print(f"configuration: {json.dumps(config)}\n")
    groups = {}
    for m in data["ministeps"]:
        key = "205–215" if m["month"] <= 215 else str(m["month"])
        groups.setdefault(key, []).append(m)
    print("| Month | Accepted / failed ministeps | Median accepted dt (d) "
          "| Accepted with MB > 1e-8 / > 5e-8 | Accepted with CNV > 1e-4 "
          "| Σ accepted MB, worst hydrocarbon | Σ accepted Jutul water \"MB\" |")
    print("|---|---|---|---|---|---|---|")
    total = {}
    for key, ms in groups.items():
        accepted = [m for m in ms if m["success"]]
        failed = len(ms) - len(accepted)
        final = [m["newton"][-1] for m in accepted]

        def hc(d):
            return {k: v for k, v in d.items() if k != "Water"}
        mb = [max(hc(n["MB"]).values()) for n in final]
        cnv = [max(hc(n["CNV"]).values()) for n in final]
        sums = {}
        for n in final:
            for k, v in n["MB"].items():
                sums[k] = sums.get(k, 0.0) + v
                total[k] = total.get(k, 0.0) + v
        worst = max((k for k in sums if k != "Water"), key=sums.get, default=None)
        median = statistics.median(m["dt_days"] for m in accepted) if accepted else float("nan")
        print(f"| {key} | {len(accepted):,} / {failed:,} | {median:.3g} "
              f"| {sum(v > 1e-8 for v in mb)} / {sum(v > 5e-8 for v in mb)} "
              f"| {sum(v > 1e-4 for v in cnv)} "
              f"| {sums[worst]:.2g} ({worst}) | {sums.get('Water', 0.0):.2g} |")
    worst = max((k for k in total if k != "Water"), key=total.get)
    print(f"\nΣ accepted MB over all months, worst hydrocarbon: {total[worst]:.3g} ({worst}); "
          f"Jutul water \"MB\": {total.get('Water', 0.0):.3g}")


def reference_reports(path):
    ref = json.load(open(path))
    return {i + 1: rep for i, rep in enumerate(ref["reports"])}, ref


def references(args):
    runs = []
    for arg in args:
        label, path = arg.split("=", 1)
        reports, ref = reference_reports(path)
        try:
            config = json.load(open(path + ".config.json"))
        except FileNotFoundError:
            config = None
        runs.append((label, reports, ref, config))
    print("| Run | Well volumes (m³) PROD / INJW / INJG | Max dt (d) | Completed | Ministeps / Newton "
          "/ failed | Month 216 / 217 / 218 / 219 ministeps | First declared stop |")
    print("|---|---|---|---|---|---|---|")
    for label, reports, ref, config in runs:
        if config:
            v = config["well_volumes_m3"]
            vols = " / ".join(f"{v[w][0]:.3g}" for w in ("PROD", "INJW", "INJG"))
            maxdt = config["max_timestep_days"]
            maxdt = "report" if isinstance(maxdt, str) else f"{maxdt:g}"
        else:
            vols, maxdt = "not recorded", "not recorded"
        d = [r["diagnostics"] for r in reports.values()]
        work = (f"{sum(x['ministeps'] for x in d):,} / {sum(x['newton_iterations'] for x in d):,} / "
                f"{sum(x['failed_ministeps'] for x in d):,}")
        crossing = " / ".join(f"{reports[m]['diagnostics']['ministeps']:,}" if m in reports else "—"
                              for m in (216, 217, 218, 219))
        stop = ref.get("stopped") or {}
        stop = f"{stop.get('day')} d, {stop.get('reason')}" if stop else "none"
        print(f"| {label} | {vols} | {maxdt} | {ref.get('completed')} ({len(reports)} reports) "
              f"| {work} | {crossing} | {stop} |")

    base_label, base = runs[0][0], runs[0][1]
    print(f"\nWorst difference from `{base_label}`, months 1–217 / all shared months (month):\n")
    quantities = (
        ("mean p (bar)", lambda r: r["mean_p_bar"], False),
        ("cell-33 p (bar)", lambda r: r["cell33_p_bar"], False),
        ("producer oil (rel.)", lambda r: r["producer_oil_m3_day"], True),
        ("GOR (rel.)", lambda r: r["producer_gor_m3_m3"] or 0.0, True),
        ("water cut (abs.)", lambda r: r["producer_watercut"] or 0.0, False),
        ("cell-33 Sg (abs.)", lambda r: r["cell33_sg"], False),
    )
    print("| Run | " + " | ".join(q[0] for q in quantities) + " |")
    print("|---|" + "---|" * len(quantities))
    for label, reports, _, _ in runs[1:]:
        cells = []
        for _, get, relative in quantities:
            out = []
            for last in (217, 240):
                worst, at = -1.0, None
                for m in sorted(set(base) & set(reports)):
                    if m > last:
                        continue
                    a, b = get(base[m]), get(reports[m])
                    v = rel(a, b) if relative else abs(a - b)
                    if v > worst:
                        worst, at = v, m
                shown = f"{100 * worst:.1f}%" if relative else f"{worst:.3g}"
                out.append(f"{shown} ({at})")
            cells.append(" / ".join(out))
        print(f"| {label} | " + " | ".join(cells) + " |")


def validate_run(path, volume, max_dt, not_before=None):
    import math
    import os
    problems = []
    config_path = path + ".config.json"
    try:
        ref = json.load(open(path))
        config = json.load(open(config_path))
    except (OSError, ValueError) as err:
        sys.exit(f"{path}: unreadable output or configuration: {err}")
    if ref.get("schema") != "ressim-spe5-reference/1":
        problems.append(f"schema {ref.get('schema')!r}")
    if ref.get("completed") is not True or ref.get("failed_at") is not None:
        problems.append(f"completed={ref.get('completed')!r} failed_at={ref.get('failed_at')!r}")
    reports = ref.get("reports") or []
    if len(reports) != 240:
        problems.append(f"{len(reports)} reports, expected 240")
    elif any(not math.isclose(r["day"], 30.4375 * (i + 1), abs_tol=1e-9)
             for i, r in enumerate(reports)):
        problems.append("report days are not the 30.4375 d schedule")
    if not math.isclose(float(ref.get("reached_day") or 0.0), 7305.0, abs_tol=1e-9):
        problems.append(f"reached_day={ref.get('reached_day')!r}")

    def expected(setting, default_label):
        return default_label if setting == "default" else float(setting)
    want_volume = expected(volume, "jutuldarcy-default")
    want_dt = expected(max_dt, "jutuldarcy-default (1 year)")
    if config.get("well_volume_setting") != want_volume:
        problems.append(f"well_volume_setting {config.get('well_volume_setting')!r}, "
                        f"requested {want_volume!r}")
    if config.get("max_timestep_days") != want_dt:
        problems.append(f"max_timestep_days {config.get('max_timestep_days')!r}, "
                        f"requested {want_dt!r}")
    if want_volume != "jutuldarcy-default":
        vols = [v for w in config.get("well_volumes_m3", {}).values() for v in w]
        if len(vols) != 3 or any(v != want_volume for v in vols):
            problems.append(f"resolved well volumes {vols}, requested {want_volume}")
    if config.get("jutuldarcy") != ref.get("jutuldarcy"):
        problems.append("configuration and reference name different JutulDarcy versions")
    if not_before is not None:
        start = os.path.getmtime(not_before)
        for f in (path, config_path):
            if os.path.getmtime(f) < start:
                problems.append(f"{f} is older than {not_before} (stale)")
    if problems:
        sys.exit(f"{path}: " + "; ".join(problems))
    print(f"{path}: valid (volume {volume}, max dt {max_dt}, 240 reports through 7305 d)")


def replay_reports(path):
    data = json.load(open(path))
    if data.get("schema") != "ressim-spe5-replay/1":
        sys.exit(f"{path}: schema {data.get('schema')!r}, expected ressim-spe5-replay/1")
    reports = reports_from_states(data["months"])
    for m in data["months"]:
        reports[m["month"]]["cumulative"] = \
            m["snapshot"]["cumulative_well_surface_production_volumes"][0]
    return reports, data


def timestep_convergence(paths, last=None):
    labels = ("ResSim report", "ResSim 5 d", "ResSim 0.5 d",
              "Jutul report", "Jutul 5 d", "Jutul 0.5 d")
    runs, replays = {}, {}
    for label, path in zip(labels[:3], paths[:3]):
        runs[label], replays[label] = replay_reports(path)
    for label, path in zip(labels[3:], paths[3:6]):
        runs[label] = reference_reports(path)[0]
    if last is None:
        last = min(max(runs[k]) for k in labels[:3])

    print("| ResSim run | Max dt (d) | Months completed | Accepted steps / failed attempts "
          "| Newton (accepted) | Wall (s) | Max \\|signed balance\\| | Stopped |")
    print("|---|---|---|---|---|---|---|---|")
    for label in labels[:3]:
        d = replays[label]
        ms, st = d["months"], d["settings"]
        balance = max(abs(v) for m in ms for v in m["balance_rel"])
        stop = d["stopped"]
        stop = (f"month {stop['month']}, day {stop['day']:.3f}: {stop['reason']}, "
                f"{stop['failure'].split('scaled residual ')[-1].rstrip(')}\" ')}"
                if stop else "no")
        cap = st["max_dt_days"]
        print(f"| {label} | {'report' if cap is None else f'{cap:g}'} "
              f"| {ms[0]['month']}–{ms[-1]['month']} "
              f"| {sum(m['accepted_steps'] for m in ms):,} / {sum(m['failed_attempts'] for m in ms):,} "
              f"| {sum(m['accepted_newton'] for m in ms):,} | {sum(m['wall_s'] for m in ms):,.0f} "
              f"| {balance:.1e} | {stop} |")

    quantities = (
        ("mean p (bar)", lambda r: r["mean_p_bar"], False),
        ("cell-33 p (bar)", lambda r: r["cell33_p_bar"], False),
        ("producer oil (rel.)", lambda r: r["producer_oil_m3_day"], True),
        ("GOR (rel.)", lambda r: r["producer_gor_m3_m3"] or 0.0, True),
        ("water cut (abs.)", lambda r: r["producer_watercut"] or 0.0, False),
        ("cell-33 Sg (abs.)", lambda r: r["cell33_sg"], False),
    )
    pairs = ((0, 2), (1, 2), (3, 5), (4, 5), (0, 3), (1, 4), (2, 5))
    print(f"\nWorst difference over months 1–{last} (month):\n")
    print("| Pair | " + " | ".join(q[0] for q in quantities) + " | Σ oil |")
    print("|---|" + "---|" * (len(quantities) + 1))
    for a, b in pairs:
        A, B = runs[labels[a]], runs[labels[b]]
        months = [m for m in range(1, last + 1) if m in A and m in B]
        if len(months) != last:
            sys.exit(f"{labels[a]} / {labels[b]}: months 1–{last} not all present")
        cells = []
        for _, get, relative in quantities:
            worst, at = max(((rel(get(A[m]), get(B[m])) if relative
                              else abs(get(A[m]) - get(B[m])), m) for m in months))
            cells.append(f"{100 * worst:.1f}% ({at})" if relative else f"{worst:.3g} ({at})")
        oil = rel(sum(A[m]["producer_oil_m3_day"] for m in months),
                  sum(B[m]["producer_oil_m3_day"] for m in months))
        print(f"| {labels[a]} vs {labels[b]} | " + " | ".join(cells) + f" | {100 * oil:.2f}% |")

    switch = {}
    for label in labels:
        r = runs[label]
        bhp = (lambda x: x["bhp_controlled"]) if label.startswith("ResSim") else \
            (lambda x: x["producer_control"] != "orat")
        switch[label] = next((m for m in sorted(r) if bhp(r[m])), None)
    print("\nProducer rate-to-BHP switch month: "
          + ", ".join(f"{k} {v}" for k, v in switch.items()))
    base = runs[labels[2]][last]["cumulative"]
    for label in labels[:2]:
        c = runs[label][last]["cumulative"]
        print(f"{label} vs {labels[2]}, cumulative surface production at month {last}: "
              f"oil {100 * rel(c['liquid_m3'], base['liquid_m3']):.3f}%, "
              f"gas {100 * rel(c['vapour_m3'], base['vapour_m3']):.3f}%")
    print("\nCell-33 Sg by month, " + " / ".join(labels) + ":\n")
    print("| Month | " + " | ".join(labels) + " |\n|---:|" + "---:|" * len(labels))
    for m in range(max(1, last - 6), last + 7):
        print(f"| {m} | " + " | ".join(f"{runs[k][m]['cell33_sg']:.4f}" if m in runs[k] else "—"
                                       for k in labels) + " |")


if __name__ == "__main__":
    if sys.argv[1] == "--validate-run":
        validate_run(*sys.argv[2:6])
    elif sys.argv[1] == "--references":
        references(sys.argv[2:])
    elif sys.argv[1] == "--timestep-convergence":
        timestep_convergence(sys.argv[2:8], int(sys.argv[8]) if len(sys.argv) > 8 else None)
    elif sys.argv[1] == "--ministeps":
        ministeps(sys.argv[2])
    else:
        main(*sys.argv[1:3])
