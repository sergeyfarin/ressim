//! #51: sourced gas-condensate thermodynamics, translations and staged surface conversion.
use super::{
    derivatives::flash_derivatives,
    flash::{PhaseState, flash},
    pinned,
    specification::FluidSpecification,
    transport::surface_separation,
    units::GAS_CONSTANT_J_PER_MOL_K,
};
use serde::Deserialize;

#[derive(Deserialize)]
pub(crate) struct Reference {
    pub schema: String,
    pub gas_constant: f64,
    pub states: Vec<State>,
    pub separators: Vec<Separator>,
    pub depletion: Depletion,
    pub cvd: Vec<CvdState>,
}
#[derive(Deserialize)]
pub(crate) struct CvdState {
    pub p: f64,
    pub z: Vec<f64>,
    pub moles: f64,
    pub sl: f64,
    pub removed: f64,
}
#[derive(Deserialize)]
pub(crate) struct State {
    pub p: f64,
    #[serde(rename = "T")]
    pub t: f64,
    pub z: Vec<f64>,
    pub beta: f64,
    pub x: Vec<f64>,
    pub y: Vec<f64>,
    pub vl: f64,
    pub vv: f64,
}
#[derive(Deserialize)]
pub(crate) struct Separator {
    pub primary_psia: f64,
    pub liquid: Vec<f64>,
    pub gas: Vec<f64>,
    pub liquid_volume: f64,
    pub gas_volume: f64,
}
#[derive(Deserialize)]
pub(crate) struct Depletion {
    pub reports: Vec<Report>,
}
#[derive(Deserialize)]
pub(crate) struct Report {
    pub day: usize,
    pub p: Vec<f64>,
    pub sl: Vec<f64>,
    pub z: Vec<Vec<f64>>,
}
pub(crate) fn reference() -> Reference {
    let r: Reference = serde_json::from_str(include_str!(
        "../../../../../opm/compositional/spe3/reference.json"
    ))
    .unwrap();
    assert_eq!(r.schema, "ressim-spe3-reference/1");
    r
}

#[test]
fn comp_spe3_flash_matches_independent_jutul_on_the_200f_isotherm() {
    let r = reference();
    let spec = pinned::spe3().unwrap();
    let scale = GAS_CONSTANT_J_PER_MOL_K / r.gas_constant;
    let mut phases = [0; 2];
    let mut worst = 0.0_f64;
    for s in r.states {
        let f = flash(&spec, s.p, s.t, &s.z, None).unwrap();
        assert!(
            (f.beta - s.beta).abs() < 1e-8,
            "{}: {} != {}",
            s.p,
            f.beta,
            s.beta
        );
        phases[usize::from(f.phase_state == PhaseState::TwoPhase)] += 1;
        for i in 0..9 {
            if f.phase_state == PhaseState::TwoPhase {
                assert!((f.x[i] - s.x[i]).abs() < 1e-8);
                assert!((f.y[i] - s.y[i]).abs() < 1e-8);
            }
            assert!(((1.0 - f.beta) * f.x[i] + f.beta * f.y[i] - s.z[i]).abs() < 1e-10);
        }
        for (ours, oracle) in [(f.liquid, s.vl), (f.vapour, s.vv)] {
            if let Some(p) = ours {
                worst = worst.max((p.molar_volume / (oracle * scale) - 1.0).abs());
            }
        }
    }
    assert!(
        phases[0] >= 2 && phases[1] >= 6,
        "missing the dew-point crossing: {phases:?}"
    );
    assert!(worst < 1e-8, "translated molar volume error {worst:e}");
    println!("SPE3 flash: worst relative volume error {worst:e}, phase census {phases:?}");
}

#[test]
fn comp_spe3_separator_train_matches_independent_flash_and_conserves_every_component() {
    let r = reference();
    let scale = GAS_CONSTANT_J_PER_MOL_K / r.gas_constant;
    for s in r.separators {
        let spec = pinned::spe3()
            .unwrap()
            .with_separator_stages(pinned::spe3_separator_stages(s.primary_psia == 315.0))
            .unwrap();
        let stream = surface_separation(&spec, &pinned::SPE3_COMPOSITION).unwrap();
        for i in 0..9 {
            assert!((stream.liquid_component_moles[i] - s.liquid[i]).abs() < 1e-9);
            assert!((stream.vapour_component_moles[i] - s.gas[i]).abs() < 1e-9);
            assert!(
                (stream.liquid_component_moles[i] + stream.vapour_component_moles[i]
                    - pinned::SPE3_COMPOSITION[i])
                    .abs()
                    < 1e-10
            );
        }
        assert!((stream.liquid_volume / (s.liquid_volume * scale) - 1.0).abs() < 1e-8);
        assert!((stream.vapour_volume / (s.gas_volume * scale) - 1.0).abs() < 1e-8);
        let doubled: Vec<_> = pinned::SPE3_COMPOSITION.iter().map(|x| 2.0 * x).collect();
        assert_eq!(
            surface_separation(&spec, &doubled).unwrap().liquid_volume,
            2.0 * stream.liquid_volume
        );
        assert_eq!(
            surface_separation(&spec, &[0.0; 9]).unwrap().total_moles,
            0.0
        );
    }
}

#[test]
fn comp_spe3_translated_density_derivatives_match_finite_differences() {
    let spec = pinned::spe3().unwrap();
    for pressure in [260e5, 220e5] {
        let z = pinned::SPE3_COMPOSITION;
        let t = spec.reservoir_temperature_k();
        let f = flash(&spec, pressure, t, &z, None).unwrap();
        let d = flash_derivatives(&spec, pressure, t, &z, &f).unwrap();
        for col in 0..9 {
            let h = if col == 0 { 100.0 } else { 1e-6 };
            let mut zp = z;
            let mut zm = z;
            let (mut pp, mut pm) = (pressure, pressure);
            if col == 0 {
                pp += h;
                pm -= h;
            } else {
                zp[col - 1] += h;
                zp[8] -= h;
                zm[col - 1] -= h;
                zm[8] += h;
            }
            let fp = flash(&spec, pp, t, &zp, None).unwrap();
            let fm = flash(&spec, pm, t, &zm, None).unwrap();
            for (plus, minus, ad) in [
                (&fp.liquid, &fm.liquid, &d.dliquid_molar_density),
                (&fp.vapour, &fm.vapour, &d.dvapour_molar_density),
            ] {
                if let (Some(p), Some(m)) = (plus, minus) {
                    let fd = (p.molar_density - m.molar_density) / (2.0 * h);
                    assert!(
                        (fd - ad[col]).abs() / fd.abs().max(1e-6) < 1e-5,
                        "p {pressure}, col {col}: FD {fd:e}, AD {:e}",
                        ad[col]
                    );
                }
            }
        }
    }
}

#[test]
fn comp_spe3_extensions_validate_and_survive_serde_and_permutation() {
    let spec = pinned::spe3().unwrap();
    assert!(spec.clone().with_volume_shifts(vec![0.0; 8]).is_err());
    assert!(spec.clone().with_volume_shifts(vec![f64::NAN; 9]).is_err());
    assert!(spec.clone().with_separator_stages(vec![]).is_err());
    let mut stages = spec.separator_stages().to_vec();
    stages.swap(0, 1);
    assert!(spec.clone().with_separator_stages(stages).is_err());
    let copy: FluidSpecification =
        serde_json::from_str(&serde_json::to_string(&spec).unwrap()).unwrap();
    assert_eq!(copy, spec);
    let permuted = spec.permuted(&[8, 7, 6, 5, 4, 3, 2, 1, 0]).unwrap();
    assert_eq!(permuted.separator_stages(), spec.separator_stages());
    let z: Vec<_> = pinned::SPE3_COMPOSITION.iter().rev().copied().collect();
    let s = surface_separation(&spec, &pinned::SPE3_COMPOSITION).unwrap();
    let p = surface_separation(&permuted, &z).unwrap();
    assert!((s.liquid_volume / p.liquid_volume - 1.0).abs() < 1e-10);
}

#[test]
fn comp_spe3_separator_handles_absent_products_and_rejects_invalid_streams() {
    let spec = pinned::spe3().unwrap();
    let mut methane = [0.0; 9];
    methane[2] = 1.0;
    let gas = surface_separation(&spec, &methane).unwrap();
    assert_eq!(gas.phase_state, PhaseState::SingleVapour);
    assert_eq!(gas.liquid_moles, 0.0);
    assert_eq!(gas.liquid_volume, 0.0);
    assert!((gas.vapour_moles - 1.0).abs() < 1e-15);
    let mut heavy = [0.0; 9];
    heavy[8] = 1.0;
    let liquid = surface_separation(&spec, &heavy).unwrap();
    assert_eq!(liquid.phase_state, PhaseState::SingleLiquid);
    assert_eq!(liquid.vapour_moles, 0.0);
    assert_eq!(liquid.vapour_volume, 0.0);
    assert!(surface_separation(&spec, &[f64::MAX; 9]).is_err());
    assert!(surface_separation(&spec, &[f64::NAN; 9]).is_err());
    assert!(surface_separation(&spec, &[-1.0; 9]).is_err());
    assert!(surface_separation(&spec, &[0.0; 8]).is_err());
}

#[test]
fn comp_spe3_constant_volume_depletion_matches_jutul_flash_and_closes_volume() {
    let spec = pinned::spe3().unwrap();
    let t = spec.reservoir_temperature_k();
    let mut z = pinned::SPE3_COMPOSITION.to_vec();
    let mut n = 1.0;
    let cell_volume = flash(&spec, 260e5, t, &z, None)
        .unwrap()
        .mixture_molar_volume();
    let mut produced = [0.0; 9];
    let r = reference();
    assert_eq!(r.cvd.len(), 13);
    for s in r.cvd {
        let f = flash(&spec, s.p, t, &z, None).unwrap();
        let vl = f.liquid.as_ref().map_or(0.0, |p| p.molar_volume);
        let vv = f.vapour.as_ref().unwrap().molar_volume;
        let nl = n * f.l_liquid();
        let nv = n * f.beta;
        let removed = ((nl * vl + nv * vv - cell_volume) / vv).max(0.0);
        assert!(removed <= nv + 1e-12);
        let remaining: Vec<_> = (0..9)
            .map(|i| nl * f.x[i] + (nv - removed) * f.y[i])
            .collect();
        n = remaining.iter().sum();
        z = remaining.iter().map(|m| m / n).collect();
        for i in 0..9 {
            produced[i] += removed * f.y[i];
            assert!((remaining[i] + produced[i] - pinned::SPE3_COMPOSITION[i]).abs() < 1e-10);
            assert!((z[i] - s.z[i]).abs() < 1e-8);
        }
        assert!((n - s.moles).abs() < 1e-8);
        assert!((removed - s.removed).abs() < 1e-8);
        assert!((nl * vl / cell_volume - s.sl).abs() < 1e-8);
        // Reflash the retained material: its phases must fill the same fixed cell volume.
        let retained = flash(&spec, s.p, t, &z, None).unwrap();
        assert!((n * retained.mixture_molar_volume() / cell_volume - 1.0).abs() < 1e-8);
    }
}

#[test]
fn comp_spe3_c0_admission_matches_opm_ptflash() {
    let oracle: super::fixture::FixtureSystem = serde_json::from_str(include_str!(
        "../../../../../opm/compositional/spe3/ptflash.json"
    ))
    .unwrap();
    let spec = pinned::spe3().unwrap();
    assert_eq!(oracle.num_components, 9);
    assert_eq!(oracle.states.len(), 12);
    for (i, c) in oracle.components.iter().enumerate() {
        let ours = spec.component(i);
        assert_eq!(ours.id, c.name);
        assert_eq!(ours.critical_temperature_k, c.critical_temperature_k);
        assert_eq!(ours.critical_pressure_pa, c.critical_pressure_pa);
        assert!((ours.molar_mass_kg_per_mol - c.molar_mass_kg_per_mol).abs() < 1e-15);
        assert_eq!(ours.acentric_factor, c.acentric_factor);
        for j in 0..9 {
            assert_eq!(spec.interaction(i, j), oracle.binary_interaction[i][j]);
        }
    }
    let scale = GAS_CONSTANT_J_PER_MOL_K / super::units::OPM_GAS_CONSTANT_J_PER_MOL_K;
    for s in oracle.states {
        assert_eq!(s.status, "ok", "{}: oracle failed", s.id);
        let f = flash(&spec, s.pressure_pa, s.temperature_k, &s.z, None).unwrap();
        assert!((f.beta - s.beta_vapour.unwrap()).abs() < 1e-8, "{}", s.id);
        for (ours, theirs, composition) in
            [(&f.liquid, &s.liquid, &f.x), (&f.vapour, &s.vapour, &f.y)]
        {
            if let Some(p) = ours {
                let raw_volume =
                    p.molar_volume + super::eos::volume_translation(&spec, composition);
                assert!(
                    (raw_volume / (theirs.as_ref().unwrap().molar_volume * scale) - 1.0).abs()
                        < 1e-8
                );
            }
        }
        if s.is_two_phase() {
            for i in 0..9 {
                assert!((f.x[i] - s.x_liquid.as_ref().unwrap()[i]).abs() < 1e-8);
                assert!((f.y[i] - s.y_vapour.as_ref().unwrap()[i]).abs() < 1e-8);
            }
        }
    }
}
