"""50-digit continuation of the #116 M0 split to its collapse or trivial cutoff.

    uv run --no-project --with mpmath python -I tools/jutul/spe5_m1_split_mp.py \
        m0_gibbs_points.json m1_split_mp.json

`m0_gibbs_points.json` is written by the ignored ResSim test
`comp_spe5_m0_branch_comparison` (`SPE5_TRACE_OUT=DIR`). This script takes the five
`switch -1e-15` points, i.e. the two-phase side of each adjacent-float label change, and continues
the physical split from there in 50-digit arithmetic along the pressure path at fixed composition,
in the direction that raises pressure towards the single-phase side.

Why fixed beta: near these states the equilibrium Jacobian's small singular value is almost pure
`beta`. Solving for `(ln K, p)` at fixed beta avoids that direction, so the branch can be followed
through the region where a Newton solve in `(ln K, beta)` stalls. The quantities recorded at every
step are `beta`, `sum ln^2 K`, `g_split - g_feed` (lower feed root), phase molar volumes and
densities, the mixture molar volume, the vapour saturation, and the implied component inventory
per m3 of pore volume. Two events are located:

- the production trivial-solution cutoff, `sum ln^2 K = 1e-5`, by bisection in beta;
- the actual collapse, i.e. the endpoint where beta reaches 0 or 1 (the phase amount vanishes).

Each case records the actual composition change between the two captured Newton iterates and the
fixed-beta pressure offset from re-solving with each endpoint's composition. The output carries a
top-level `complete` flag: an unclosed case is listed in `incomplete_cases`, and the script exits
non-zero after writing it, so a truncated continuation cannot be consumed as a finished M1 result.

The engine-side consequences (inventory and face-flux changes against the standard single-phase
flash) are measured by `comp_spe5_m1_cutoff_jump`, which validates `complete`, `reached_endpoint`
and the endpoint beta before accepting the file. Nothing here changes production code; the script
is a measurement.

The 50-digit EOS and flash routines are the independent implementation in `spe5_flash_mp.py`,
written from the textbook Peng-Robinson equations rather than from ResSim's code.
"""
import json
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

import mpmath as mp  # noqa: E402

from spe5_flash_mp import cubic_roots, ln_phi, residual, setup  # noqa: E402
from spe5_m1_points import point_for  # noqa: E402

mp.mp.dps = 60

CUTOFF = mp.mpf("1e-5")


def gibbs(x, ln_phi_values):
    return sum(
        x[i] * (mp.log(x[i]) + ln_phi_values[i])
        for i in range(len(x))
        if x[i] > 0
    )


def feed_gibbs_roots(eos, z, p):
    """Dimensionless Gibbs energy of the feed on every real cubic root above B."""
    n = eos["n"]
    a = sum(
        z[i] * z[j] * eos["a_ij"][i][j]
        for i in range(n)
        for j in range(n)
    ) * p
    b = sum(z[i] * eos["b_i"][i] for i in range(n)) * p
    roots = cubic_roots(a, b)
    out = []
    for z_root in roots:
        bi = [value * p for value in eos["b_i"]]
        b_mix = sum(z[i] * bi[i] for i in range(n))
        ai = [
            sum(z[j] * eos["a_ij"][i][j] for j in range(n)) * p
            for i in range(n)
        ]
        a_mix = sum(z[i] * ai[i] for i in range(n))
        s2 = mp.sqrt(2)
        log_term = mp.log((z_root + (1 + s2) * b_mix) / (z_root + (1 - s2) * b_mix))
        ln_phi_values = [
            bi[i] / b_mix * (z_root - 1)
            - mp.log(z_root - b_mix)
            - a_mix / (2 * s2 * b_mix) * (2 * ai[i] / a_mix - bi[i] / b_mix) * log_term
            for i in range(n)
        ]
        out.append(gibbs(z, ln_phi_values))
    return out


def solve_fixed_beta(eos, z, ln_k0, p0, beta):
    """Solve the equilibrium equations for `(ln K, p)` at fixed beta."""
    n = eos["n"]
    u = list(ln_k0) + [p0]
    step = mp.mpf(10) ** -30
    for _ in range(80):
        values, state = residual(eos, z, u[n], list(u[:n]) + [beta])[:2]
        norm = max(abs(value) for value in values)
        if norm < mp.mpf(10) ** -35:
            return u[:n], u[n], state, norm
        matrix = mp.matrix(n + 1, n + 1)
        for column in range(n + 1):
            up, down = list(u), list(u)
            up[column] += step
            down[column] -= step
            f_up, _ = residual(eos, z, up[n], list(up[:n]) + [beta])[:2]
            f_down, _ = residual(eos, z, down[n], list(down[:n]) + [beta])[:2]
            for row in range(n + 1):
                matrix[row, column] = (f_up[row] - f_down[row]) / (2 * step)
        try:
            delta = mp.lu_solve(matrix, mp.matrix([-value for value in values]))
        except Exception:
            return None
        u = [u[i] + delta[i] for i in range(n + 1)]
    return None


def branch_state(eos, z, molar_mass, ln_k, p, beta):
    n = eos["n"]
    values, state = residual(eos, z, p, list(ln_k) + [beta])[:2]
    sum_ln_k_sq = sum(value * value for value in ln_k)
    feed = feed_gibbs_roots(eos, z, p)
    liquid_ln_phi, z_liquid = ln_phi(eos, state["x"], p, "liquid")
    vapour_ln_phi, z_vapour = ln_phi(eos, state["y"], p, "vapour")
    g_split = (1 - beta) * gibbs(state["x"], liquid_ln_phi) + beta * gibbs(
        state["y"], vapour_ln_phi
    )
    gas_constant, temperature = eos["R"], eos["T"]
    v_liquid = z_liquid * gas_constant * temperature / p
    v_vapour = z_vapour * gas_constant * temperature / p
    v_mixture = (1 - beta) * v_liquid + beta * v_vapour
    c_liquid, c_vapour = 1 / v_liquid, 1 / v_vapour
    molar_mass_liquid = sum(state["x"][i] * molar_mass[i] for i in range(n))
    molar_mass_vapour = sum(state["y"][i] * molar_mass[i] for i in range(n))
    return {
        "beta": beta,
        "p_pa": p,
        "sum_ln_k_sq": sum_ln_k_sq,
        "max_residual": max(abs(value) for value in values),
        "g_split_minus_lower_feed": g_split - min(feed),
        "g_split_minus_li_feed": g_split - feed[0],
        "Z_L": z_liquid,
        "Z_V": z_vapour,
        "v_mix": v_mixture,
        "vapour_saturation": beta * v_vapour / v_mixture,
        "liquid_molar_density": c_liquid,
        "vapour_molar_density": c_vapour,
        "liquid_mass_density": c_liquid * molar_mass_liquid,
        "vapour_mass_density": c_vapour * molar_mass_vapour,
        "x": list(state["x"]),
        "y": list(state["y"]),
        "inventory_per_m3": [float(value / v_mixture) for value in z],
        "ln_k": list(ln_k),
    }


def trace(eos, z, molar_mass, ln_k, p, beta, direction, max_steps=500):
    """Follow the physical branch at fixed beta, in the direction that moves p toward single phase."""
    rows = []
    step = mp.mpf("0.01")
    current_beta, current_ln_k, current_p = beta, list(ln_k), p
    for _ in range(max_steps):
        next_beta = current_beta + direction * step
        while next_beta <= 0 or next_beta >= 1:
            step /= 2
            next_beta = current_beta + direction * step
        solved = solve_fixed_beta(eos, z, current_ln_k, current_p, next_beta)
        if solved is None:
            step /= 2
            if step < mp.mpf("1e-12"):
                break
            continue
        next_ln_k, next_p, _, _ = solved
        sum_ln_k_sq = sum(value * value for value in next_ln_k)
        # Reject a jump onto the trivial solution unless the phase amount is genuinely vanishing.
        if sum_ln_k_sq < mp.mpf("1e-10") and 1e-4 < next_beta < 1 - 1e-4:
            step /= 2
            if step < mp.mpf("1e-12"):
                break
            continue
        current_beta, current_ln_k, current_p = next_beta, next_ln_k, next_p
        rows.append(branch_state(eos, z, molar_mass, current_ln_k, current_p, current_beta))
        if current_beta < mp.mpf("1e-10") or current_beta > 1 - mp.mpf("1e-10"):
            break
        if step < mp.mpf("0.02"):
            step *= 2
    return rows


def bisect_cutoff(eos, z, molar_mass, low, high):
    """Locate sum ln^2 K = 1e-5 between two adjacent branch points."""
    low_beta, high_beta = low["beta"], high["beta"]
    low_state, high_state = low, high
    low_high = sum(value * value for value in low["ln_k"]) > CUTOFF
    for _ in range(120):
        mid = (low_beta + high_beta) / 2
        solved = solve_fixed_beta(
            eos,
            z,
            [mp.mpf(value) for value in low_state["ln_k"]],
            mp.mpf(low_state["p_pa"]),
            mid,
        )
        if solved is None:
            return None
        ln_k, p, _, _ = solved
        state = branch_state(eos, z, molar_mass, ln_k, p, mid)
        if (state["sum_ln_k_sq"] > CUTOFF) == low_high:
            low_beta, low_state = mid, state
        else:
            high_beta, high_state = mid, state
    if abs(low_state["sum_ln_k_sq"] - CUTOFF) < abs(high_state["sum_ln_k_sq"] - CUTOFF):
        return low_state
    return high_state


def normalized_z(point):
    if point is None:
        return None
    z = [mp.mpf(value) for value in point["z"]]
    total = sum(z)
    return [value / total for value in z]


def composition_sensitivity(eos, z_switch, ln_k, switch_p, beta, z_two, z_single):
    """Actual composition change on the Newton cycle and its fixed-beta pressure effect."""
    if z_two is None or z_single is None:
        return None
    max_change = max(abs(a - b) for a, b in zip(z_two, z_single))
    offsets = {}
    for name, z in (("two_phase", z_two), ("single", z_single)):
        solved = solve_fixed_beta(eos, z, ln_k, switch_p, beta)
        offsets[name] = None if solved is None else float(solved[1] - switch_p)
    return {
        "max_component_change_two_to_single": float(max_change),
        "max_component_change_switch_to_two": float(
            max(abs(a - b) for a, b in zip(z_switch, z_two))
        ),
        "max_component_change_switch_to_single": float(
            max(abs(a - b) for a, b in zip(z_switch, z_single))
        ),
        "fixed_beta_p_offset_pa": offsets,
    }


def main(in_path, out_path):
    data = json.load(open(in_path))
    fluid = data["fluid"]
    molar_mass = [mp.mpf(component["molar_mass_kg_per_mol"]) for component in fluid["components"]]
    cases = []
    for point in data["points"]:
        tag = point["tag"]
        if not tag.endswith("switch -1e-15") or "split_seed" not in point:
            continue
        prefix = tag[: -len(" switch -1e-15")]
        cell = point["cell"]
        eos = setup(fluid, point["temperature_k"])
        z = [mp.mpf(value) for value in point["z"]]
        z = [value / sum(z) for value in z]
        ln_k = [mp.mpf(value) for value in point["split_seed"]["ln_k"]]
        beta = mp.mpf(point["split_seed"]["beta"])
        switch_p = mp.mpf(point["p_pa"])
        solved = solve_fixed_beta(eos, z, ln_k, switch_p, beta)
        if solved is None:
            cases.append({"case": prefix, "cell": cell, "error": "start did not converge"})
            continue
        ln_k, switch_p, _, _ = solved

        # The direction that raises pressure is the one towards the single-phase side.
        direction = None
        for candidate in (mp.mpf(1), mp.mpf(-1)):
            probe = solve_fixed_beta(eos, z, ln_k, switch_p, beta + candidate * mp.mpf("1e-6"))
            if probe is not None and probe[1] > switch_p:
                direction = candidate
                break
        if direction is None:
            cases.append({"case": prefix, "cell": cell, "error": "no p-increasing direction"})
            continue

        rows = trace(eos, z, molar_mass, ln_k, switch_p, beta, direction)
        cutoff = None
        for low, high in zip(rows, rows[1:]):
            if (low["sum_ln_k_sq"] - CUTOFF) * (high["sum_ln_k_sq"] - CUTOFF) <= 0:
                cutoff = bisect_cutoff(eos, z, molar_mass, low, high)
                break
        single = point_for(data["points"], prefix, cell, "iterate 20 (single side)")
        if single is None:
            single = point_for(data["points"], prefix, cell, "iterate 19 (single side)")
        two_phase = point_for(data["points"], prefix, cell, "iterate 19 (two-phase side)")
        if two_phase is None:
            two_phase = point_for(data["points"], prefix, cell, "iterate 20 (two-phase side)")
        sensitivity = composition_sensitivity(
            eos,
            z,
            ln_k,
            switch_p,
            beta,
            normalized_z(two_phase),
            normalized_z(single),
        )
        last = rows[-1] if rows else None
        record = {
            "case": prefix,
            "cell": cell,
            "temperature_k": point["temperature_k"],
            "z": [float(value) for value in z],
            "switch_p_pa": float(switch_p),
            "switch_beta": float(beta),
            "switch_sum_ln_k_sq": float(sum(value * value for value in ln_k)),
            "switch_standard_label": point["standard_label"],
            "two_phase_p_pa": two_phase["p_pa"] if two_phase else None,
            "single_p_pa": single["p_pa"] if single else None,
            "single_standard_label": single["standard_label"] if single else None,
            "composition_sensitivity": sensitivity,
            "direction": int(direction),
            "start": branch_state(eos, z, molar_mass, ln_k, switch_p, beta),
            "rows": rows,
            "trivial_cutoff": cutoff,
            "collapse": last,
            "reached_endpoint": bool(last and (last["beta"] < 1e-9 or last["beta"] > 1 - 1e-9)),
        }
        cases.append(record)
        endpoint = "none" if last is None else f"beta {mp.nstr(last['beta'], 10)}"
        cutoff_text = (
            "none"
            if cutoff is None
            else f"beta {mp.nstr(cutoff['beta'], 10)}, p {mp.nstr(mp.mpf(cutoff['p_pa']) / 1e5, 12)} bar"
        )
        print(
            f"{prefix} cell {cell}: switch beta {mp.nstr(beta, 10)}, "
            f"sum ln^2 K {mp.nstr(sum(value * value for value in ln_k), 4)}; "
            f"cutoff {cutoff_text}; collapse {endpoint}"
        )

    incomplete = [
        f"{record['case']} cell {record['cell']}"
        for record in cases
        if not record.get("reached_endpoint", False)
    ]
    complete = not incomplete and len(cases) == 8

    def plain(value):
        if isinstance(value, mp.mpf):
            return float(value)
        if isinstance(value, dict):
            return {key: plain(item) for key, item in value.items()}
        if isinstance(value, list):
            return [plain(item) for item in value]
        return value

    with open(out_path, "w") as handle:
        json.dump(
            {
                "schema": "ressim-spe5-m1-split-mp/1",
                "source": os.path.basename(in_path),
                "digits": mp.mp.dps,
                "complete": complete,
                "incomplete_cases": incomplete,
                "fluid": fluid,
                "cases": plain(cases),
            },
            handle,
            indent=1,
        )
    if not complete:
        print(f"incomplete continuation: {', '.join(incomplete) or 'not all 8 cases'}")
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1], sys.argv[2]))
