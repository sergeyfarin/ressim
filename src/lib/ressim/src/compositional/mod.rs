//! Compositional reservoir state, layout and — from C8 on — physics.
//!
//! Where [`crate::fluid`] owns thermodynamics and no simulation state, this module owns the state
//! and the discretization: which unknown lives in which matrix column, what a cell holds, what an
//! accepted state is as opposed to a Newton trial, and what may be cached.
//!
//! The split matters because it is what let C1–C6 validate the thermodynamics against an external
//! oracle in isolation. Nothing here reaches back into `fluid` except to call it.
//!
//! Status: C7–C10 complete — layout, state, accumulation, face flux, global assembly, the
//! Newton solve and the timestep lifecycle. C9's gravity subtask and C11's wells are not done,
//! and C10's iterative linear adapter is deferred behind the direct solve it would be measured
//! against.

// Consumed by this module's tests and, from C8 on, by the accumulation and assembly code. The
// crate has no non-test caller yet; scoped here and removable as soon as C8 lands.
#![allow(dead_code)]

pub mod accumulation;
pub mod assembly;
pub mod flux;
pub mod layout;
pub mod newton;
pub mod state;
pub mod timestep;

#[cfg(test)]
mod accumulation_tests;
#[cfg(test)]
mod assembly_tests;
#[cfg(test)]
mod flux_tests;
#[cfg(test)]
mod layout_tests;
#[cfg(test)]
mod newton_tests;
#[cfg(test)]
mod state_tests;
#[cfg(test)]
mod timestep_tests;
