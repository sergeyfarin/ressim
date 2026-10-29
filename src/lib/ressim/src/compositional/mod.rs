//! Compositional reservoir state, layout and — from C8 on — physics.
//!
//! Where [`crate::fluid`] owns thermodynamics and no simulation state, this module owns the state
//! and the discretization: which unknown lives in which matrix column, what a cell holds, what an
//! accepted state is as opposed to a Newton trial, and what may be cached.
//!
//! The split matters because it is what let C1–C6 validate the thermodynamics against an external
//! oracle in isolation. Nothing here reaches back into `fluid` except to call it.
//!
//! Status: C7–C12 complete — layout, state, accumulation, face flux and gravity, global assembly,
//! wells, the Newton solve, the timestep lifecycle, and validation against an independent
//! simulator. `NATIVE-COMPOSITIONAL-READY` is declared; see `docs/COMPOSITIONAL_VALIDATION.md`.
//! The Jacobian is block-sparse and solves by sparse LU or FGMRES with a compositional CPR, both
//! measured against the dense direct solve they replaced (#49; [`linear`]).
//!
//! [`water`] is C15 / V1b (#46): an optional immiscible water phase with its own unknown and
//! balance, Stone II three-phase relative permeability, validated against JutulDarcy. A case
//! without water runs none of it.
//!
//! [`api`] is C13's engine-side boundary — a configured case and the payloads that cross the WASM
//! edge — and [`frontend`] is the thin `wasm_bindgen` shell over it.

// Much of this module is reached only through `api`, `frontend` and the test suites; the physics
// layers below them expose more than any single caller uses, which is what makes them testable in
// isolation.
#![allow(dead_code)]

pub mod accumulation;
pub mod api;
pub mod assembly;
pub mod block_matrix;
pub mod flux;
#[cfg(feature = "wasm")]
pub mod frontend;
pub mod layout;
pub mod linear;
pub mod newton;
pub mod relperm;
pub mod state;
pub mod timestep;
pub mod water;
pub mod wells;

#[cfg(test)]
mod accumulation_tests;
#[cfg(test)]
mod api_tests;
#[cfg(test)]
mod assembly_tests;
#[cfg(test)]
mod cost_tests;
#[cfg(test)]
mod flux_tests;
#[cfg(test)]
mod gravity_tests;
#[cfg(test)]
mod layout_tests;
#[cfg(test)]
mod linear_tests;
#[cfg(test)]
mod newton_tests;
#[cfg(test)]
mod reference_tests;
#[cfg(test)]
mod relperm_tests;
#[cfg(test)]
mod spe3_tests;
#[cfg(test)]
mod state_tests;
#[cfg(test)]
mod timestep_tests;
#[cfg(test)]
mod water_tests;
#[cfg(test)]
mod wells_tests;
