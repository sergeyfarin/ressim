use super::*;

pub(super) fn small_dt_hotspot_neighborhood_indices(
    sim: &ReservoirSimulator,
    center_idx: usize,
) -> Vec<usize> {
    let center_i = center_idx % sim.nx;
    let center_j = (center_idx / sim.nx) % sim.ny;
    let center_k = center_idx / (sim.nx * sim.ny);
    let mut indices = Vec::new();

    for dj in -1_i32..=1 {
        for di in -1_i32..=1 {
            let neighbor_i = center_i as i32 + di;
            let neighbor_j = center_j as i32 + dj;
            if neighbor_i < 0
                || neighbor_i >= sim.nx as i32
                || neighbor_j < 0
                || neighbor_j >= sim.ny as i32
            {
                continue;
            }
            let idx =
                center_k * sim.nx * sim.ny + neighbor_j as usize * sim.nx + neighbor_i as usize;
            indices.push(idx);
        }
    }

    if center_k > 0 {
        indices.push(center_idx - sim.nx * sim.ny);
    }
    if center_k + 1 < sim.nz {
        indices.push(center_idx + sim.nx * sim.ny);
    }

    indices.sort_unstable();
    indices.dedup();
    indices
}

pub(super) fn maybe_trace_small_dt_hotspot_neighborhood(
    sim: &mut ReservoirSimulator,
    verbose: bool,
    context: &str,
    dt_days: f64,
    previous_state: &FimState,
    candidate_state: &FimState,
    hotspot_site: FimHotspotSite,
) {
    const SMALL_DT_NEIGHBORHOOD_TRACE_THRESHOLD_DAYS: f64 = 1.0e-3;

    if dt_days > SMALL_DT_NEIGHBORHOOD_TRACE_THRESHOLD_DAYS {
        return;
    }

    let FimHotspotSite::Cell(center_idx) = hotspot_site else {
        return;
    };

    let center_i = center_idx % sim.nx;
    let center_j = (center_idx / sim.nx) % sim.ny;
    let center_k = center_idx / (sim.nx * sim.ny);
    fim_trace!(
        sim,
        verbose,
        "      hotspot-nbhd {} dt={:.6} center=cell{}({},{},{})",
        context,
        dt_days,
        center_idx,
        center_i,
        center_j,
        center_k,
    );

    for idx in small_dt_hotspot_neighborhood_indices(sim, center_idx) {
        let before_cell = previous_state.cell(idx);
        let after_cell = candidate_state.cell(idx);
        let before = previous_state.derive_cell(sim, idx);
        let after = candidate_state.derive_cell(sim, idx);
        let i = idx % sim.nx;
        let j = (idx / sim.nx) % sim.ny;
        let k = idx / (sim.nx * sim.ny);
        fim_trace!(
            sim,
            verbose,
            "        cell{}({},{},{}) {}->{} p={:.2}->{:.2} dP={:+.2} sw={:.4}->{:.4} dSw={:+.4} so={:.4}->{:.4} dSo={:+.4} sg={:.4}->{:.4} dSg={:+.4} rs={:.4}->{:.4}",
            idx,
            i,
            j,
            k,
            hydrocarbon_state_label(before_cell.regime),
            hydrocarbon_state_label(after_cell.regime),
            before_cell.pressure_bar,
            after_cell.pressure_bar,
            after_cell.pressure_bar - before_cell.pressure_bar,
            before_cell.sw,
            after_cell.sw,
            after_cell.sw - before_cell.sw,
            before.so,
            after.so,
            after.so - before.so,
            before.sg,
            after.sg,
            after.sg - before.sg,
            before.rs,
            after.rs,
        );
    }
}
