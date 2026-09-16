//! Compositional fluid thermodynamics.
//!
//! This module is the EOS side of the compositional model described in
//! `docs/COMPOSITIONAL_FLUID_EXECUTION_PLAN_2026-09-14.md`. It owns component data, unit
//! conversion, and — as later tasks land — the Peng–Robinson EOS, phase stability, flash and
//! their derivatives.
//!
//! **It owns no simulation state.** Nothing here takes a `ReservoirSimulator`, reads a grid or
//! touches a well. That separation is what lets the thermodynamics be validated against an
//! external oracle on its own, which is the whole premise of the plan's C1–C6 sequence; a
//! function that needed the simulator to evaluate a fugacity could not be checked against
//! `opm/compositional/ptflash_fixtures.json`.
//!
//! Units inside this module are SI — Pa, K, mol, m³, kg/mol — without exception. The reservoir's
//! oil-field units stop at [`units`].
//!
//! Status: C1–C5 complete — specification and units, the PR EOS, phase stability, the PT
//! flash, equilibrium derivatives, LBC viscosity and the surface separation. C6 (the standalone
//! thermodynamic admission gate) and everything from C7 on are not yet started.

// The specification and unit API is consumed by this module's own tests and, from C2 onward, by
// the EOS. Until then the crate has no non-test caller, and without this the build gains ~30
// dead-code warnings that would bury a real one. Scoped to `fluid` on purpose, and removable as
// soon as C2 lands: `eos.rs` uses the components, the interaction matrix, the gas constant and
// every unit conversion here.
#![allow(dead_code)]

pub mod derivatives;
pub mod eos;
#[cfg(test)]
pub(crate) mod fixture;
pub mod flash;
pub mod pinned;
pub mod specification;
pub mod stability;
pub mod transport;
pub(crate) mod units;

#[cfg(test)]
mod derivative_tests;
#[cfg(test)]
mod eos_tests;
#[cfg(test)]
mod flash_tests;
#[cfg(test)]
mod stability_tests;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod transport_tests;
