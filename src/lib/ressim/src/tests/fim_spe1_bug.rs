use crate::ReservoirSimulator;
use crate::well_control::*;

#[test]
fn test_gas_injection_drsdt_0() {
    let mut sim = ReservoirSimulator::new(2, 2, 1, 0.3);
    sim.set_fim_enabled(true);
    sim.set_cell_dimensions(300.0, 300.0, 10.0);
    sim.set_three_phase_mode_enabled(true);
    sim.set_gas_redissolution_enabled(false);
    sim.set_injected_fluid("gas");
    sim.set_initial_pressure(331.0);
    sim.set_initial_saturation(0.12);
    sim.set_initial_gas_saturation(0.0);
    sim.set_initial_rs(226.0); // undersaturated
    
    // Some PVT to allow saturated evaluation
    use crate::pvt::{PvtRow, PvtTable};
    sim.pvt_table = Some(PvtTable::new(vec![
        PvtRow { p_bar: 100.0, rs_m3m3: 100.0, bo_m3m3: 1.1, mu_o_cp: 1.0, bg_m3m3: 0.1, mu_g_cp: 0.02 },
        PvtRow { p_bar: 277.0, rs_m3m3: 226.0, bo_m3m3: 1.2, mu_o_cp: 0.8, bg_m3m3: 0.02, mu_g_cp: 0.02 },
        PvtRow { p_bar: 400.0, rs_m3m3: 350.0, bo_m3m3: 1.3, mu_o_cp: 0.6, bg_m3m3: 0.01, mu_g_cp: 0.02 },
    ]));
    
    sim.add_well_with_id(0, 0, 0, 600.0, 0.1, 0.0, "INJ");
    sim.set_well_schedule("INJ", "rate", f64::NAN, 1e6, 600.0, true);
    
    sim.add_well_with_id(1, 1, 0, 100.0, 0.1, 0.0, "PROD");
    sim.set_well_schedule("PROD", "pressure", f64::NAN, f64::NAN, 100.0, true);
    
    println!("Before: Sg={} Rs={}", sim.sat_gas[0], sim.rs[0]);
    sim.step_fim(1.0);
    println!("After: Sg={} Rs={}", sim.sat_gas[0], sim.rs[0]);
}
