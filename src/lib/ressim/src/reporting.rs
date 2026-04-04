use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use crate::fim::state::FimState;
use crate::fim::wells::{
    build_well_topology, perforation_component_rates_sc_day, physical_well_control,
    producer_control_state,
};
use crate::well_control::ResolvedWellControl;
use crate::{InjectedFluid, ReservoirSimulator};

const MIN_GOR_OIL_RATE_SC_DAY: f64 = 10.0;

/// Configuration for sweep efficiency diagnostics computed per time step.
#[derive(Clone, Serialize, Deserialize)]
pub struct SweepConfig {
    /// Which sweep components are physically meaningful: "areal", "vertical", or "both".
    pub geometry: String,
    /// Water saturation threshold above which a cell is considered swept.
    pub swept_threshold: f64,
    /// Initial oil saturation (used for mobile oil recovered in "both" mode).
    pub initial_oil_saturation: f64,
    /// Residual oil saturation S_or.
    pub residual_oil_saturation: f64,
}

/// Per-step sweep efficiency metrics appended to each `TimePointRates` entry.
#[derive(Clone, Serialize, Deserialize)]
pub struct SweepMetrics {
    /// Areal sweep efficiency (None for "both" geometry).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub e_a: Option<f64>,
    /// Vertical sweep efficiency (None for "both" geometry).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub e_v: Option<f64>,
    /// Volumetric sweep efficiency.
    pub e_vol: f64,
    /// Fraction of initial mobile oil recovered (Some only for "both" geometry).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mobile_oil_recovered: Option<f64>,
}

fn compute_sweep_metrics(sim: &ReservoirSimulator, config: &SweepConfig) -> SweepMetrics {
    let nx = sim.nx;
    let ny = sim.ny;
    let nz = sim.nz;
    let threshold = config.swept_threshold;

    let mut swept_cells = 0.0_f64;
    let mut swept_columns = 0.0_f64;

    for j in 0..ny {
        for i in 0..nx {
            let mut column_weight = 0.0_f64;
            for k in 0..nz {
                let sw = sim.sat_water[k * nx * ny + j * nx + i];
                let weight = if sw > threshold { 1.0 } else { 0.0 };
                swept_cells += weight;
                column_weight = column_weight.max(weight);
            }
            swept_columns += column_weight;
        }
    }

    let total = (nx * ny * nz) as f64;
    let e_vol = if total > 0.0 {
        swept_cells / total
    } else {
        0.0
    };
    let e_a_raw = if (nx * ny) > 0 {
        swept_columns / (nx * ny) as f64
    } else {
        0.0
    };

    match config.geometry.as_str() {
        "areal" => SweepMetrics {
            e_a: Some(e_a_raw),
            e_v: Some(1.0),
            e_vol,
            mobile_oil_recovered: None,
        },
        "vertical" => SweepMetrics {
            e_a: Some(1.0),
            e_v: Some(e_vol),
            e_vol,
            mobile_oil_recovered: None,
        },
        _ => {
            // "both" or unknown: eA/eV are null, compute mobile oil recovered
            let initial_mobile_per_cell =
                (config.initial_oil_saturation - config.residual_oil_saturation).max(0.0);
            let initial_mobile = total * initial_mobile_per_cell;
            let mobile_oil_recovered = if initial_mobile > 1e-12 {
                let remaining: f64 = sim
                    .sat_oil
                    .iter()
                    .take(nx * ny * nz)
                    .map(|&so| (so - config.residual_oil_saturation).max(0.0))
                    .sum();
                (1.0 - remaining / initial_mobile).clamp(0.0, 1.0)
            } else {
                0.0
            };
            SweepMetrics {
                e_a: None,
                e_v: None,
                e_vol,
                mobile_oil_recovered: Some(mobile_oil_recovered),
            }
        }
    }
}

fn effective_injected_fluid(sim: &ReservoirSimulator) -> InjectedFluid {
    if sim.three_phase_mode {
        sim.injected_fluid
    } else {
        InjectedFluid::Water
    }
}

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
    /// Oil reporting/material-balance diagnostic [Sm³]: cumulative reported oil production vs
    /// actual stock-tank oil inventory depletion.
    /// Current runtime scenarios do not inject oil, so this is the first-class direct diagnostic
    /// for whether reported oil production is tracking inventory change. Serialized with a default
    /// to preserve hydration of older histories.
    #[serde(default)]
    pub material_balance_error_oil_m3: f64,
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
    /// Fraction of rate-controlled producer physical wells currently clamped by BHP limits.
    #[serde(default)]
    pub producer_bhp_limited_fraction: f64,
    /// Fraction of rate-controlled injector physical wells currently clamped by BHP limits.
    #[serde(default)]
    pub injector_bhp_limited_fraction: f64,
    /// Sweep efficiency diagnostics (present when sweep config is set).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sweep: Option<SweepMetrics>,
}

impl ReservoirSimulator {
    pub(crate) fn average_reservoir_pressure_pv_weighted(&self) -> f64 {
        let mut weighted_pressure_sum = 0.0;
        let mut pore_volume_sum = 0.0;

        for id in 0..self.nx * self.ny * self.nz {
            let pore_volume = self.pore_volume_m3(id);
            if pore_volume <= 0.0 || !pore_volume.is_finite() {
                continue;
            }
            weighted_pressure_sum += self.pressure[id] * pore_volume;
            pore_volume_sum += pore_volume;
        }

        if pore_volume_sum > 0.0 {
            weighted_pressure_sum / pore_volume_sum
        } else {
            0.0
        }
    }

    pub(crate) fn record_step_report(
        &mut self,
        well_controls: &[Option<ResolvedWellControl>],
        dt_days: f64,
        actual_change_m3: f64,
        actual_oil_removed_sc: f64,
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
        let mut counted_control_groups = HashSet::new();

        for (w_idx, w) in self.wells.iter().enumerate() {
            let id = self.idx(w.i, w.j, w.k);
            if let Some(control) = well_controls.get(w_idx).and_then(|control| *control) {
                let control_config = self.well_control_config(w);
                let group_key = self.well_control_group_key(w);
                if control_config.rate_controlled && counted_control_groups.insert(group_key) {
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
                                total_injection += -q_m3_day / self.b_w.max(1e-9);
                                total_water_injection_reservoir += -q_m3_day;
                            }
                            InjectedFluid::Gas => {
                                let bg = self.get_b_g(self.pressure[id]).max(1e-9);
                                total_injection += -q_m3_day / bg;
                                total_gas_injection_sc += -q_m3_day / bg;
                            }
                        }
                    } else {
                        total_injection += -q_m3_day / self.b_w.max(1e-9);
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

        let produced_oil_sc = total_prod_oil * dt_days;
        self.cumulative_mb_oil_error_m3 += produced_oil_sc - actual_oil_removed_sc;

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

        let sweep = self
            .sweep_config
            .as_ref()
            .map(|cfg| compute_sweep_metrics(self, cfg));

        self.rate_history.push(TimePointRates {
            time: self.time_days + dt_days,
            total_production_oil: total_prod_oil,
            total_production_liquid: total_prod_liquid,
            total_production_liquid_reservoir: total_prod_liquid_reservoir,
            total_injection,
            total_injection_reservoir,
            material_balance_error_m3: mb_error,
            material_balance_error_oil_m3: self.cumulative_mb_oil_error_m3.abs(),
            material_balance_error_gas_m3: self.cumulative_mb_gas_error_m3.abs(),
            avg_reservoir_pressure,
            avg_water_saturation,
            total_production_gas: total_gas_sc,
            avg_gas_saturation,
            producing_gor,
            producer_bhp_limited_fraction,
            injector_bhp_limited_fraction,
            sweep,
        });
    }

    pub(crate) fn record_fim_step_report(
        &mut self,
        state: &FimState,
        dt_days: f64,
        actual_change_m3: f64,
        actual_oil_removed_sc: f64,
        actual_change_gas_sc: f64,
    ) {
        let n_cells = self.nx * self.ny * self.nz;
        let topology = build_well_topology(self);
        let mut total_prod_oil = 0.0;
        let mut total_prod_liquid = 0.0;
        let mut total_prod_liquid_reservoir = 0.0;
        let mut total_injection = 0.0;
        let mut total_injection_reservoir = 0.0;
        let mut total_water_injection_reservoir = 0.0;
        let mut total_prod_water_reservoir = 0.0;
        let mut total_prod_gas = 0.0;
        let mut total_gas_injection_sc = 0.0;
        let mut producer_rate_controlled_wells = 0usize;
        let mut injector_rate_controlled_wells = 0usize;
        let mut producer_bhp_limited_wells = 0usize;
        let mut injector_bhp_limited_wells = 0usize;

        for (well_idx, physical_well) in topology.wells.iter().enumerate() {
            let control = physical_well_control(self, &topology, well_idx);
            if control.enabled && control.rate_controlled {
                let bhp_bar = state.well_bhp[well_idx];
                let bhp_limited = if physical_well.injector {
                    bhp_bar >= control.bhp_limit - 1e-6
                } else {
                    bhp_bar <= control.bhp_limit + 1e-6
                };

                if physical_well.injector {
                    injector_rate_controlled_wells += 1;
                    if bhp_limited {
                        injector_bhp_limited_wells += 1;
                    }
                } else {
                    producer_rate_controlled_wells += 1;
                    if bhp_limited {
                        producer_bhp_limited_wells += 1;
                    }
                }
            }
        }

        for (perf_idx, perforation) in topology.perforations.iter().enumerate() {
            let q_m3_day = state.perforation_rates_m3_day[perf_idx];
            let components_sc_day =
                perforation_component_rates_sc_day(self, state, &topology, perf_idx);

            if perforation.injector {
                total_injection_reservoir += (-q_m3_day).max(0.0);
                match effective_injected_fluid(self) {
                    InjectedFluid::Water => {
                        total_injection += (-components_sc_day[0]).max(0.0);
                        total_water_injection_reservoir += (-q_m3_day).max(0.0);
                    }
                    InjectedFluid::Gas => {
                        total_injection += (-components_sc_day[2]).max(0.0);
                        total_gas_injection_sc += (-components_sc_day[2]).max(0.0);
                    }
                }
                continue;
            }

            total_prod_liquid_reservoir += q_m3_day.max(0.0);
            let producer = producer_control_state(self, state, perforation);
            total_prod_water_reservoir += q_m3_day.max(0.0) * producer.water_fraction;
            total_prod_oil += components_sc_day[1].max(0.0);
            total_prod_liquid += components_sc_day[0].max(0.0) + components_sc_day[1].max(0.0);
            total_prod_gas += components_sc_day[2].max(0.0);
        }

        self.cumulative_injection_m3 += total_water_injection_reservoir * dt_days;
        self.cumulative_production_m3 += total_prod_water_reservoir * dt_days;

        let net_water_added_m3 =
            (total_water_injection_reservoir - total_prod_water_reservoir) * dt_days;
        self.cumulative_mb_error_m3 += net_water_added_m3 - actual_change_m3;

        let produced_oil_sc = total_prod_oil * dt_days;
        self.cumulative_mb_oil_error_m3 += produced_oil_sc - actual_oil_removed_sc;

        if self.three_phase_mode {
            let net_gas_added_sc = (total_gas_injection_sc - total_prod_gas) * dt_days;
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

        let producing_gor = if total_prod_oil > MIN_GOR_OIL_RATE_SC_DAY {
            total_prod_gas / total_prod_oil
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

        let sweep = self
            .sweep_config
            .as_ref()
            .map(|cfg| compute_sweep_metrics(self, cfg));

        self.rate_history.push(TimePointRates {
            time: self.time_days + dt_days,
            total_production_oil: total_prod_oil,
            total_production_liquid: total_prod_liquid,
            total_production_liquid_reservoir: total_prod_liquid_reservoir,
            total_injection,
            total_injection_reservoir,
            material_balance_error_m3: mb_error,
            material_balance_error_oil_m3: self.cumulative_mb_oil_error_m3.abs(),
            avg_reservoir_pressure,
            avg_water_saturation,
            total_production_gas: total_prod_gas,
            avg_gas_saturation,
            material_balance_error_gas_m3: self.cumulative_mb_gas_error_m3.abs(),
            producing_gor,
            producer_bhp_limited_fraction,
            injector_bhp_limited_fraction,
            sweep,
        });
    }
}
