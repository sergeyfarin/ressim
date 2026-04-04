use crate::ReservoirSimulator;
use crate::pvt::{PvtRow, PvtTable};

#[test]
fn physics_wells_sources_gas_injection_surface_totals_match_target_on_both_solvers() {
    fn run_case(fim_enabled: bool) -> f64 {
        let mut sim = ReservoirSimulator::new(1, 1, 1, 0.2);
        sim.set_fim_enabled(fim_enabled);
        sim.set_three_phase_rel_perm_props(
            0.10, 0.10, 0.05, 0.05, 0.10, 2.0, 2.0, 1.5, 0.8, 0.9, 0.7,
        )
        .unwrap();
        sim.set_three_phase_mode_enabled(true);
        sim.set_injected_fluid("gas").unwrap();
        sim.set_gas_fluid_properties(0.02, 1e-4, 10.0).unwrap();
        sim.set_initial_pressure(100.0);
        sim.set_initial_saturation(0.10);
        sim.pvt_table = Some(PvtTable::new(
            vec![PvtRow {
                p_bar: 100.0,
                rs_m3m3: 0.0,
                bo_m3m3: 1.2,
                mu_o_cp: 1.0,
                bg_m3m3: 0.25,
                mu_g_cp: 0.02,
            }],
            sim.pvt.c_o,
        ));
        sim.set_well_control_modes("rate".to_string(), "bhp".to_string());
        sim.set_target_well_surface_rates(120.0, 0.0).unwrap();
        sim.set_well_bhp_limits(0.0, 1.0e9).unwrap();
        sim.add_well(0, 0, 0, 100.0, 0.1, 0.0, true).unwrap();

        sim.step(0.1);
        assert!(
            sim.last_solver_warning.is_empty(),
            "surface-rate injector case emitted solver warning for fim_enabled={}: {}",
            fim_enabled,
            sim.last_solver_warning
        );

        sim.rate_history
            .last()
            .expect("rate history should have an entry")
            .total_injection
    }

    for fim_enabled in [false, true] {
        let total_injection = run_case(fim_enabled);
        let rel_diff = ((total_injection - 120.0) / 120.0).abs();
        assert!(
            rel_diff <= 0.10,
            "solver should keep gas injector surface total near target: fim_enabled={} total={} rel_diff={}",
            fim_enabled,
            total_injection,
            rel_diff
        );
    }
}
