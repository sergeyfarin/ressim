import type { PresetEntry } from '../presetCases';

export const depletion_corner_producer: PresetEntry = {
    "key": "depletion_corner_producer",
    "category": "depletion",
    "mode": "dep",
    "label": "Corner Producer (1D)",
    "description": "1D depletion with producer at corner — linear geometry",
    "params": {
        "mu_w": 0.5,
        "mu_o": 1,
        "c_o": 0.00001,
        "c_w": 0.000003,
        "rock_compressibility": 0.000001,
        "depth_reference": 0,
        "volume_expansion_o": 1,
        "volume_expansion_w": 1,
        "rho_w": 1000,
        "rho_o": 800,
        "well_radius": 0.1,
        "well_skin": 0,
        "max_pressure_change_per_step": 75,
        "max_well_rate_change_fraction": 0.75,
        "injectorEnabled": false,
        "injectorControlMode": "pressure",
        "producerControlMode": "pressure",
        "injectorBhp": 500,
        "producerBhp": 80,
        "targetInjectorRate": 350,
        "targetProducerRate": 350,
        "reservoirPorosity": 0.2,
        "nx": 48,
        "ny": 1,
        "nz": 1,
        "cellDx": 10,
        "cellDy": 10,
        "cellDz": 5,
        "delta_t_days": 0.5,
        "steps": 36,
        "max_sat_change_per_step": 0.1,
        "initialPressure": 300,
        "initialSaturation": 0.1,
        "injectorI": 0,
        "injectorJ": 0,
        "producerI": 0,
        "producerJ": 0,
        "permMode": "uniform",
        "uniformPermX": 100,
        "uniformPermY": 100,
        "uniformPermZ": 10,
        "s_wc": 0.1,
        "s_or": 0.1,
        "n_w": 2,
        "n_o": 2,
        "k_rw_max": 1,
        "k_ro_max": 1,
        "gravityEnabled": false,
        "capillaryEnabled": false,
        "capillaryPEntry": 0,
        "capillaryLambda": 2
    },
    "layoutConfig": {
        "rateChart": {
            "logScale": true,
            "curves": {
                "Water Cut (Sim)": {
                    "disabled": true
                },
                "Water Cut (Reference Solution)": {
                    "disabled": true
                }
            }
        }
    }
};
