import type { CaseMode, ToggleState } from "../caseCatalog";
import type {
  BasePresetProfile,
  BenchmarkProvenance,
} from "../stores/phase2PresetContract";
import type { WarningPolicy } from "../warningPolicy";
import type { ModePanelParameterBindings } from "./modePanelSchema";

export type ScenarioMode = Exclude<CaseMode, "benchmark">;

export type ModePanelProps = {
  activeMode: CaseMode;
  isModified?: boolean;
  toggles: ToggleState;
  disabledOptions: Record<string, Record<string, string>>;
  onModeChange: (mode: CaseMode) => void;
  onParamEdit?: () => void;
  onToggleChange: (key: string, value: string) => void;
  basePreset?: BasePresetProfile | null;
  benchmarkProvenance?: BenchmarkProvenance | null;
  onCloneBenchmarkToCustom?: () => void;
  params: ModePanelParameterBindings;
  validationErrors?: Record<string, string>;
  warningPolicy?: WarningPolicy;
};

export type ScenarioModePanelProps = {
  activeMode: ScenarioMode;
  toggles: ToggleState;
  disabledOptions: Record<string, Record<string, string>>;
  onToggleChange: (key: string, value: string) => void;
  onParamEdit?: () => void;
  params: ModePanelParameterBindings;
  validationErrors?: Record<string, string>;
};

export type ScenarioModePanelContentProps = Omit<
  ScenarioModePanelProps,
  "activeMode"
>;

export type BenchmarkModePanelProps = {
  toggles: ToggleState;
  disabledOptions: Record<string, Record<string, string>>;
  isModified?: boolean;
  benchmarkProvenance?: BenchmarkProvenance | null;
  onToggleChange: (key: string, value: string) => void;
  onCloneBenchmarkToCustom?: () => void;
};
