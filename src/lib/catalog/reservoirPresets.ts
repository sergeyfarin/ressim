import type { ModePanelParameterBindings } from "../ui/modePanelTypes";

export type ProfilePreset = {
  key: string;
  label: string;
  description: string;
  params: Partial<ModePanelParameterBindings>;
};

export const ROCK_PRESETS: readonly ProfilePreset[] = [
  {
    key: "sandstone",
    label: "Sandstone",
    description: "Conventional sandstone reservoir (high perm, moderate porosity)",
    params: {
      reservoirPorosity: 0.22,
      uniformPermX: 200, uniformPermY: 200, uniformPermZ: 20,
      permMode: "uniform",
      rock_compressibility: 5e-6,
      s_wc: 0.20, s_or: 0.20, n_w: 3.0, n_o: 2.0, k_rw_max: 0.3, k_ro_max: 1.0,
      capillaryPEntry: 0.5, capillaryLambda: 2.0,
    }
  },
  {
    key: "carbonate",
    label: "Carbonate",
    description: "Carbonate reservoir (variable perm, lower porosity, oil-wet tendency)",
    params: {
      reservoirPorosity: 0.12,
      uniformPermX: 50, uniformPermY: 50, uniformPermZ: 5,
      permMode: "uniform",
      rock_compressibility: 8e-6,
      s_wc: 0.10, s_or: 0.30, n_w: 2.0, n_o: 4.0, k_rw_max: 0.3, k_ro_max: 1.0,
      capillaryPEntry: 3.0, capillaryLambda: 1.5,
    }
  },
  {
    key: "shale",
    label: "Shale / Tight",
    description: "Tight/shale reservoir (very low perm, low porosity)",
    params: {
      reservoirPorosity: 0.06,
      uniformPermX: 0.1, uniformPermY: 0.1, uniformPermZ: 0.01,
      permMode: "uniform",
      rock_compressibility: 3e-6,
      s_wc: 0.30, s_or: 0.35, n_w: 4.0, n_o: 2.0, k_rw_max: 0.15, k_ro_max: 1.0,
      capillaryPEntry: 20.0, capillaryLambda: 1.0,
    }
  },
  {
    key: "high_perm_sand",
    label: "High Perm Sand",
    description: "Unconsolidated/high-perm sand (excellent properties)",
    params: {
      reservoirPorosity: 0.30,
      uniformPermX: 2000, uniformPermY: 2000, uniformPermZ: 200,
      permMode: "uniform",
      rock_compressibility: 1e-5,
      s_wc: 0.15, s_or: 0.15, n_w: 2.5, n_o: 2.5, k_rw_max: 0.4, k_ro_max: 1.0,
      capillaryPEntry: 0.1, capillaryLambda: 3.0,
    }
  }
];

export const FLUID_PRESETS: readonly ProfilePreset[] = [
  {
    key: "dead_light_oil",
    label: "Light Oil (Constant)",
    description: "Standard light oil with no dissolved gas",
    params: {
      pvtMode: "constant",
      mu_o: 1.0, mu_w: 0.5,
      rho_o: 800, rho_w: 1020,
      c_o: 1.5e-5, c_w: 4.5e-6,
      volume_expansion_o: 1.2, volume_expansion_w: 1.0,
      initialPressure: 250,
    }
  },
  {
    key: "heavy_oil",
    label: "Heavy Oil (Constant)",
    description: "Viscous dead heavy oil",
    params: {
      pvtMode: "constant",
      mu_o: 50.0, mu_w: 1.0,
      rho_o: 980, rho_w: 1000,
      c_o: 0.8e-5, c_w: 4.0e-6,
      volume_expansion_o: 1.05, volume_expansion_w: 1.0,
      initialPressure: 150,
    }
  },
  {
    key: "volatile_oil",
    label: "Volatile Oil (Black-Oil)",
    description: "Live oil properties with high GOR",
    params: {
      pvtMode: "black-oil",
      apiGravity: 42,
      gasSpecificGravity: 0.8,
      reservoirTemperature: 80,
      bubblePoint: 200,
      mu_w: 0.5, rho_w: 1020, c_w: 4.5e-6, volume_expansion_w: 1.0,
      initialPressure: 250,
    }
  },
  {
    key: "gas_condensate",
    label: "Light Oil/Gas Condensate (Black-Oil)",
    description: "Extremely light Black-Oil system (high API, high shrinkage)",
    params: {
      pvtMode: "black-oil",
      apiGravity: 55,
      gasSpecificGravity: 0.65,
      reservoirTemperature: 100,
      bubblePoint: 350,
      mu_w: 0.3, rho_w: 1000, c_w: 4e-6, volume_expansion_w: 1.0,
      initialPressure: 400,
    }
  }
];
