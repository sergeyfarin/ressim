use super::*;

#[test]
fn per_layer_dz_affects_pore_volume_and_depth() {
    let mut sim = ReservoirSimulator::new(2, 2, 3, 0.25);
    sim.set_cell_dimensions_per_layer(100.0, 100.0, vec![6.0, 9.0, 15.0])
        .unwrap();

    let id_k0 = sim.idx(0, 0, 0);
    let id_k1 = sim.idx(0, 0, 1);
    let id_k2 = sim.idx(0, 0, 2);

    let pv0 = sim.pore_volume_m3(id_k0);
    let pv1 = sim.pore_volume_m3(id_k1);
    let pv2 = sim.pore_volume_m3(id_k2);

    assert!((pv0 - 100.0 * 100.0 * 6.0 * 0.25).abs() < 1e-10);
    assert!((pv1 - 100.0 * 100.0 * 9.0 * 0.25).abs() < 1e-10);
    assert!((pv2 - 100.0 * 100.0 * 15.0 * 0.25).abs() < 1e-10);

    let d0 = sim.depth_at_k(0);
    let d1 = sim.depth_at_k(1);
    let d2 = sim.depth_at_k(2);

    assert!(
        (d0 - 3.0).abs() < 1e-10,
        "k=0: depth should be 3, got {}",
        d0
    );
    assert!(
        (d1 - 10.5).abs() < 1e-10,
        "k=1: depth should be 10.5, got {}",
        d1
    );
    assert!(
        (d2 - 22.5).abs() < 1e-10,
        "k=2: depth should be 22.5, got {}",
        d2
    );
}

#[test]
fn per_layer_dz_validation_rejects_invalid_inputs() {
    let mut sim = ReservoirSimulator::new(2, 2, 3, 0.2);

    err_contains(
        sim.set_cell_dimensions_per_layer(10.0, 10.0, vec![1.0, 2.0]),
        "length equal to nz",
    );
    err_contains(
        sim.set_cell_dimensions_per_layer(10.0, 10.0, vec![1.0, 0.0, 3.0]),
        "positive and finite",
    );
    err_contains(
        sim.set_cell_dimensions_per_layer(-1.0, 10.0, vec![1.0, 2.0, 3.0]),
        "positive",
    );
}

#[test]
fn non_uniform_dz_transmissibility_z_direction() {
    let mut sim = ReservoirSimulator::new(1, 1, 2, 0.2);
    sim.set_cell_dimensions_per_layer(10.0, 10.0, vec![6.0, 15.0])
        .unwrap();
    sim.set_permeability_random_seeded(100.0, 100.0, 42)
        .unwrap();

    let id0 = sim.idx(0, 0, 0);
    let id1 = sim.idx(0, 0, 1);

    let t_z = sim.geometric_transmissibility(id0, id1, 'z');
    let kz0 = sim.perm_z[id0];
    let kz1 = sim.perm_z[id1];
    let k_h = 2.0 * kz0 * kz1 / (kz0 + kz1);
    let expected = k_h * 100.0 / 10.5;

    assert!((t_z - expected).abs() / expected < 1e-9);
}

#[test]
fn average_reservoir_pressure_is_pv_weighted() {
    let mut sim = ReservoirSimulator::new(1, 1, 2, 0.25);
    sim.set_cell_dimensions_per_layer(10.0, 10.0, vec![1.0, 9.0])
        .unwrap();

    let id0 = sim.idx(0, 0, 0);
    let id1 = sim.idx(0, 0, 1);
    sim.pressure[id0] = 100.0;
    sim.pressure[id1] = 200.0;

    let pv0 = sim.pore_volume_m3(id0);
    let pv1 = sim.pore_volume_m3(id1);
    let expected = (100.0 * pv0 + 200.0 * pv1) / (pv0 + pv1);

    assert!((sim.average_reservoir_pressure_pv_weighted() - expected).abs() < 1e-12);
    assert!((sim.average_reservoir_pressure_pv_weighted() - 150.0).abs() > 1e-6);
}
