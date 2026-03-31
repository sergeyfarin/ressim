use crate::ReservoirSimulator;
use crate::fim::state::HydrocarbonState;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct FimFlashResult {
    pub(crate) regime: HydrocarbonState,
    pub(crate) so: f64,
    pub(crate) sg: f64,
    pub(crate) rs: f64,
    pub(crate) bubble_point_bar: f64,
}

pub(crate) fn classify_cell_regime(
    sim: &ReservoirSimulator,
    pressure_bar: f64,
    gas_saturation: f64,
    rs_sm3_sm3: f64,
    drsdt0_base_rs: Option<f64>,
) -> HydrocarbonState {
    if !sim.three_phase_mode {
        return HydrocarbonState::Saturated;
    }

    let Some(table) = &sim.pvt_table else {
        return HydrocarbonState::Saturated;
    };

    if gas_saturation > 1e-9 {
        HydrocarbonState::Saturated
    } else {
        let mut rs_sat = table.interpolate(pressure_bar).rs_m3m3;
        if let Some(base_rs) = drsdt0_base_rs {
            rs_sat = rs_sat.min(base_rs);
        }
        if rs_sm3_sm3 < rs_sat - 1e-6 {
            HydrocarbonState::Undersaturated
        } else {
            HydrocarbonState::Saturated
        }
    }
}

pub(crate) fn resolve_cell_flash(
    sim: &ReservoirSimulator,
    pressure_bar: f64,
    sw: f64,
    hydrocarbon_var: f64,
    regime: HydrocarbonState,
    drsdt0_base_rs: Option<f64>,
) -> FimFlashResult {
    let total_hydrocarbon_saturation = (1.0 - sw).max(0.0);
    let bubble_point_bar = sim
        .pvt_table
        .as_ref()
        .map(|table| match regime {
            HydrocarbonState::Saturated => pressure_bar,
            HydrocarbonState::Undersaturated => {
                table.bubble_point_pressure(hydrocarbon_var.max(0.0))
            }
        })
        .unwrap_or(pressure_bar);

    if !sim.three_phase_mode {
        return FimFlashResult {
            regime,
            so: total_hydrocarbon_saturation,
            sg: 0.0,
            rs: 0.0,
            bubble_point_bar,
        };
    }

    if sim.pvt_table.is_none() {
        let sg = match regime {
            HydrocarbonState::Saturated => hydrocarbon_var.clamp(0.0, total_hydrocarbon_saturation),
            HydrocarbonState::Undersaturated => 0.0,
        };
        return FimFlashResult {
            regime,
            so: (1.0 - sw - sg).max(0.0),
            sg,
            rs: 0.0,
            bubble_point_bar,
        };
    }

    let table = sim.pvt_table.as_ref().expect("checked above");
    match regime {
        HydrocarbonState::Saturated => {
            let sg = hydrocarbon_var.clamp(0.0, total_hydrocarbon_saturation);
            let mut rs = table.interpolate(pressure_bar).rs_m3m3;
            if let Some(base_rs) = drsdt0_base_rs {
                rs = rs.min(base_rs);
            }
            FimFlashResult {
                regime,
                so: (1.0 - sw - sg).max(0.0),
                sg,
                rs,
                bubble_point_bar,
            }
        }
        HydrocarbonState::Undersaturated => {
            let mut rs_cap = table.interpolate(pressure_bar).rs_m3m3.max(0.0);
            if let Some(base_rs) = drsdt0_base_rs {
                rs_cap = rs_cap.min(base_rs.max(0.0));
            }

            let rs_trial = hydrocarbon_var.max(0.0);
            if rs_trial <= rs_cap + 1e-6 {
                return FimFlashResult {
                    regime,
                    so: total_hydrocarbon_saturation,
                    sg: 0.0,
                    rs: rs_trial,
                    bubble_point_bar,
                };
            }

            let bo_trial = sim.get_b_o_for_rs(pressure_bar, rs_trial).max(1e-9);
            let dissolved_gas_sc = total_hydrocarbon_saturation * rs_trial / bo_trial;
            let (sg, so, rs) = sim.split_gas_inventory_after_transport(
                pressure_bar,
                1.0,
                sw,
                0.0,
                dissolved_gas_sc,
                drsdt0_base_rs,
            );

            FimFlashResult {
                regime: if sg > 1e-12 {
                    HydrocarbonState::Saturated
                } else {
                    HydrocarbonState::Undersaturated
                },
                so,
                sg,
                rs,
                bubble_point_bar,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::ReservoirSimulator;
    use crate::pvt::{PvtRow, PvtTable};

    use super::*;

    #[test]
    fn classifies_undersaturated_cell_without_free_gas() {
        let mut sim = ReservoirSimulator::new(1, 1, 1, 0.2);
        sim.set_three_phase_mode_enabled(true);
        sim.pvt_table = Some(PvtTable::new(
            vec![
                PvtRow {
                    p_bar: 100.0,
                    rs_m3m3: 10.0,
                    bo_m3m3: 1.1,
                    mu_o_cp: 1.2,
                    bg_m3m3: 0.01,
                    mu_g_cp: 0.02,
                },
                PvtRow {
                    p_bar: 200.0,
                    rs_m3m3: 20.0,
                    bo_m3m3: 1.0,
                    mu_o_cp: 1.1,
                    bg_m3m3: 0.005,
                    mu_g_cp: 0.02,
                },
            ],
            sim.pvt.c_o,
        ));

        let regime = classify_cell_regime(&sim, 150.0, 0.0, 12.0, None);
        assert_eq!(regime, HydrocarbonState::Undersaturated);
    }
}
