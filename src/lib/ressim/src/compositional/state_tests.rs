//! C7 state contract tests (`comp_state_*`).

use super::layout::CompositionalLayout;
use super::state::{
    CHECKPOINT_SCHEMA, Checkpoint, CompositionalCellState, CompositionalState, FlashCache,
    FlashCacheKey, RockView, StateError, StateVersion,
};
use crate::fluid::flash::flash;
use crate::fluid::pinned;

fn layout(n: usize, cells: usize) -> CompositionalLayout {
    CompositionalLayout::new(n, cells, 0, 0).unwrap()
}

fn cell(p: f64, z: &[f64]) -> CompositionalCellState {
    CompositionalCellState::new(p, z.to_vec()).unwrap()
}

fn ternary_state(cells: usize) -> (CompositionalLayout, CompositionalState) {
    let l = layout(3, cells);
    let s = CompositionalState::new(
        &l,
        (0..cells)
            .map(|i| cell(150.0 + i as f64, &[0.2, 0.5]))
            .collect(),
    )
    .unwrap();
    (l, s)
}

// ---------------------------------------------------------------------------------------------
// The dependent composition
// ---------------------------------------------------------------------------------------------

/// A cell stores `N-1` coordinates and derives the last, so `sum z = 1` is structural: there is no
/// second copy that an update could leave behind.
#[test]
fn comp_state_dependent_composition_is_derived_not_stored() {
    let c = cell(150.0, &[0.2, 0.5]);
    assert_eq!(c.independent_z(), &[0.2, 0.5]);
    assert!((c.dependent_z() - 0.3).abs() < 1e-15);

    let z = c.overall_composition();
    assert_eq!(z.len(), 3);
    assert!((z.iter().sum::<f64>() - 1.0).abs() < 1e-15);
}

/// Whatever an update does to the independent coordinates, the full composition still sums to one.
/// There is no path that can break it, which is the point of storing one fewer entry.
#[test]
fn comp_state_composition_sums_to_one_after_any_admissible_update() {
    let (_, s) = ternary_state(1);
    let mut trial = s.begin_trial();
    for (a, b) in [(0.05, 0.9), (0.45, 0.45), (0.0, 1.0), (0.999, 0.0005)] {
        trial.cell_mut(0).independent_z_mut()[0] = a;
        trial.cell_mut(0).independent_z_mut()[1] = b;
        let full = trial.cells()[0].overall_composition();
        assert!(
            (full.iter().sum::<f64>() - 1.0).abs() < 1e-14,
            "({a}, {b}) sums to {}",
            full.iter().sum::<f64>()
        );
    }
}

/// Independent coordinates summing above one would give a negative dependent component, which is
/// not a state. Rejected rather than clamped.
#[test]
fn comp_state_rejects_a_negative_dependent_component() {
    assert!(matches!(
        CompositionalCellState::new(150.0, vec![0.7, 0.5]),
        Err(StateError::DependentCompositionNegative { .. })
    ));
    // Exactly one is admissible: the dependent component is absent, not negative.
    assert!(CompositionalCellState::new(150.0, vec![0.7, 0.3]).is_ok());
}

#[test]
fn comp_state_rejects_non_physical_primaries() {
    assert!(matches!(
        CompositionalCellState::new(0.0, vec![0.2, 0.5]),
        Err(StateError::NonPositivePressure { .. })
    ));
    assert!(matches!(
        CompositionalCellState::new(-10.0, vec![0.2, 0.5]),
        Err(StateError::NonPositivePressure { .. })
    ));
    assert!(matches!(
        CompositionalCellState::new(f64::NAN, vec![0.2, 0.5]),
        Err(StateError::NonPhysicalPrimary { .. })
    ));
    assert!(matches!(
        CompositionalCellState::new(150.0, vec![-0.1, 0.5]),
        Err(StateError::NonPhysicalPrimary { .. })
    ));
    assert!(matches!(
        CompositionalCellState::new(150.0, vec![f64::INFINITY, 0.5]),
        Err(StateError::NonPhysicalPrimary { .. })
    ));
}

#[test]
fn comp_state_rejects_a_layout_mismatch() {
    let l = layout(3, 2);
    assert!(matches!(
        CompositionalState::new(&l, vec![cell(150.0, &[0.2, 0.5])]),
        Err(StateError::LayoutMismatch {
            what: "cell count",
            ..
        })
    ));
    assert!(matches!(
        CompositionalState::new(&l, vec![cell(150.0, &[0.2]), cell(150.0, &[0.2])]),
        Err(StateError::CompositionLength { .. })
    ));
}

// ---------------------------------------------------------------------------------------------
// Accepted versus trial
// ---------------------------------------------------------------------------------------------

/// The central contract: editing a trial cannot reach the accepted state. A rejected step changes
/// nothing because there is no path by which it could, not because something remembered to undo it.
#[test]
fn comp_state_a_rejected_trial_leaves_the_accepted_state_untouched() {
    let (_, accepted) = ternary_state(3);
    let before = accepted.clone();

    let mut trial = accepted.begin_trial();
    trial.cell_mut(0).pressure_bar = 999.0;
    trial.cell_mut(1).independent_z_mut()[0] = 0.9;
    // Drop the trial without committing, which is what a rejected step does.
    drop(trial);

    assert_eq!(accepted, before);
    assert_eq!(accepted.version(), StateVersion(0));
}

/// Committing produces a new accepted state with a strictly newer version.
#[test]
fn comp_state_commit_advances_the_version() {
    let (_, accepted) = ternary_state(2);
    assert_eq!(accepted.version(), StateVersion(0));

    let mut trial = accepted.begin_trial();
    assert_eq!(trial.base_version(), StateVersion(0));
    trial.cell_mut(0).pressure_bar = 160.0;

    let committed = trial.commit().unwrap();
    assert_eq!(committed.version(), StateVersion(1));
    assert_eq!(committed.cell(0).pressure_bar, 160.0);
    // And the original is still the original.
    assert_eq!(accepted.cell(0).pressure_bar, 150.0);

    let next = committed.begin_trial().commit().unwrap();
    assert_eq!(next.version(), StateVersion(2));
}

/// Validation happens at commit, not at each edit, so an update may pass through a transiently
/// inadmissible intermediate — but an inadmissible *final* state is refused.
#[test]
fn comp_state_commit_rejects_an_inadmissible_state() {
    let (_, accepted) = ternary_state(2);

    let mut trial = accepted.begin_trial();
    trial.cell_mut(1).independent_z_mut()[0] = 0.8;
    trial.cell_mut(1).independent_z_mut()[1] = 0.9; // sums above one
    assert!(matches!(
        trial.commit(),
        Err(StateError::DependentCompositionNegative { cell: 1, .. })
    ));

    // The accepted state is still intact: a failed commit consumes the trial, not the state.
    assert_eq!(accepted.version(), StateVersion(0));
    assert_eq!(accepted.cell(1).independent_z(), &[0.2, 0.5]);
}

/// A trial owns its cells. Two trials from the same accepted state must not share storage.
#[test]
fn comp_state_trials_are_independent_of_each_other() {
    let (_, accepted) = ternary_state(2);
    let mut a = accepted.begin_trial();
    let b = accepted.begin_trial();

    a.cell_mut(0).pressure_bar = 500.0;
    assert_eq!(b.cells()[0].pressure_bar, 150.0);
    assert_eq!(accepted.cell(0).pressure_bar, 150.0);
}

// ---------------------------------------------------------------------------------------------
// Checkpoints
// ---------------------------------------------------------------------------------------------

#[test]
fn comp_state_checkpoint_round_trips_through_serialization() {
    let (l, accepted) = ternary_state(4);
    let committed = accepted.begin_trial().commit().unwrap();

    let checkpoint = committed.checkpoint();
    let json = serde_json::to_string(&checkpoint).unwrap();
    let back: Checkpoint = serde_json::from_str(&json).unwrap();

    let restored = CompositionalState::from_checkpoint(&l, back).unwrap();
    assert_eq!(restored, committed);
    assert_eq!(restored.version(), committed.version());
}

/// A checkpoint from another schema is refused, not migrated. A restart that silently
/// reinterprets old bytes is worse than one that will not start.
#[test]
fn comp_state_checkpoint_rejects_an_incompatible_schema() {
    let (l, accepted) = ternary_state(2);
    let mut checkpoint = accepted.checkpoint();
    assert_eq!(checkpoint.schema, CHECKPOINT_SCHEMA);
    checkpoint.schema = CHECKPOINT_SCHEMA + 1;

    assert!(matches!(
        CompositionalState::from_checkpoint(&l, checkpoint),
        Err(StateError::IncompatibleCheckpoint { .. })
    ));
}

/// A checkpoint written for a different component count cannot be read into this layout, even
/// though its bytes are perfectly valid.
#[test]
fn comp_state_checkpoint_rejects_a_component_count_mismatch() {
    let (_, ternary) = ternary_state(2);
    let binary_layout = layout(2, 2);
    assert!(matches!(
        CompositionalState::from_checkpoint(&binary_layout, ternary.checkpoint()),
        Err(StateError::LayoutMismatch {
            what: "component count",
            ..
        })
    ));
}

// ---------------------------------------------------------------------------------------------
// The flash cache
// ---------------------------------------------------------------------------------------------

/// A hit requires the **exact** primaries. A state that differs in the last bit is a different
/// state, and returning a cached equilibrium for it would be returning an equilibrium for a state
/// nobody is at.
#[test]
fn comp_state_flash_cache_requires_bit_identical_primaries() {
    let (_, accepted) = ternary_state(1);
    let spec = pinned::ternary().unwrap();
    let c = accepted.cell(0);
    let key = FlashCacheKey::new(accepted.version(), c);

    let state = flash(
        &spec,
        c.pressure_bar * 1.0e5,
        spec.reservoir_temperature_k(),
        &c.overall_composition(),
        None,
    )
    .unwrap();

    let mut cache = FlashCache::with_cells(1);
    cache.insert(0, key.clone(), state);
    assert!(cache.get(0, &key).is_some());

    // One ulp of pressure is a different state.
    let nudged = CompositionalCellState::new(
        f64::from_bits(c.pressure_bar.to_bits() + 1),
        c.independent_z().to_vec(),
    )
    .unwrap();
    let nudged_key = FlashCacheKey::new(accepted.version(), &nudged);
    assert_ne!(nudged_key, key);
    assert!(cache.get(0, &nudged_key).is_none());
}

/// A committed step invalidates every cached entry, because the version is part of the key. No
/// walk over the cache is needed, and nothing can be missed.
#[test]
fn comp_state_flash_cache_is_invalidated_by_a_commit() {
    let (_, accepted) = ternary_state(2);
    let spec = pinned::ternary().unwrap();
    let mut cache = FlashCache::with_cells(2);

    for i in 0..2 {
        let c = accepted.cell(i);
        let state = flash(
            &spec,
            c.pressure_bar * 1.0e5,
            spec.reservoir_temperature_k(),
            &c.overall_composition(),
            None,
        )
        .unwrap();
        cache.insert(i, FlashCacheKey::new(accepted.version(), c), state);
    }

    // Commit without changing anything: the primaries are identical, the version is not.
    let committed = accepted.begin_trial().commit().unwrap();
    for i in 0..2 {
        let key = FlashCacheKey::new(committed.version(), committed.cell(i));
        assert!(
            cache.get(i, &key).is_none(),
            "cell {i} still hits after a commit, so the version is not in the key"
        );
    }
}

/// The cache changes no result. Dropping it entirely must give bit-identical answers, which is
/// what "a performance hint, not physical truth" means operationally.
#[test]
fn comp_state_flash_cache_changes_no_result() {
    let (_, accepted) = ternary_state(5);
    let spec = pinned::ternary().unwrap();
    let t = spec.reservoir_temperature_k();
    let mut cache = FlashCache::with_cells(5);

    let mut with_cache = Vec::new();
    for pass in 0..2 {
        for i in 0..5 {
            let c = accepted.cell(i);
            let key = FlashCacheKey::new(accepted.version(), c);
            let state = match cache.get(i, &key) {
                Some(hit) => hit.clone(),
                None => {
                    let computed = flash(
                        &spec,
                        c.pressure_bar * 1.0e5,
                        t,
                        &c.overall_composition(),
                        None,
                    )
                    .unwrap();
                    cache.insert(i, key, computed.clone());
                    computed
                }
            };
            if pass == 1 {
                with_cache.push(state);
            }
        }
    }
    assert_eq!(
        cache.hits(),
        5,
        "the second pass should have hit every cell"
    );

    cache.clear();
    for (i, cached) in with_cache.iter().enumerate() {
        let c = accepted.cell(i);
        let fresh = flash(
            &spec,
            c.pressure_bar * 1.0e5,
            t,
            &c.overall_composition(),
            None,
        )
        .unwrap();
        assert_eq!(&fresh, cached, "cell {i}: the cache changed the answer");
    }
}

// ---------------------------------------------------------------------------------------------
// The geometry boundary
// ---------------------------------------------------------------------------------------------

/// `RockView` is the whole of the geometry surface. It borrows, it is read-only, and nothing
/// reachable from it can step a timestep or touch a well — which is the plan's requirement that
/// EOS and accumulation code not be handed a `ReservoirSimulator` to reach a porosity.
#[test]
fn comp_state_rock_view_reproduces_the_existing_pore_volume_relation() {
    let pv_ref = [100.0, 200.0, 50.0];
    let rock = RockView {
        pore_volume_ref_m3: &pv_ref,
        reference_pressure_bar: 200.0,
        compressibility_per_bar: 4.0e-5,
    };
    assert_eq!(rock.cell_count(), 3);

    // At the reference pressure the pore volume is exactly the reference value.
    for i in 0..3 {
        assert_eq!(rock.pore_volume_m3(i, 200.0), pv_ref[i]);
    }

    // pv(p) = pv_ref * exp(c * (p - p_ref)), the same relation as fim/properties.rs.
    let expected = 100.0 * (4.0e-5f64 * (250.0 - 200.0)).exp();
    assert!((rock.pore_volume_m3(0, 250.0) - expected).abs() < 1e-12);

    // Compaction below the reference pressure, dilation above it.
    assert!(rock.pore_volume_m3(1, 150.0) < pv_ref[1]);
    assert!(rock.pore_volume_m3(1, 250.0) > pv_ref[1]);
}

/// A state is usable by the thermodynamics without anything in between: the overall composition it
/// produces flashes directly. That is the boundary working.
#[test]
fn comp_state_flashes_directly_through_the_fluid_module() {
    let (_, s) = ternary_state(4);
    let spec = pinned::ternary().unwrap();
    for i in 0..4 {
        let c = s.cell(i);
        let result = flash(
            &spec,
            c.pressure_bar * 1.0e5,
            spec.reservoir_temperature_k(),
            &c.overall_composition(),
            None,
        );
        assert!(result.is_ok(), "cell {i}: {:?}", result.err());
    }
}

/// Both production component counts carry a state, and a binary cell stores exactly one
/// independent coordinate.
#[test]
fn comp_state_supports_both_production_component_counts() {
    let binary = CompositionalState::new(&layout(2, 2), vec![cell(150.0, &[0.6]); 2]).unwrap();
    assert_eq!(binary.component_count(), 2);
    assert_eq!(binary.cell(0).independent_z().len(), 1);
    assert!((binary.cell(0).dependent_z() - 0.4).abs() < 1e-15);

    let (_, ternary) = ternary_state(2);
    assert_eq!(ternary.component_count(), 3);
    assert_eq!(ternary.cell(0).independent_z().len(), 2);
}
