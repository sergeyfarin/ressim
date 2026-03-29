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
        let rs_sat = table.interpolate(pressure_bar).rs_m3m3;
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

    if !sim.three_phase_mode || sim.pvt_table.is_none() {
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
            FimFlashResult {
                regime,
                so: (1.0 - sw - sg).max(0.0),
                sg,
                rs: table.interpolate(pressure_bar).rs_m3m3,
                bubble_point_bar,
            }
        }
        HydrocarbonState::Undersaturated => FimFlashResult {
            regime,
            so: total_hydrocarbon_saturation,
            sg: 0.0,
            rs: hydrocarbon_var.max(0.0),
            bubble_point_bar,
        },
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

        let regime = classify_cell_regime(&sim, 150.0, 0.0, 12.0);
        assert_eq!(regime, HydrocarbonState::Undersaturated);
    }
}
