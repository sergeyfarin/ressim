import type { CaseMode, ToggleState } from "../catalog/caseCatalog";
import type {
  BasePresetProfile,
  BenchmarkProvenance,
  ComparisonSelection,
  LibraryCaseGroup,
  ProductFamily,
  ScenarioEditabilityPolicy,
  ScenarioNavigationState,
  ScenarioSource,
} from "../stores/phase2PresetContract";
import type { BenchmarkRunResult } from "../benchmarkRunModel";
import type { WarningPolicy } from "../warningPolicy";

export type {
  ComparisonSelection,
  LibraryCaseGroup,
  ProductFamily,
  ScenarioEditabilityPolicy,
  ScenarioNavigationState,
  ScenarioSource,
};

export type PermMode = "uniform" | "random" | "perLayer";

export type WellControlMode = "rate" | "pressure";

export type AnalyticalSolutionMode = "waterflood" | "depletion";

export type ModePanelParameterBindings = {
  nx: number;
  ny: number;
  nz: number;
  cellDx: number;
  cellDy: number;
  cellDz: number;
  initialPressure: number;
  initialSaturation: number;
  reservoirPorosity: number;
  mu_w: number;
  mu_o: number;
  c_o: number;
  c_w: number;
  rho_w: number;
  rho_o: number;
  rock_compressibility: number;
  depth_reference: number;
  volume_expansion_o: number;
  volume_expansion_w: number;
  gravityEnabled: boolean;
  permMode: PermMode;
  uniformPermX: number;
  uniformPermY: number;
  uniformPermZ: number;
  useRandomSeed: boolean;
  randomSeed: number;
  minPerm: number;
  maxPerm: number;
  layerPermsX: number[];
  layerPermsY: number[];
  layerPermsZ: number[];
  s_wc: number;
  s_or: number;
  n_w: number;
  n_o: number;
  k_rw_max: number;
  k_ro_max: number;
  capillaryEnabled: boolean;
  capillaryPEntry: number;
  capillaryLambda: number;
  well_radius: number;
  well_skin: number;
  injectorEnabled: boolean;
  injectorControlMode: WellControlMode;
  producerControlMode: WellControlMode;
  injectorBhp: number;
  producerBhp: number;
  targetInjectorRate: number;
  targetProducerRate: number;
  injectorI: number;
  injectorJ: number;
  producerI: number;
  producerJ: number;
  delta_t_days: number;
  max_sat_change_per_step: number;
  max_pressure_change_per_step: number;
  max_well_rate_change_fraction: number;
  analyticalSolutionMode: AnalyticalSolutionMode;
  analyticalDepletionRateScale: number;
  parameterOverrideCount: number;
  handleNzOrPermModeChange: () => void;
  handleAnalyticalSolutionModeChange: (mode: AnalyticalSolutionMode) => void;
};

export type ScenarioMode = Exclude<CaseMode, "benchmark">;

export type ModePanelProps = {
  activeMode: CaseMode;
  navigationState?: ScenarioNavigationState;
  isModified?: boolean;
  toggles: ToggleState;
  disabledOptions: Record<string, Record<string, string>>;
  onModeChange: (mode: CaseMode) => void;
  onParamEdit?: () => void;
  onToggleChange: (key: string, value: string) => void;
  basePreset?: BasePresetProfile | null;
  benchmarkProvenance?: BenchmarkProvenance | null;
  benchmarkSweepRunning?: boolean;
  benchmarkSweepProgressLabel?: string;
  benchmarkSweepError?: string;
  benchmarkRunResults?: BenchmarkRunResult[];
  onCloneBenchmarkToCustom?: () => void;
  onActivateLibraryEntry?: (entryKey: string) => void;
  onRunBenchmarkSelection?: (variantKeys: string[]) => void;
  onStopBenchmarkSweep?: () => void;
  params: ModePanelParameterBindings;
  validationErrors?: Record<string, string>;
  warningPolicy?: WarningPolicy;
};

export type ScenarioModePanelProps = {
  activeMode: ScenarioMode;
  navigationState?: ScenarioNavigationState;
  toggles: ToggleState;
  disabledOptions: Record<string, Record<string, string>>;
  onToggleChange: (key: string, value: string) => void;
  onParamEdit?: () => void;
  params: ModePanelParameterBindings;
  validationErrors?: Record<string, string>;
};

export type BenchmarkModePanelProps = {
  navigationState?: ScenarioNavigationState;
  toggles: ToggleState;
  disabledOptions: Record<string, Record<string, string>>;
  isModified?: boolean;
  benchmarkProvenance?: BenchmarkProvenance | null;
  benchmarkSweepRunning?: boolean;
  benchmarkSweepProgressLabel?: string;
  benchmarkSweepError?: string;
  benchmarkRunResults?: BenchmarkRunResult[];
  onToggleChange: (key: string, value: string) => void;
  onCloneBenchmarkToCustom?: () => void;
  onRunBenchmarkSelection?: (variantKeys: string[]) => void;
  onStopBenchmarkSweep?: () => void;
};
