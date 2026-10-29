//! Immiscible water and three-phase relative permeability (compositional plan, C15 / V1b; #46).
//!
//! # The model
//!
//! Water is a conserved component carried by a phase of its own, **outside the EOS**. No
//! hydrocarbon dissolves in it and none of it vaporizes: the flash sees only the hydrocarbon
//! components, and water moves only through the water phase. That exclusion is the scope of V1b
//! rather than a simplification made here; aqueous CO2 and brine thermodynamics are not delivered.
//!
//! A cell with water carries one more primary, `Sw`, and one more equation, the water mole
//! balance. The hydrocarbon phases share the remaining pore volume:
//!
//! ```text
//! S_L + S_V = 1 - Sw         S_L = (1 - Sw) s_L,   S_V = (1 - Sw) s_V
//! ```
//!
//! where `s_L`, `s_V` are the flash's hydrocarbon-only volume fractions. The flash's `beta` stays
//! a mole split **within** the hydrocarbons.
//!
//! # Water properties: PVTW
//!
//! The ECLIPSE `PVTW` law, which OPM Flow and JutulDarcy (`ConstMuBTable`) both implement, so an
//! oracle runs the same water as this engine:
//!
//! ```text
//! X     = c_w (p - p_ref)            b_w(p) = (1 + X + X²/2) / B_w,ref
//! Y     = -c_mu (p - p_ref)          mu_w(p) = mu_ref / (1 + Y + Y²/2)
//! rho_w = rho_w,surface * b_w        c_w,molar = rho_w / M_H2O
//! ```
//!
//! Water is counted in **moles** so its balance is a mole balance like every other row, with the
//! molar mass of H2O as the conversion. The choice of unit cancels in the row scaling, and a
//! surface volume is recovered exactly as `moles * M_H2O / rho_w,surface`.
//!
//! # Three-phase relative permeability: Stone II
//!
//! Over SWOF/SGOF tables, through [`crate::relperm::RockFluidPropsThreePhase`] — the same
//! functions the black-oil engine runs, not a second implementation of them. The liquid
//! hydrocarbon phase is the oil and the vapour is the gas:
//!
//! ```text
//! kr_w = krw(Sw)          kr_V = krg(S_V)          kr_L = Stone II(Sw, S_V)
//! ```
//!
//! **Relperm is required and has no default**, as for the two-phase models: a case with water must
//! supply its tables. Capillary pressure stays zero — a table with a nonzero `pcow` or `pcog`
//! column is rejected rather than having the column ignored, because unequal phase pressures need
//! a phase-pressure convention the plan requires be written down before capillarity is enabled.
//!
//! # The zero-hydrocarbon limit
//!
//! At `Sw = 1` the hydrocarbon composition multiplies a zero pore volume and is unidentifiable:
//! every hydrocarbon column of the Jacobian vanishes. The plan allows a sourced primary switch or
//! rejecting the domain, and this rejects it: an accepted state must keep `Sw` strictly below one
//! ([`StateError`](super::state::StateError) names the cell), and the Newton step bound keeps a
//! trial off the boundary. A waterflood stops short of it physically — gas or oil is trapped at
//! its residual, where its relative permeability is zero.

use crate::ad::Scalar;
use crate::relperm::{RockFluidPropsThreePhase, SgofRow, SwofRow, ThreePhaseScalTables};

/// Molar mass of H2O [kg/mol] (IUPAC standard atomic weights: 2 × 1.00794 + 15.9994 g/mol).
pub const WATER_MOLAR_MASS_KG_PER_MOL: f64 = 18.015_28e-3;

/// Water properties, the `PVTW` record plus a surface density.
///
/// Construct with [`WaterProperties::new`]; there is no default, for the same reason the relative
/// permeability has none.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WaterProperties {
    reference_pressure_bar: f64,
    formation_volume_factor: f64,
    compressibility_per_bar: f64,
    viscosity_cp: f64,
    viscosibility_per_bar: f64,
    surface_density_kg_per_m3: f64,
}

/// Why a water model could not be constructed.
#[derive(Clone, Debug, PartialEq)]
pub enum WaterError {
    /// A parameter is non-finite or outside its permitted range.
    InvalidParameter {
        field: &'static str,
        value: f64,
        reason: &'static str,
    },
    /// A SWOF/SGOF table failed validation.
    Table(String),
    /// A table carries a nonzero capillary pressure, which V1b does not model.
    CapillaryPressureNotSupported {
        table: &'static str,
        row: usize,
        value: f64,
    },
}

impl core::fmt::Display for WaterError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::InvalidParameter {
                field,
                value,
                reason,
            } => write!(f, "{field} = {value} is invalid: {reason}"),
            Self::Table(reason) => write!(f, "{reason}"),
            Self::CapillaryPressureNotSupported { table, row, value } => write!(
                f,
                "{table} row {row} has capillary pressure {value} bar; capillary pressure is zero \
                 in this model and a nonzero column is rejected rather than ignored"
            ),
        }
    }
}

impl WaterProperties {
    /// Validate and construct from a `PVTW` record and a surface density.
    pub fn new(
        reference_pressure_bar: f64,
        formation_volume_factor: f64,
        compressibility_per_bar: f64,
        viscosity_cp: f64,
        viscosibility_per_bar: f64,
        surface_density_kg_per_m3: f64,
    ) -> Result<Self, WaterError> {
        let positive = |field: &'static str, value: f64| {
            if value.is_finite() && value > 0.0 {
                Ok(())
            } else {
                Err(WaterError::InvalidParameter {
                    field,
                    value,
                    reason: "must be finite and positive",
                })
            }
        };
        let finite = |field: &'static str, value: f64| {
            if value.is_finite() {
                Ok(())
            } else {
                Err(WaterError::InvalidParameter {
                    field,
                    value,
                    reason: "must be finite",
                })
            }
        };
        positive("reference_pressure_bar", reference_pressure_bar)?;
        positive("formation_volume_factor", formation_volume_factor)?;
        finite("compressibility_per_bar", compressibility_per_bar)?;
        positive("viscosity_cp", viscosity_cp)?;
        finite("viscosibility_per_bar", viscosibility_per_bar)?;
        positive("surface_density_kg_per_m3", surface_density_kg_per_m3)?;
        if compressibility_per_bar < 0.0 {
            return Err(WaterError::InvalidParameter {
                field: "compressibility_per_bar",
                value: compressibility_per_bar,
                reason: "must be non-negative",
            });
        }
        Ok(Self {
            reference_pressure_bar,
            formation_volume_factor,
            compressibility_per_bar,
            viscosity_cp,
            viscosibility_per_bar,
            surface_density_kg_per_m3,
        })
    }

    pub fn surface_density_kg_per_m3(&self) -> f64 {
        self.surface_density_kg_per_m3
    }

    /// Reciprocal formation volume factor `b_w = 1/B_w` at pressure `p` [bar].
    pub fn reciprocal_fvf<S: Scalar>(&self, pressure_bar: S) -> S {
        let x = (pressure_bar - self.reference_pressure_bar) * self.compressibility_per_bar;
        (x * x * 0.5 + x + 1.0) / self.formation_volume_factor
    }

    /// Reservoir mass density [kg/m³].
    pub fn mass_density<S: Scalar>(&self, pressure_bar: S) -> S {
        self.reciprocal_fvf(pressure_bar) * self.surface_density_kg_per_m3
    }

    /// Reservoir molar density [mol/m³].
    pub fn molar_density<S: Scalar>(&self, pressure_bar: S) -> S {
        self.mass_density(pressure_bar) / WATER_MOLAR_MASS_KG_PER_MOL
    }

    /// Viscosity [cP].
    pub fn viscosity_cp<S: Scalar>(&self, pressure_bar: S) -> S {
        let y = (pressure_bar - self.reference_pressure_bar) * (-self.viscosibility_per_bar);
        S::from_f64(self.viscosity_cp) / (y * y * 0.5 + y + 1.0)
    }

    /// Surface volume of `moles` of water [m³].
    pub fn surface_volume_m3(&self, moles: f64) -> f64 {
        moles * WATER_MOLAR_MASS_KG_PER_MOL / self.surface_density_kg_per_m3
    }
}

/// Stone II three-phase relative permeability over SWOF/SGOF tables.
///
/// Holds the black-oil engine's [`RockFluidPropsThreePhase`] with tables set, so every curve and
/// the Stone II combination are the functions `relperm.rs` already runs and tests.
#[derive(Clone, Debug, PartialEq)]
pub struct StoneTwoRelPerm {
    props: RockFluidPropsThreePhase,
}

impl StoneTwoRelPerm {
    /// Validate and construct from SWOF/SGOF tables.
    ///
    /// The Corey fields of [`RockFluidPropsThreePhase`] are unused when tables are present except
    /// `k_ro_max`, Stone II's normalization, which is the oil relative permeability at connate
    /// water: SWOF's first `krow`, as ECLIPSE defines it. The Corey fields are filled from the
    /// tables' endpoints so a reader of the struct does not see invented exponents.
    pub fn new(swof: Vec<SwofRow>, sgof: Vec<SgofRow>) -> Result<Self, WaterError> {
        let tables = ThreePhaseScalTables { swof, sgof };
        tables.validate().map_err(WaterError::Table)?;
        for (row, r) in tables.swof.iter().enumerate() {
            if let Some(pc) = r.pcow
                && pc != 0.0
            {
                return Err(WaterError::CapillaryPressureNotSupported {
                    table: "SWOF",
                    row,
                    value: pc,
                });
            }
        }
        for (row, r) in tables.sgof.iter().enumerate() {
            if let Some(pc) = r.pcog
                && pc != 0.0
            {
                return Err(WaterError::CapillaryPressureNotSupported {
                    table: "SGOF",
                    row,
                    value: pc,
                });
            }
        }
        let first_swof = &tables.swof[0];
        let k_ro_max = first_swof.krow;
        if !(k_ro_max > 0.0) {
            return Err(WaterError::Table(
                "SWOF krow at connate water must be positive; it normalizes Stone II".to_string(),
            ));
        }
        let first_sgof = &tables.sgof[0];
        if (first_sgof.krog - k_ro_max).abs() > 1e-12 {
            return Err(WaterError::Table(format!(
                "SGOF krog at zero gas ({}) must equal SWOF krow at connate water ({k_ro_max}); \
                 Stone II assumes one oil endpoint",
                first_sgof.krog
            )));
        }
        let props = RockFluidPropsThreePhase {
            s_wc: first_swof.sw,
            s_or: 0.0,
            n_w: 1.0,
            n_o: 1.0,
            k_rw_max: tables.swof.last().map_or(0.0, |r| r.krw),
            k_ro_max,
            s_gc: 0.0,
            s_gr: 0.0,
            s_org: 0.0,
            n_g: 1.0,
            k_rg_max: tables.sgof.last().map_or(0.0, |r| r.krg),
            tables: Some(tables),
        };
        Ok(Self { props })
    }

    /// Water relative permeability at water saturation `sw`.
    pub fn kr_water<S: Scalar>(&self, sw: S) -> S {
        self.props.k_rw_generic(sw)
    }

    /// Gas (hydrocarbon vapour) relative permeability at gas saturation `sg`.
    pub fn kr_vapour<S: Scalar>(&self, sg: S) -> S {
        self.props.k_rg_generic(sg)
    }

    /// Oil (hydrocarbon liquid) relative permeability, Stone II, at `sw` and `sg`.
    pub fn kr_liquid<S: Scalar>(&self, sw: S, sg: S) -> S {
        self.props.k_ro_stone2_generic(sw, sg)
    }
}

/// Everything the water extension adds to a case: the phase's properties and the three-phase
/// relative permeability that replaces the hydrocarbon two-phase one.
#[derive(Clone, Debug, PartialEq)]
pub struct WaterModel {
    pub properties: WaterProperties,
    pub relperm: StoneTwoRelPerm,
}
