import {
    catalog,
    buildCaseKey,
    composeCaseParams,
    getCaseLibraryEntry,
    resolveCaseLibraryEntryFromScenario,
    getBenchmarkEntry,
    getBenchmarkFamily,
    getBenchmarkVariantsForFamily,
    getDefaultToggles,
    getDisabledOptions,
    stabilizeToggleState,
    type CaseMode,
    type ToggleState,
} from '../catalog/caseCatalog';
import { evaluateAnalyticalStatus, type AnalyticalStatus } from '../warningPolicy';
import {
    getDefaultScenarioAnalyticalMode,
    getScenario,
    getScenarioChartLayout,
    getDefaultVariantKeys,
    getScenarioWithVariantParams,
    resolveCapabilities,
    suppressesPrimaryAnalyticalOverlays,
    type ScenarioAnalyticalOption,
    type ScenarioAnalyticalOutput,
} from '../catalog/scenarios';
import {
    buildReferenceCloneProvenance,
    buildBasePresetProfile,
    buildComparisonSelection,
    buildOverrideResetPlan,
    buildParameterOverrides,
    groupParameterOverrides,
    shouldAllowReferenceClone,
    shouldAutoClearModifiedState,
    resolveProductFamily,
    resolveScenarioSource,
    buildScenarioEditabilityPolicy,
    type ScenarioNavigationState,
    type ReferenceProvenance,
    type ComparisonSelection,
} from './phase2PresetContract';
import type { ParameterStore } from './parameterStore.svelte';
import type { RuntimeStore } from './runtimeStore.svelte';
import type { BenchmarkFamily } from '../catalog/benchmarkCases';
import { getReferenceRateChartLayoutConfig } from '../charts/referenceChartConfig';
import type { RockProps, FluidProps } from '../analytical/fractionalFlow';
import {
    computeSweepRecoveryFactor,
    type SweepAnalyticalMethod,
    type SweepGeometry,
    type SweepRFResult,
} from '../analytical/sweepEfficiency';
import type { GridState, WellState, SimulatorSnapshot } from '../simulator-types';

// ---------- Presentation-layer types (live in nav store; used by App.svelte) ----------

export type OutputSelectionProfile = {
    gridState: GridState | null;
    nx: number; ny: number; nz: number;
    cellDx: number; cellDy: number; cellDz: number;
    simTime: number; injectionRate: number;
    scenarioMode: 'waterflood' | 'depletion' | 'none';
    sourceLabel: string; producerJ: number; initialSaturation: number;
    rockProps: RockProps; fluidProps: FluidProps;
};

export type Output3DSelection = {
    history: SimulatorSnapshot[];
    nx: number; ny: number; nz: number;
    cellDx: number; cellDy: number; cellDz: number; cellDzPerLayer: number[];
    gridState: GridState | null;
    wellState: WellState | null;
    replayTime: number | null;
    currentIndex: number;
    sourceLabel: string;
};

type SimSweepPoint = {
    time: number;
    eA: number | null;
    eV: number | null;
    eVol: number;
    mobileOilRecovered: number | null;
};

const EMPTY_ANALYTICAL_OUTPUT: ScenarioAnalyticalOutput = {
    production: [],
    meta: { mode: 'none', shapeFactor: null, shapeLabel: '' },
};

// ---------- Constants ----------

const CUSTOM_SUBCASE_BY_MODE: Partial<Record<CaseMode, { key: string; label: string }>> = {
    dep: { key: 'depletion_custom_subcase', label: 'Custom Depletion Sub-case' },
    wf: { key: 'waterflood_custom_subcase', label: 'Custom Waterflood Sub-case' },
    sim: { key: 'simulation_custom_subcase', label: 'Custom Simulation Sub-case' },
};

// ---------- Store ----------

class NavigationStoreImpl {
    readonly #params: ParameterStore;
    readonly #runtime: RuntimeStore;

    constructor(params: ParameterStore, runtime: RuntimeStore) {
        this.#params = params;
        this.#runtime = runtime;

        $effect(() => {
            if (!shouldAutoClearModifiedState({
                isModified: this.isModified,
                referenceProvenance: this.referenceProvenance,
                parameterOverrideCount: this.parameterOverrideCount,
            })) return;

            this.isModified = false;
            this.baseCaseSignature = this.#params.buildCaseSignature();
        });
    }

    // ===== $state: Navigation =====

    activeMode = $state<CaseMode>('dep');
    activeCase = $state('');
    isModified = $state(false);
    toggles = $state<ToggleState>(getDefaultToggles('dep'));
    referenceProvenance: ReferenceProvenance | null = $state(null);
    activeComparisonSelection = $state<ComparisonSelection>(buildComparisonSelection());
    explicitLibraryEntryKey: string | null = $state(null);

    // Scenario-picker state
    activeScenarioKey: string | null = $state(null);
    activeSensitivityDimensionKey: string | null = $state(null);
    activeAnalyticalOptionKey: string | null = $state(null);
    activeVariantKeys: string[] = $state([]);
    isCustomMode = $state(false);

    // Tracks case signature for modified-state detection
    baseCaseSignature = $state('');

    // ===== $derived =====

    disabledOptions = $derived(getDisabledOptions(this.toggles));

    // ===== $derived: Library / Navigation =====

    activeLibraryEntry = $derived.by(() => {
        if (this.isModified) return null;

        if (this.explicitLibraryEntryKey) {
            return getCaseLibraryEntry(this.explicitLibraryEntryKey);
        }

        return resolveCaseLibraryEntryFromScenario({
            activeMode: this.activeMode,
            benchmarkId: this.toggles.benchmarkId ?? null,
            scenarioParams: composeCaseParams(this.toggles),
        });
    });

    activeReferenceFamily = $derived.by(() => {
        const benchmarkFamilyKey = this.activeLibraryEntry?.benchmarkFamilyKey ?? null;
        return benchmarkFamilyKey ? getBenchmarkFamily(benchmarkFamilyKey) : null;
    });

    activeNavigationLibraryEntry = $derived.by(() => {
        if (this.activeLibraryEntry) return this.activeLibraryEntry;
        if (this.referenceProvenance?.sourceCaseKey) {
            return getCaseLibraryEntry(this.referenceProvenance.sourceCaseKey);
        }
        return null;
    });

    basePreset = $derived.by(() => {
        const benchmarkId = this.activeReferenceFamily?.key ?? null;
        const benchmarkLabel = this.activeLibraryEntry?.label
            ?? (benchmarkId ? getBenchmarkEntry(benchmarkId)?.label ?? null : null);

        return buildBasePresetProfile({
            key: this.activeCase,
            mode: this.activeMode,
            toggles: this.toggles,
            isModified: this.isModified,
            benchmarkId,
            benchmarkLabel,
            benchmarkAnalyticalMethod: this.activeReferenceFamily?.analyticalMethod ?? null,
            activeLibraryCaseKey: this.activeLibraryEntry?.key ?? null,
            activeLibraryGroup: this.activeLibraryEntry?.group ?? null,
        });
    });

    navigationState = $derived.by((): ScenarioNavigationState => {
        const benchmarkId = this.activeReferenceFamily?.key ?? null;
        const activeSource = resolveScenarioSource({ isModified: this.isModified });
        const activeLibraryGroup = activeSource === 'custom' ? null : (this.activeLibraryEntry?.group ?? null);

        return {
            activeFamily: resolveProductFamily({
                activeMode: this.activeMode,
                activeLibraryFamily: this.activeNavigationLibraryEntry?.family ?? null,
                benchmarkAnalyticalMethod: this.activeReferenceFamily?.analyticalMethod ?? null,
                benchmarkId,
            }),
            activeSource,
            activeLibraryCaseKey: activeSource === 'custom' ? null : (this.activeLibraryEntry?.key ?? null),
            activeLibraryGroup,
            sourceLabel: activeSource === 'custom' ? null : (this.activeLibraryEntry?.sourceLabel ?? null),
            referenceSourceLabel: activeSource === 'custom' ? null : (this.activeLibraryEntry?.referenceSourceLabel ?? null),
            provenanceSummary: activeSource === 'custom' ? null : (this.activeLibraryEntry?.provenanceSummary ?? null),
            activeComparisonSelection: buildComparisonSelection(this.activeComparisonSelection),
            editabilityPolicy: buildScenarioEditabilityPolicy({
                caseSource: activeSource,
                activeLibraryGroup,
            }),
        };
    });

    parameterOverrides = $derived.by(() => {
        return buildParameterOverrides({
            currentParams: this.#params.buildCurrentParameterSnapshot(),
            baseParams: composeCaseParams(this.toggles),
        });
    });

    parameterOverrideGroups = $derived(groupParameterOverrides(this.parameterOverrides));
    parameterOverrideCount = $derived(Object.keys(this.parameterOverrides).length);

    analyticalStatus = $derived.by((): AnalyticalStatus => {
        return evaluateAnalyticalStatus({
            activeMode: this.activeMode,
            analyticalMode: this.#params.analyticalMode,
            injectorEnabled: this.#params.injectorEnabled,
            gravityEnabled: this.#params.gravityEnabled,
            capillaryEnabled: this.#params.capillaryEnabled,
            permMode: this.#params.permMode,
            toggles: this.toggles,
        });
    });

    activeScenarioObject = $derived(getScenario(this.activeScenarioKey));

    activeAnalyticalOption = $derived.by((): ScenarioAnalyticalOption | null => {
        const scenario = this.activeScenarioObject;
        const options = scenario?.analyticalOptions ?? [];
        if (options.length === 0) return null;
        const selected = options.find((option) => option.key === this.activeAnalyticalOptionKey);
        if (selected) return selected;
        return options.find((option) => option.default) ?? options[0] ?? null;
    });

    // ===== $derived: Scenario → Chart Adapter =====

    /** BenchmarkFamily-shaped adapter for the active scenario (chart layer use only). */
    activeScenarioAsFamily = $derived.by((): BenchmarkFamily | null => {
        const sc = this.activeScenarioObject;
        if (!sc || this.isCustomMode) return null;
        const resolved = resolveCapabilities(sc.capabilities);
        const activeDimension = sc.sensitivities.find((d) => d.key === this.activeSensitivityDimensionKey) ?? null;
        const chartLayout = getScenarioChartLayout(sc, this.activeSensitivityDimensionKey);
        const xAxis = resolved.analyticalNativeXAxis as BenchmarkFamily['displayDefaults']['xAxis'];
        const panels = (resolved.primaryRateCurve === 'oil-rate'
            ? ['oil-rate', 'cumulative-oil', 'decline-diagnostics']
            : ['watercut-breakthrough', 'recovery', 'pressure']
        ) as BenchmarkFamily['displayDefaults']['panels'][number][];
        return {
            key: sc.key,
            baseCaseKey: sc.key,
            analyticalMethod: resolved.analyticalMethod,
            sensitivityAxes: [],
            reference: {
                kind: 'analytical' as const,
                source: resolved.analyticalMethod === 'digitized-reference'
                    ? `${sc.key}:digitized-reference`
                    : `${sc.key}:analytical`,
            },
            displayDefaults: { xAxis, panels },
            stylePolicy: {
                colorBy: 'case' as const,
                lineStyleBy: 'quantity-or-reference' as const,
                separatePressurePanel: true,
            },
            runPolicy: 'compare-to-reference' as const,
            label: sc.label,
            description: sc.description,
            baseCase: { key: sc.key, label: sc.label, description: sc.description, params: sc.params },
            suppressPrimaryAnalyticalOverlays: suppressesPrimaryAnalyticalOverlays(chartLayout),
            showSweepPanel: resolved.showSweepPanel,
            sweepGeometry: resolved.sweepGeometry,
            sweepAnalyticalMethod: this.activeAnalyticalOption?.sweepMethod,
            analyticalOverlayMode: activeDimension?.analyticalOverlayMode ?? 'auto',
            publishedReferenceSeries: sc.publishedReferenceSeries,
        } as BenchmarkFamily;
    });

    /** Chart-facing family: scenario adapter when in scenario mode, otherwise the library benchmark family. */
    activeChartFamily = $derived(this.activeScenarioAsFamily ?? this.activeReferenceFamily);

    // ===== $derived: Sweep / Analytical Config =====

    showSweepPanel = $derived(this.activeScenarioObject?.capabilities.showSweepPanel ?? false);

    sweepGeometry = $derived.by((): SweepGeometry => {
        return this.activeChartFamily?.sweepGeometry
            ?? this.activeScenarioObject?.capabilities.sweepGeometry
            ?? 'both';
    });

    sweepAnalyticalMethod = $derived.by((): SweepAnalyticalMethod => {
        return this.activeAnalyticalOption?.sweepMethod
            ?? (this.activeChartFamily as BenchmarkFamily | null)?.sweepAnalyticalMethod
            ?? 'dykstra-parsons';
    });

    analyticalPerVariant = $derived.by((): boolean => {
        const sc = this.activeScenarioObject;
        if (!sc) return false;
        const activeDim = sc.sensitivities.find((d) => d.key === this.activeSensitivityDimensionKey);
        if (!activeDim) return false;
        if (activeDim.analyticalOverlayMode === 'per-result') return true;
        if (activeDim.analyticalOverlayMode === 'shared') return false;
        return activeDim.variants
            .filter((v) => this.activeVariantKeys.includes(v.key))
            .some((v) => v.affectsAnalytical);
    });

    previewVariantParams = $derived.by(() => {
        if (!this.analyticalPerVariant) return undefined;
        const sc = this.activeScenarioObject;
        if (!sc) return undefined;
        const dim = sc.sensitivities.find((d) => d.key === this.activeSensitivityDimensionKey);
        if (!dim) return undefined;
        if (this.activeVariantKeys.length === 0) return undefined;
        return this.activeVariantKeys.flatMap((vk) => {
            const variant = dim.variants.find((v) => v.key === vk);
            if (!variant) return [];
            return [{ label: variant.label, variantKey: vk, params: getScenarioWithVariantParams(sc.key, dim.key, vk) }];
        });
    });

    // ===== $derived: Reference Results / Comparison =====

    activeReferenceResults = $derived.by(() => {
        const familyKey = this.activeChartFamily?.key ?? null;
        if (!familyKey) return this.#runtime.referenceRunResults.slice(0, 0); // typed empty
        return this.#runtime.referenceRunResults.filter((result) => result.familyKey === familyKey);
    });

    activeReferenceBaseResult = $derived.by(() => {
        return this.activeReferenceResults.find((result) => result.variantKey === null) ?? null;
    });

    activePrimaryComparisonResultKey = $derived.by((): string | null => {
        const primaryResultKey = this.activeComparisonSelection.primaryResultKey;
        if (!primaryResultKey) return null;
        return this.activeReferenceResults.some((result) => result.key === primaryResultKey)
            ? primaryResultKey
            : null;
    });

    activeSelectedReferenceResult = $derived.by(() => {
        if (!this.activePrimaryComparisonResultKey) return null;
        return this.activeReferenceResults.find(
            (result) => result.key === this.activePrimaryComparisonResultKey,
        ) ?? null;
    });

    pendingPreviewVariants = $derived.by(() => {
        if (!this.previewVariantParams?.length) return undefined;
        if (this.activeReferenceResults.length === 0) return undefined;
        const completedVariantKeys = new Set(
            this.activeReferenceResults.map((r) => r.variantKey).filter(Boolean),
        );
        const pending = this.previewVariantParams.filter((v) => !completedVariantKeys.has(v.variantKey));
        return pending.length > 0 ? pending : undefined;
    });

    // ===== $derived: Chart Layout =====

    activeRateChartLayoutConfig = $derived.by(() => {
        if (this.activeScenarioObject) {
            return getScenarioChartLayout(this.activeScenarioObject, this.activeSensitivityDimensionKey);
        }
        if (this.activeChartFamily) {
            return getReferenceRateChartLayoutConfig({
                family: this.activeChartFamily,
                referencePolicy: this.activeReferenceBaseResult?.referencePolicy ?? null,
            });
        }
        return this.activeLibraryEntry?.layoutConfig ?? {};
    });

    // ===== $derived: Output Profiles (for child components) =====

    selectedOutputProfile = $derived.by((): OutputSelectionProfile => {
        const ref = this.activeSelectedReferenceResult;
        return {
            gridState: ref?.finalSnapshot?.grid ?? this.#runtime.gridStateRaw ?? null,
            nx: Number(ref?.params.nx ?? this.#params.nx),
            ny: Number(ref?.params.ny ?? this.#params.ny),
            nz: Number(ref?.params.nz ?? this.#params.nz),
            cellDx: Number(ref?.params.cellDx ?? this.#params.cellDx),
            cellDy: Number(ref?.params.cellDy ?? this.#params.cellDy),
            cellDz: Number(ref?.params.cellDz ?? this.#params.cellDz),
            simTime: ref?.finalSnapshot?.time
                ?? Number(ref?.rateHistory.at(-1)?.time ?? this.#runtime.simTime),
            injectionRate: Math.max(0, Number(ref?.rateHistory.at(-1)?.total_injection ?? this.#runtime.latestInjectionRate ?? 0)),
            scenarioMode: ref?.analyticalMethod === 'depletion' ? 'depletion' : this.#params.analyticalMode,
            sourceLabel: ref ? ref.label : 'Live runtime',
            producerJ: Number(ref?.params.producerJ ?? this.#params.producerJ),
            initialSaturation: Number(ref?.params.initialSaturation ?? this.#params.initialSaturation),
            rockProps: {
                s_wc: Number(ref?.params.s_wc ?? this.#params.s_wc),
                s_or: Number(ref?.params.s_or ?? this.#params.s_or),
                n_w: Number(ref?.params.n_w ?? this.#params.n_w),
                n_o: Number(ref?.params.n_o ?? this.#params.n_o),
                k_rw_max: Number(ref?.params.k_rw_max ?? this.#params.k_rw_max),
                k_ro_max: Number(ref?.params.k_ro_max ?? this.#params.k_ro_max),
            },
            fluidProps: {
                mu_w: Number(ref?.params.mu_w ?? this.#params.mu_w),
                mu_o: Number(ref?.params.mu_o ?? this.#params.mu_o),
            },
        };
    });

    selectedOutput3D = $derived.by((): Output3DSelection => {
        const ref = this.activeSelectedReferenceResult;
        const history = ref?.history ?? this.#runtime.history;
        const currentIndex = history.length === 0
            ? -1
            : Math.max(0, Math.min(this.#runtime.currentIndex, history.length - 1));
        const cellDzPerLayerRaw = ref?.params.cellDzPerLayer ?? this.#params.cellDzPerLayer;
        return {
            history,
            nx: Number(ref?.params.nx ?? this.#params.nx),
            ny: Number(ref?.params.ny ?? this.#params.ny),
            nz: Number(ref?.params.nz ?? this.#params.nz),
            cellDx: Number(ref?.params.cellDx ?? this.#params.cellDx),
            cellDy: Number(ref?.params.cellDy ?? this.#params.cellDy),
            cellDz: Number(ref?.params.cellDz ?? this.#params.cellDz),
            cellDzPerLayer: Array.isArray(cellDzPerLayerRaw)
                ? cellDzPerLayerRaw.map((v) => Number(v))
                : [],
            gridState: ref?.finalSnapshot?.grid ?? this.#runtime.gridStateRaw ?? null,
            wellState: ref?.finalSnapshot?.wells ?? this.#runtime.wellStateRaw ?? null,
            replayTime: currentIndex >= 0 && currentIndex < history.length
                ? history[currentIndex]?.time ?? null
                : ref?.finalSnapshot?.time ?? this.#runtime.replayTime,
            currentIndex,
            sourceLabel: ref ? ref.label : 'Live runtime',
        };
    });

    default3DProperty = $derived.by((): 'pressure' | 'saturation_water' | 'saturation_oil' | 'saturation_gas' | 'saturation_ternary' | null => {
        const ref = this.activeSelectedReferenceResult;
        if (ref) {
            const resultParams = ref.params ?? {};
            if (resultParams.injectedFluid === 'gas') return 'saturation_gas';
            if (ref.analyticalMethod !== 'depletion') return 'saturation_water';
            return null;
        }
        if (this.#params.injectedFluid === 'gas' && this.#params.threePhaseModeEnabled) {
            return 'saturation_gas';
        }
        const default3D = this.activeScenarioObject?.capabilities.default3DScalar;
        if (default3D) return default3D as 'pressure' | 'saturation_water' | 'saturation_oil' | 'saturation_gas' | 'saturation_ternary';
        return null;
    });

    // ===== $derived: Analytical Output =====

    liveAnalyticalOutput = $derived.by((): ScenarioAnalyticalOutput => {
        if (this.#runtime.rateHistory.length === 0) return EMPTY_ANALYTICAL_OUTPUT;
        const def = this.activeScenarioObject?.analyticalDef;
        if (!def) return EMPTY_ANALYTICAL_OUTPUT;
        const inputs = def.inputsFromParams(this.#params as unknown as Record<string, unknown>, this.#runtime.rateHistory);
        return def.fn(inputs);
    });

    sweepEfficiencySimSeries = $derived.by((): SimSweepPoint[] | null => {
        if (!this.showSweepPanel || this.#runtime.rateHistory.length === 0) return null;
        const points: SimSweepPoint[] = [{
            time: 0,
            eA: this.sweepGeometry === 'both' ? null : 0,
            eV: this.sweepGeometry === 'both' ? null : 0,
            eVol: 0,
            mobileOilRecovered: this.sweepGeometry === 'both' ? 0 : null,
        }];
        for (const p of this.#runtime.rateHistory) {
            if (!p.sweep) continue;
            points.push({
                time: p.time,
                eA: p.sweep.e_a ?? null,
                eV: p.sweep.e_v ?? null,
                eVol: p.sweep.e_vol,
                mobileOilRecovered: p.sweep.mobile_oil_recovered ?? null,
            });
        }
        return points.length > 1 ? points : null;
    });

    sweepRFAnalytical = $derived.by((): SweepRFResult | null => {
        if (!this.showSweepPanel) return null;
        const { rockProps, fluidProps } = this.selectedOutputProfile;
        if (!rockProps || !fluidProps) return null;
        const perms = this.#params.permMode === 'perLayer' && (this.#params.layerPermsX as number[]).length > 1
            ? (this.#params.layerPermsX as number[])
            : (this.#params.nz as number) > 1
                ? Array.from({ length: this.#params.nz as number }, () => this.#params.uniformPermX as number)
                : [this.#params.uniformPermX as number];
        return computeSweepRecoveryFactor(
            rockProps,
            fluidProps,
            perms,
            this.#params.cellDz as number,
            3.0,
            200,
            this.sweepGeometry,
            this.sweepAnalyticalMethod,
        );
    });

    // Navigation state delegation getters — flatten navigationState properties for direct access
    get activeFamily() { return this.navigationState.activeFamily; }
    get activeSource() { return this.navigationState.activeSource; }
    get activeLibraryCaseKey() { return this.navigationState.activeLibraryCaseKey; }
    get activeLibraryGroup() { return this.navigationState.activeLibraryGroup; }
    get sourceLabel() { return this.navigationState.sourceLabel; }
    get referenceSourceLabel() { return this.navigationState.referenceSourceLabel; }
    get provenanceSummary() { return this.navigationState.provenanceSummary; }
    get editabilityPolicy() { return this.navigationState.editabilityPolicy; }

    // ===== Internal Helpers =====

    resolveCustomSubCase(mode: CaseMode | string): { key: string; label: string } | null {
        const raw = String(mode ?? '').toLowerCase();
        const normalizedMode: CaseMode | null =
            raw === 'dep' || raw === 'depletion' ? 'dep'
                : raw === 'wf' || raw === 'waterflood' ? 'wf'
                    : raw === 'sim' || raw === 'simulation' ? 'sim'
                        : null;
        if (!normalizedMode) return null;
        return CUSTOM_SUBCASE_BY_MODE[normalizedMode] ?? null;
    }

    maybySwitchToCustomSubCaseOnReinit(): boolean {
        if (this.isModified || !this.activeCase || !this.baseCaseSignature) return false;

        const customSubCase = this.resolveCustomSubCase(this.activeMode);
        if (!customSubCase) return false;
        const nextSignature = this.#params.buildCaseSignature();
        if (nextSignature === this.baseCaseSignature) return false;
        this.activeCase = customSubCase.key;
        this.baseCaseSignature = nextSignature;
        return true;
    }

    resolveOwningModeForLibraryEntry(entryKey: string): CaseMode | null {
        const entry = getCaseLibraryEntry(entryKey);
        if (!entry) return null;

        if (entry.entryKind === 'preset') {
            return entry.activation.activeMode;
        }

        if (entry.family === 'waterflood') return 'wf';
        if (entry.family === 'scenario-builder') return 'sim';
        return 'dep';
    }

    restoreActiveReferenceBaseDisplay(): void {
        const family = this.activeReferenceFamily;
        if (!family) return;

        this.applyCaseParams(family.baseCase.params);

        const baseResult = this.#runtime.referenceRunResults.find(
            (result) => result.familyKey === family.key && result.variantKey === null,
        );
        if (baseResult) {
            this.#runtime.hydrateRuntimeFromReferenceResult(baseResult);
        }
    }

    // ===== Case Params Application =====

    /**
     * Full case params application: sets param values, resets runtime display state,
     * and marks the model for reinit.
     */
    applyCaseParams(params: Record<string, any>) {
        this.#params.applyParamValues(params);
        this.#runtime.resetModelAndVisualizationState(true, false);
        this.#runtime.modelNeedsReinit = true;
        this.#runtime.modelReinitNotice = '';
    }

    // ===== Case Navigation =====

    handleModeChange(mode: CaseMode) {
        if (this.#runtime.referenceSweepRunning || this.#runtime.activeReferenceRunSpec) {
            this.#runtime.runtimeWarning = 'Stop reference runs before switching families.';
            return;
        }

        this.isModified = false;
        this.referenceProvenance = null;
        this.activeMode = mode;
        this.toggles = getDefaultToggles(mode);
        this.explicitLibraryEntryKey = null;
        this.activeComparisonSelection = buildComparisonSelection();
        this.baseCaseSignature = '';
        this.#runtime.clearReferenceRunnerState(true);

        this.handleToggleChange();
    }

    handleToggleChange(dimKey?: string, value?: string) {
        if (this.#runtime.referenceSweepRunning || this.#runtime.activeReferenceRunSpec) {
            this.#runtime.runtimeWarning = 'Stop reference runs before changing the active case.';
            return;
        }

        const nextToggles = { ...this.toggles };
        if (dimKey && value) {
            nextToggles[dimKey] = value;
        }
        this.toggles = stabilizeToggleState(nextToggles);

        const newKey = buildCaseKey(this.toggles);
        this.activeCase = newKey;
        this.explicitLibraryEntryKey = null;
        this.isModified = false;
        this.referenceProvenance = null;
        this.activeComparisonSelection = buildComparisonSelection();
        this.#runtime.clearReferenceRunnerState(true);
        this.#params.clearRuntimeOverrides();

        this.applyCaseParams(composeCaseParams(this.toggles));
        this.baseCaseSignature = this.#params.buildCaseSignature();
    }

    handleParamEdit() {
        if (this.isModified) return;
        this.isModified = true;
        this.baseCaseSignature = '';
    }

    activateLibraryEntry(entryKey: string): boolean {
        const entry = getCaseLibraryEntry(entryKey);
        if (!entry) {
            this.#runtime.runtimeError = 'Selected library case could not be resolved.';
            return false;
        }
        if (this.#runtime.referenceSweepRunning || this.#runtime.activeReferenceRunSpec) {
            this.#runtime.runtimeWarning = 'Stop reference runs before changing the active library case.';
            return false;
        }

        const nextMode = this.resolveOwningModeForLibraryEntry(entryKey);
        if (!nextMode) {
            this.#runtime.runtimeError = 'Selected library case could not be mapped to a scenario mode.';
            return false;
        }

        this.isModified = false;
        this.referenceProvenance = null;
        this.activeMode = nextMode;
        this.toggles = getDefaultToggles(nextMode);
        this.explicitLibraryEntryKey = entry.key;
        this.activeCase = entry.key;
        this.activeComparisonSelection = buildComparisonSelection();
        this.baseCaseSignature = '';
        this.#runtime.clearReferenceRunnerState(true);
        this.#params.clearRuntimeOverrides();

        this.applyCaseParams(entry.params);
        this.baseCaseSignature = this.#params.buildCaseSignature();
        return true;
    }

    cloneActiveReferenceToCustom(): boolean {
        if (!shouldAllowReferenceClone({
            isModified: this.isModified,
            hasReferenceLibraryCase: Boolean(this.activeNavigationLibraryEntry),
        })) return false;

        const benchmarkId = this.activeReferenceFamily?.key ?? this.toggles.benchmarkId ?? null;
        const benchmarkLabel = this.activeNavigationLibraryEntry?.label
            ?? (benchmarkId ? getBenchmarkEntry(benchmarkId)?.label ?? null : null);
        const provenance = buildReferenceCloneProvenance({
            benchmarkId,
            sourceCaseKey: this.activeNavigationLibraryEntry?.key ?? this.activeCase,
            sourceLabel: benchmarkLabel,
        });

        this.handleParamEdit();
        if (provenance && !this.referenceProvenance) {
            this.referenceProvenance = provenance;
        }

        return true;
    }

    setReferenceProvenance(provenance: ReferenceProvenance | null) {
        this.referenceProvenance = provenance;
    }

    // ===== Scenario-Picker Actions =====

    selectScenario(key: string) {
        const scenario = getScenario(key);
        if (!scenario) return;
        if (this.#runtime.referenceSweepRunning || this.#runtime.activeReferenceRunSpec) return;

        this.activeScenarioKey = key;
        this.isCustomMode = false;
        this.isModified = false;
        this.referenceProvenance = null;
        this.activeComparisonSelection = buildComparisonSelection();
        this.#runtime.clearReferenceRunnerState(true);

        // Initialise sensitivity dimension and pre-select enabled variants.
        const defaultDimKey = scenario.defaultSensitivityDimensionKey ?? scenario.sensitivities[0]?.key ?? null;
        this.activeSensitivityDimensionKey = defaultDimKey;
        const defaultDim = scenario.sensitivities.find((d) => d.key === defaultDimKey) ?? null;
        this.activeVariantKeys = defaultDim ? getDefaultVariantKeys(defaultDim) : [];
        this.activeAnalyticalOptionKey = scenario.analyticalOptions?.find((option) => option.default)?.key
            ?? scenario.analyticalOptions?.[0]?.key
            ?? null;

        // Derive CaseMode from scenario capabilities.
        const nextMode: CaseMode = scenario.capabilities.requiresThreePhaseMode ? '3p'
            : scenario.capabilities.analyticalMethod === 'buckley-leverett' ? 'wf' : 'dep';
        this.activeMode = nextMode;
        this.toggles = getDefaultToggles(nextMode);
        this.explicitLibraryEntryKey = null;
        this.activeCase = key;
        this.#params.clearRuntimeOverrides();
        this.#params.analyticalMode = getDefaultScenarioAnalyticalMode(scenario.capabilities);

        this.applyCaseParams(scenario.params);
        this.baseCaseSignature = this.#params.buildCaseSignature();
    }

    /**
     * Switch the active sensitivity dimension for the current scenario.
     * Resets activeVariantKeys to the new dimension's default-enabled variants.
     */
    selectSensitivityDimension(dimensionKey: string) {
        const scenario = this.activeScenarioObject;
        if (!scenario) return;
        if (this.#runtime.referenceSweepRunning || this.#runtime.activeReferenceRunSpec) return;
        const dimension = scenario.sensitivities.find((d) => d.key === dimensionKey);
        if (!dimension) {
            if (import.meta.env.DEV) {
                console.warn(`[store] selectSensitivityDimension: unknown key "${dimensionKey}" for scenario "${scenario.key}"`);
            }
            return;
        }
        if (dimensionKey === this.activeSensitivityDimensionKey) return;

        this.activeComparisonSelection = buildComparisonSelection();
        this.#runtime.clearReferenceRunnerState(true);
        this.activeSensitivityDimensionKey = dimensionKey;
        this.activeVariantKeys = getDefaultVariantKeys(dimension);
    }

    selectAnalyticalOption(optionKey: string) {
        const scenario = this.activeScenarioObject;
        if (!scenario) return;
        if (this.#runtime.referenceSweepRunning || this.#runtime.activeReferenceRunSpec) return;
        if (!(scenario.analyticalOptions ?? []).some((option) => option.key === optionKey)) return;
        if (optionKey === this.activeAnalyticalOptionKey) return;

        this.activeComparisonSelection = buildComparisonSelection();
        this.#runtime.clearReferenceRunnerState(true);
        this.activeAnalyticalOptionKey = optionKey;
    }

    toggleScenarioVariant(variantKey: string) {
        if (this.#runtime.referenceSweepRunning || this.#runtime.activeReferenceRunSpec) return;

        this.activeComparisonSelection = buildComparisonSelection();
        this.#runtime.clearReferenceRunnerState(true);
        this.activeVariantKeys = this.activeVariantKeys.includes(variantKey)
            ? this.activeVariantKeys.filter((k) => k !== variantKey)
            : [...this.activeVariantKeys, variantKey];
    }

    enterCustomMode() {
        this.isCustomMode = true;
        this.activeAnalyticalOptionKey = null;
        this.handleParamEdit();
    }

    resetOverrideGroupsToBase(groupKeys: string[]): { resetCount: number } {
        if (!Array.isArray(groupKeys) || groupKeys.length === 0) {
            return { resetCount: 0 };
        }

        const resetPlan = buildOverrideResetPlan({
            groupKeys,
            groupedOverrides: this.parameterOverrideGroups,
            overrides: this.parameterOverrides,
        });

        for (const item of resetPlan) {
            const nextValue = Array.isArray(item.base) ? [...item.base] : item.base;
            (this.#params as unknown as Record<string, unknown>)[item.key] = nextValue;
        }

        return { resetCount: resetPlan.length };
    }

    setComparisonSelection(selection: Partial<ComparisonSelection>) {
        this.activeComparisonSelection = buildComparisonSelection(selection);
    }

    // ===== activeScenarioChartLayout helper =====
    // (used by App.svelte to get the rate chart layout config for a scenario)
    getActiveScenarioChartLayout(dimensionKey: string | null) {
        const sc = this.activeScenarioObject;
        if (!sc) return null;
        return getScenarioChartLayout(sc, dimensionKey);
    }
}

// ---------- Factory ----------

export function createNavigationStore(params: ParameterStore, runtime: RuntimeStore) {
    return new NavigationStoreImpl(params, runtime);
}

export type NavigationStore = InstanceType<typeof NavigationStoreImpl>;
