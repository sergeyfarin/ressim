use serde::{Deserialize, Serialize};

use crate::well_control::{ResolvedWellControl, WellControlDecision};
use crate::{InjectedFluid, ReservoirSimulator};

const MIN_GOR_OIL_RATE_SC_DAY: f64 = 10.0;

#[derive(Clone, Serialize, Deserialize)]
pub struct WellRates {
    pub oil_rate: f64,
    pub water_rate: f64,
    pub total_liquid_rate: f64,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct TimePointRates {
    pub time: f64,
    pub total_production_oil: f64,
    pub total_production_liquid: f64,
    pub total_production_liquid_reservoir: f64,
    pub total_injection: f64,
    pub total_injection_reservoir: f64,
    /// Material balance error [m³]: cumulative (injection - production) vs actual in-place change
    pub material_balance_error_m3: f64,
    /// Average reservoir pressure [bar]
    pub avg_reservoir_pressure: f64,
    /// Average water saturation
    pub avg_water_saturation: f64,
    /// Total gas produced [m³/day] (non-zero only in three-phase mode)
    #[serde(default)]
    pub total_production_gas: f64,
    /// Average gas saturation (non-zero only in three-phase mode)
    #[serde(default)]
    pub avg_gas_saturation: f64,
    /// Gas material balance error [Sm³]: cumulative (surface gas injection − surface gas production)
    /// vs actual in-place total-gas inventory change expressed at standard conditions.
    /// Includes both free gas and dissolved gas. Non-zero only in three-phase mode.
    #[serde(default)]
    pub material_balance_error_gas_m3: f64,
    /// Producing gas-oil ratio [Sm³/Sm³]: total surface gas / surface oil at producers.
    /// Includes both free gas and dissolved gas (Rs) liberated at surface.
    /// Non-zero only in three-phase mode with PVT table.
    #[serde(default)]
    pub producing_gor: f64,
    /// Fraction of rate-controlled producer completions currently clamped by BHP limits.
    #[serde(default)]
    pub producer_bhp_limited_fraction: f64,
    /// Fraction of rate-controlled injector completions currently clamped by BHP limits.
    #[serde(default)]
    pub injector_bhp_limited_fraction: f64,
}

impl ReservoirSimulator {
    pub(crate) fn record_step_report(
        &mut self,
        well_controls: &[Option<ResolvedWellControl>],
        dt_days: f64,
        actual_change_m3: f64,
        actual_change_gas_sc: f64,
    ) {
        let n_cells = self.nx * self.ny * self.nz;
        let mut total_prod_oil = 0.0;
        let mut total_prod_liquid = 0.0;
        let mut total_prod_liquid_reservoir = 0.0;
        let mut total_injection = 0.0;
        let mut total_injection_reservoir = 0.0;
        let mut total_water_injection_reservoir = 0.0;
        let mut total_prod_water_reservoir = 0.0;
        let mut total_prod_gas = 0.0;
        let mut total_prod_dissolved_gas = 0.0;
        let mut total_gas_injection_sc = 0.0;
        let mut producer_rate_controlled_wells = 0usize;
        let mut injector_rate_controlled_wells = 0usize;
        let mut producer_bhp_limited_wells = 0usize;
        let mut injector_bhp_limited_wells = 0usize;

        for (w_idx, w) in self.wells.iter().enumerate() {
            let id = self.idx(w.i, w.j, w.k);
            if let Some(control) = well_controls.get(w_idx).and_then(|control| *control) {
                let group_rate_controlled = if w.injector {
                    self.injector_rate_controlled
                } else {
                    self.producer_rate_controlled
                };
                if group_rate_controlled {
                    if w.injector {
                        injector_rate_controlled_wells += 1;
                        if control.bhp_limited {
                            injector_bhp_limited_wells += 1;
                        }
                    } else {
                        producer_rate_controlled_wells += 1;
                        if control.bhp_limited {
                            producer_bhp_limited_wells += 1;
                        }
                    }
                }

                let q_m3_day =
                    match self.well_transport_rate_from_control(w, control, self.pressure[id]) {
                        Some(q_m3_day) if q_m3_day.is_finite() => q_m3_day,
                        _ => continue,
                    };

                if w.injector {
                    total_injection_reservoir += -q_m3_day;
                    if self.three_phase_mode {
                        match self.injected_fluid {
                            InjectedFluid::Water => {
                                if matches!(control.decision, WellControlDecision::Rate { .. }) {
                                    if let Some(surface_target_sc_day) =
                                        self.target_injector_surface_rate_m3_day
                                    {
                                        total_injection += surface_target_sc_day
                                            / self.injector_well_count().max(1) as f64;
                                    } else {
                                        total_injection += -q_m3_day / self.b_w.max(1e-9);
                                    }
                                } else {
                                    total_injection += -q_m3_day / self.b_w.max(1e-9);
                                }
                                total_water_injection_reservoir += -q_m3_day;
                            }
                            InjectedFluid::Gas => {
                                let bg = self.get_b_g(self.pressure[id]).max(1e-9);
                                total_injection += -q_m3_day / bg;
                                total_gas_injection_sc += -q_m3_day / bg;
                            }
                        }
                    } else {
                        if matches!(control.decision, WellControlDecision::Rate { .. }) {
                            if let Some(surface_target_sc_day) =
                                self.target_injector_surface_rate_m3_day
                            {
                                total_injection += surface_target_sc_day
                                    / self.injector_well_count().max(1) as f64;
                            } else {
                                total_injection += -q_m3_day / self.b_w.max(1e-9);
                            }
                        } else {
                            total_injection += -q_m3_day / self.b_w.max(1e-9);
                        }
                        total_water_injection_reservoir += -q_m3_day;
                    }
                } else {
                    total_prod_liquid_reservoir += q_m3_day;
                    let producer_state = self.producer_control_state_from_resolved_control(
                        w,
                        control,
                        &self.pressure,
                    );
                    let (fw, fg) = if self.three_phase_mode {
                        (producer_state.water_fraction, producer_state.gas_fraction)
                    } else {
                        (self.frac_flow_water(id), 0.0)
                    };

                    total_prod_water_reservoir += q_m3_day * fw;
                    let bo = producer_state.oil_fvf.max(1e-9);
                    let bw = self.b_w.max(1e-9);
                    let oil_rate_sc = q_m3_day * (1.0 - fw - fg) / bo;
                    let water_rate_sc = q_m3_day * fw / bw;
                    total_prod_oil += oil_rate_sc;
                    total_prod_liquid += oil_rate_sc + water_rate_sc;

                    let bg = producer_state.gas_fvf.max(1e-9);
                    total_prod_gas += q_m3_day * fg / bg;
                    if self.pvt_table.is_some() && self.three_phase_mode {
                        total_prod_dissolved_gas += oil_rate_sc * producer_state.rs_sm3_sm3;
                    }
                }
            }
        }

        self.cumulative_injection_m3 += total_water_injection_reservoir * dt_days;
        self.cumulative_production_m3 += total_prod_water_reservoir * dt_days;

        let net_water_added_m3 =
            (total_water_injection_reservoir - total_prod_water_reservoir) * dt_days;
        self.cumulative_mb_error_m3 += net_water_added_m3 - actual_change_m3;

        if self.three_phase_mode {
            let total_gas_prod_sc = total_prod_gas + total_prod_dissolved_gas;
            let net_gas_added_sc = (total_gas_injection_sc - total_gas_prod_sc) * dt_days;
            self.cumulative_mb_gas_error_m3 += net_gas_added_sc - actual_change_gas_sc;
        }

        let mb_error = self.cumulative_mb_error_m3.abs();

        let mut sum_sat_water = 0.0;
        let mut sum_sat_gas = 0.0;
        for i in 0..n_cells {
            sum_sat_water += self.sat_water[i];
            sum_sat_gas += self.sat_gas[i];
        }

        let avg_reservoir_pressure = self.average_reservoir_pressure_pv_weighted();
        let avg_water_saturation = if n_cells > 0 {
            sum_sat_water / n_cells as f64
        } else {
            0.0
        };
        let avg_gas_saturation = if n_cells > 0 {
            sum_sat_gas / n_cells as f64
        } else {
            0.0
        };

        let total_gas_sc = total_prod_gas + total_prod_dissolved_gas;
        let producing_gor = if total_prod_oil > MIN_GOR_OIL_RATE_SC_DAY {
            total_gas_sc / total_prod_oil
        } else {
            0.0
        };
        let producer_bhp_limited_fraction = if producer_rate_controlled_wells > 0 {
            producer_bhp_limited_wells as f64 / producer_rate_controlled_wells as f64
        } else {
            0.0
        };
        let injector_bhp_limited_fraction = if injector_rate_controlled_wells > 0 {
            injector_bhp_limited_wells as f64 / injector_rate_controlled_wells as f64
        } else {
            0.0
        };

        self.rate_history.push(TimePointRates {
            time: self.time_days + dt_days,
            total_production_oil: total_prod_oil,
            total_production_liquid: total_prod_liquid,
            total_production_liquid_reservoir: total_prod_liquid_reservoir,
            total_injection,
            total_injection_reservoir,
            material_balance_error_m3: mb_error,
            material_balance_error_gas_m3: self.cumulative_mb_gas_error_m3.abs(),
            avg_reservoir_pressure,
            avg_water_saturation,
            total_production_gas: total_gas_sc,
            avg_gas_saturation,
            producing_gor,
            producer_bhp_limited_fraction,
            injector_bhp_limited_fraction,
        });
    }
}
