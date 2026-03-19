<script lang="ts">
    import { onMount, onDestroy, tick } from "svelte";
    import FractionalFlow from "./lib/analytical/FractionalFlow.svelte";
    import DepletionAnalytical from "./lib/analytical/DepletionAnalytical.svelte";
    import ReferenceResultsCard from "./lib/ui/cards/ReferenceResultsCard.svelte";
    import ReferenceExecutionCard from "./lib/ui/cards/ReferenceExecutionCard.svelte";
    import RunControls from "./lib/ui/cards/RunControls.svelte";
    import WarningPolicyPanel from "./lib/ui/feedback/WarningPolicyPanel.svelte";
    import ScenarioPicker from "./lib/ui/modes/ScenarioPicker.svelte";
    import { getReferenceRateChartLayoutConfig } from "./lib/charts/referenceChartConfig";
    import { getChartPreset, getScenarioWithVariantParams } from "./lib/catalog/scenarios";
    import { computeSimSweepPoint, computeSweepRecoveryFactor, type SweepRFResult } from "./lib/analytical/sweepEfficiency";
    import Button from "./lib/ui/controls/Button.svelte";
    import Card from "./lib/ui/controls/Card.svelte";
    import { createSimulationStore } from "./lib/stores/simulationStore.svelte";

    // ---------- Store ----------
    const store = createSimulationStore();
    const scenario = store.scenarioSelection;
    const params = store.parameterState;
    const runtime = store.runtimeState;

    // ---------- UI-only state ----------
    let theme: "dark" | "light" = $state("light");
    let showDebugState = $state(false);
    let showProperty: "pressure" | "saturation_water" | "saturation_oil" =
        $state("pressure");
    let legendFixedMin = $state(0);
    let legendFixedMax = $state(1);

    type ThreeDViewComponentType = typeof import("./lib/visualization/3dview.svelte").default;
    type RateChartComponentType =
        typeof import("./lib/charts/RateChart.svelte").default;
    type ReferenceComparisonChartComponentType =
        typeof import("./lib/charts/ReferenceComparisonChart.svelte").default;
    let ThreeDViewComponent = $state<ThreeDViewComponentType | null>(null);
    let RateChartComponent = $state<RateChartComponentType | null>(null);
    let ReferenceComparisonChartComponent = $state<ReferenceComparisonChartComponentType | null>(null);
    let loadingThreeDView = $state(false);
    const activeReferenceFamily = $derived(scenario.activeScenarioAsFamily ?? scenario.activeReferenceFamily);
    // True only for sweep-domain scenarios — hides the sweep efficiency panel for 1D waterflood/depletion.
    const showSweepPanel = $derived(scenario.activeScenarioObject?.domain === 'sweep');

    // Simulation sweep efficiency time series — computed from per-cell saturation snapshots.
    // Only populated for sweep-domain scenarios; null otherwise.
    type SimSweepPoint = { time: number; eA: number; eV: number; eVol: number };
    const sweepEfficiencySimSeries = $derived.by((): SimSweepPoint[] | null => {
        if (!showSweepPanel || runtime.history.length === 0) return null;
        const { nx, ny, nz, s_wc } = params;
        const result: SimSweepPoint[] = [];
        for (const entry of runtime.history) {
            const sw = entry.grid?.sat_water;
            if (!sw || sw.length !== nx * ny * nz) continue;
            const { eA, eV, eVol } = computeSimSweepPoint(sw, nx, ny, nz, s_wc);
            result.push({ time: entry.time, eA, eV, eVol });
        }
        return result.length > 0 ? result : null;
    });

    // Analytical recovery factor for sweep scenarios: RF = E_vol(Craig+DP) × E_D_BL(PVI_local).
    // Computed purely from rock/fluid props — no simulation data required.
    const sweepRFAnalytical = $derived.by((): SweepRFResult | null => {
        if (!showSweepPanel || !outputProfileRockProps || !outputProfileFluidProps) return null;
        const perms = params.permMode === 'perLayer' && params.layerPermsX.length > 1
            ? params.layerPermsX
            : params.nz > 1
                ? Array.from({ length: params.nz }, () => params.uniformPermX)
                : [params.uniformPermX];
        return computeSweepRecoveryFactor(outputProfileRockProps, outputProfileFluidProps, perms, params.cellDz);
    });

    // True when any active sensitivity variant is declared to affect the analytical solution.
    // Drives per-result vs shared analytical curve rendering in the comparison chart.
    const analyticalPerVariant = $derived.by(() => {
        const sc = scenario.activeScenarioObject;
        if (!sc) return false;
        const activeDim = sc.sensitivities.find(
            (d) => d.key === scenario.activeSensitivityDimensionKey,
        );
        if (!activeDim) return false;
        return activeDim.variants
            .filter((v) => scenario.activeVariantKeys.includes(v.key))
            .some((v) => v.affectsAnalytical);
    });

    // When analyticalPerVariant is true, build one preview entry per selected
    // variant so the comparison chart can show all analytical curves before any
    // simulation runs complete. Each entry carries the full merged params so the
    // chart can compute the BL / depletion analytical solution independently.
    const previewVariantParams = $derived.by(() => {
        if (!analyticalPerVariant) return undefined;
        const sc = scenario.activeScenarioObject;
        if (!sc) return undefined;
        const dim = sc.sensitivities.find(
            (d) => d.key === scenario.activeSensitivityDimensionKey,
        );
        if (!dim) return undefined;
        if (scenario.activeVariantKeys.length === 0) return undefined;
        return scenario.activeVariantKeys.flatMap((vk) => {
            const variant = dim.variants.find((v) => v.key === vk);
            if (!variant) return [];
            return [{
                label: variant.label,
                variantKey: vk,
                params: getScenarioWithVariantParams(sc.key, dim.key, vk),
            }];
        });
    });
    const activeReferenceResults = $derived.by(() => {
        const familyKey = activeReferenceFamily?.key ?? null;
        if (!familyKey) return [];
        return runtime.referenceRunResults.filter((result) => result.familyKey === familyKey);
    });
    const activeComparisonSelection = $derived(scenario.activeComparisonSelection);
    const activeReferenceBaseResult = $derived.by(() => {
        return activeReferenceResults.find((result) => result.variantKey === null) ?? null;
    });
    const activePrimaryComparisonResultKey = $derived.by(() => {
        const primaryResultKey = activeComparisonSelection.primaryResultKey;
        if (!primaryResultKey) return null;
        return activeReferenceResults.some((result) => result.key === primaryResultKey)
            ? primaryResultKey
            : null;
    });
    const activeSelectedReferenceResult = $derived.by(() => {
        if (!activePrimaryComparisonResultKey) return null;
        return activeReferenceResults.find((result) => result.key === activePrimaryComparisonResultKey) ?? null;
    });
    const outputProfileGridState = $derived.by(() => (
        activeSelectedReferenceResult?.finalSnapshot?.grid ?? runtime.gridStateRaw ?? null
    ));
    const outputProfileNx = $derived.by(() => (
        Number(activeSelectedReferenceResult?.params.nx ?? params.nx)
    ));
    const outputProfileNy = $derived.by(() => (
        Number(activeSelectedReferenceResult?.params.ny ?? params.ny)
    ));
    const outputProfileNz = $derived.by(() => (
        Number(activeSelectedReferenceResult?.params.nz ?? params.nz)
    ));
    const outputProfileCellDx = $derived.by(() => (
        Number(activeSelectedReferenceResult?.params.cellDx ?? params.cellDx)
    ));
    const outputProfileCellDy = $derived.by(() => (
        Number(activeSelectedReferenceResult?.params.cellDy ?? params.cellDy)
    ));
    const outputProfileCellDz = $derived.by(() => (
        Number(activeSelectedReferenceResult?.params.cellDz ?? params.cellDz)
    ));
    const outputProfileSimTime = $derived.by(() => (
        activeSelectedReferenceResult?.finalSnapshot?.time
        ?? Number(activeSelectedReferenceResult?.rateHistory.at(-1)?.time ?? runtime.simTime)
    ));
    const outputProfileInjectionRate = $derived.by(() => (
        Math.max(0, Number(activeSelectedReferenceResult?.rateHistory.at(-1)?.total_injection ?? runtime.latestInjectionRate ?? 0))
    ));
    const outputProfileScenarioMode = $derived.by(() => (
        activeSelectedReferenceResult?.scenarioClass === "depletion" ? "depletion" : params.analyticalSolutionMode
    ));
    const outputProfileSourceLabel = $derived.by(() => (
        activeSelectedReferenceResult ? activeSelectedReferenceResult.label : "Live runtime"
    ));
    const outputProfileProducerJ = $derived.by(() => (
        Number(activeSelectedReferenceResult?.params.producerJ ?? params.producerJ)
    ));
    const outputProfileInitialSaturation = $derived.by(() => (
        Number(activeSelectedReferenceResult?.params.initialSaturation ?? params.initialSaturation)
    ));
    const outputProfileRockProps = $derived.by(() => ({
        s_wc: Number(activeSelectedReferenceResult?.params.s_wc ?? params.s_wc),
        s_or: Number(activeSelectedReferenceResult?.params.s_or ?? params.s_or),
        n_w: Number(activeSelectedReferenceResult?.params.n_w ?? params.n_w),
        n_o: Number(activeSelectedReferenceResult?.params.n_o ?? params.n_o),
    }));
    const outputProfileFluidProps = $derived.by(() => ({
        mu_w: Number(activeSelectedReferenceResult?.params.mu_w ?? params.mu_w),
        mu_o: Number(activeSelectedReferenceResult?.params.mu_o ?? params.mu_o),
    }));
    const output3DHistory = $derived.by(() => (
        activeSelectedReferenceResult?.history ?? runtime.history
    ));
    const output3DNx = $derived.by(() => (
        Number(activeSelectedReferenceResult?.params.nx ?? params.nx)
    ));
    const output3DNy = $derived.by(() => (
        Number(activeSelectedReferenceResult?.params.ny ?? params.ny)
    ));
    const output3DNz = $derived.by(() => (
        Number(activeSelectedReferenceResult?.params.nz ?? params.nz)
    ));
    const output3DCellDx = $derived.by(() => (
        Number(activeSelectedReferenceResult?.params.cellDx ?? params.cellDx)
    ));
    const output3DCellDy = $derived.by(() => (
        Number(activeSelectedReferenceResult?.params.cellDy ?? params.cellDy)
    ));
    const output3DCellDz = $derived.by(() => (
        Number(activeSelectedReferenceResult?.params.cellDz ?? params.cellDz)
    ));
    const output3DCurrentIndex = $derived.by(() => {
        if (output3DHistory.length === 0) return -1;
        return Math.max(0, Math.min(runtime.currentIndex, output3DHistory.length - 1));
    });
    const output3DGridState = $derived.by(() => (
        activeSelectedReferenceResult?.finalSnapshot?.grid ?? runtime.gridStateRaw ?? null
    ));
    const output3DWellState = $derived.by(() => (
        activeSelectedReferenceResult?.finalSnapshot?.wells ?? runtime.wellStateRaw ?? null
    ));
    const output3DReplayTime = $derived.by(() => {
        if (
            output3DHistory.length > 0
            && output3DCurrentIndex >= 0
            && output3DCurrentIndex < output3DHistory.length
        ) {
            return output3DHistory[output3DCurrentIndex]?.time ?? null;
        }

        return activeSelectedReferenceResult?.finalSnapshot?.time ?? runtime.replayTime;
    });
    const output3DSourceLabel = $derived.by(() => (
        activeSelectedReferenceResult ? activeSelectedReferenceResult.label : "Live runtime"
    ));
    const activeRunManifest = $derived.by(() => ({
        referencePolicySummary: scenario.activeLibraryEntry?.referencePolicySummary ?? null,
        referenceProvenance: scenario.referenceProvenance,
        referenceFamily: scenario.activeReferenceFamily ?? null,
    }));

    const activeRateChartLayoutConfig = $derived.by(() => {
        if (scenario.activeScenarioObject) {
            // If the active sensitivity dimension has a chart preset override, use it.
            const activeDim = scenario.activeScenarioObject.sensitivities.find(
                (d) => d.key === scenario.activeSensitivityDimensionKey,
            );
            const presetKey = activeDim?.chartPresetOverride ?? scenario.activeScenarioObject.chartPreset;
            return getChartPreset(presetKey);
        }
        if (activeReferenceFamily) {
            return getReferenceRateChartLayoutConfig({
                family: activeReferenceFamily,
                referencePolicy: activeReferenceBaseResult?.referencePolicy ?? null,
            });
        }
        return scenario.activeLibraryEntry?.layoutConfig ?? {};
    });

    function handleRun() {
        const scenarioKey = scenario.activeScenarioKey;
        const dimensionKey = scenario.activeSensitivityDimensionKey;
        if (scenarioKey && dimensionKey && !scenario.isCustomMode && scenario.activeVariantKeys.length > 0) {
            runtime.runScenarioSweep(scenarioKey, dimensionKey, scenario.activeVariantKeys);
        } else {
            runtime.runSteps();
        }
    }

    function handleSelectComparisonResult(resultKey: string) {
        scenario.setComparisonSelection({
            primaryResultKey: resultKey,
            comparedResultKeys: [],
        });
    }

    function clearComparisonSelection() {
        scenario.setComparisonSelection({
            primaryResultKey: null,
            comparedResultKeys: [],
        });
    }

    function handleApplyOutputHistoryIndex(index: number) {
        if (activeSelectedReferenceResult) {
            runtime.currentIndex = index;
            return;
        }

        runtime.applyHistoryIndex(index);
    }

    $effect(() => {
        const hasActiveResults = activeReferenceResults.length > 0;
        if (!hasActiveResults) {
            if (activeComparisonSelection.primaryResultKey || activeComparisonSelection.comparedResultKeys.length > 0) {
                clearComparisonSelection();
            }
            return;
        }

        if (!activeComparisonSelection.primaryResultKey) return;
        if (activePrimaryComparisonResultKey) return;

        clearComparisonSelection();
    });


    // ---------- Config diff $effect ----------
    $effect(() => {
        runtime.checkConfigDiff();
    });

    // ---------- Theme ----------
    function toggleTheme() {
        theme = theme === "dark" ? "light" : "dark";
    }

    $effect(() => {
        if (typeof document === "undefined") return;
        document.documentElement.setAttribute("data-theme", theme);
    });
    $effect(() => {
        if (typeof localStorage === "undefined") return;
        localStorage.setItem("ressim-theme", theme);
    });

    // ---------- Lazy module loading ----------
    async function loadRateChartModule() {
        try {
            const rateChartModule = await import("./lib/charts/RateChart.svelte");
            RateChartComponent = rateChartModule.default;
        } catch (error) {
            console.error("Failed to load rate chart module:", error);
        }
    }

    async function loadReferenceComparisonChartModule() {
        try {
            const comparisonChartModule = await import("./lib/charts/ReferenceComparisonChart.svelte");
            ReferenceComparisonChartComponent = comparisonChartModule.default;
        } catch (error) {
            console.error("Failed to load comparison chart module:", error);
        }
    }

    async function loadThreeDViewModule() {
        if (ThreeDViewComponent || loadingThreeDView) return;
        loadingThreeDView = true;
        try {
            const threeDModule = await import("./lib/visualization/3dview.svelte");
            ThreeDViewComponent = threeDModule.default;
        } catch (error) {
            console.error("Failed to load 3D view module:", error);
        } finally {
            loadingThreeDView = false;
        }
    }

    // ---------- Lifecycle ----------
    onMount(async () => {
        const savedTheme = localStorage.getItem("ressim-theme");
        if (savedTheme === "light" || savedTheme === "dark") theme = savedTheme;
        document.documentElement.setAttribute("data-theme", theme);
        runtime.setupWorker();

        await loadRateChartModule();
        await loadReferenceComparisonChartModule();
        await loadThreeDViewModule();
        await tick();

        scenario.selectScenario("wf_bl1d");
    });

    onDestroy(() => {
        runtime.dispose();
    });
</script>

<main
    class="min-h-screen text-foreground bg-background relative"
    data-theme={theme}
>
    

    <!-- Main Content — z-[2] ensures it renders above both layers and gradient overlay -->
    <div class="mx-auto w-full space-y-4 p-4 lg:p-6 2xl:px-8 relative z-2">
        <!-- Hidden component for analytical calculations -->
        <FractionalFlow
            rockProps={{
                s_wc: params.s_wc,
                s_or: params.s_or,
                n_w: params.n_w,
                n_o: params.n_o,
                k_rw_max: params.k_rw_max,
                k_ro_max: params.k_ro_max,
            }}
            fluidProps={{ mu_w: params.mu_w, mu_o: params.mu_o }}
            initialSaturation={params.initialSaturation}
            timeHistory={runtime.rateHistory.map((point) => point.time)}
            injectionRateSeries={runtime.rateHistory.map((point) =>
                Number(point.total_injection ?? 0),
            )}
            reservoir={{
                length: params.nx * params.cellDx,
                area: params.ny * params.cellDy * params.nz * params.cellDz,
                porosity: params.reservoirPorosity,
            }}
            scenarioMode={params.analyticalSolutionMode}
            onAnalyticalData={(detail) => {
                if (params.analyticalSolutionMode === "waterflood") {
                    runtime.analyticalProductionData = detail.production;
                }
            }}
            onAnalyticalMeta={(detail) => {
                if (params.analyticalSolutionMode === "waterflood") {
                    runtime.analyticalMeta = detail;
                }
            }}
        />

        <DepletionAnalytical
            enabled={params.analyticalSolutionMode === "depletion"}
            timeHistory={runtime.rateHistory.map((point) => point.time)}
            reservoir={{
                length: params.nx * params.cellDx,
                area: params.ny * params.cellDy * params.nz * params.cellDz,
                porosity: params.reservoirPorosity,
            }}
            initialSaturation={params.initialSaturation}
            nz={params.nz}
            permMode={params.permMode}
            uniformPermX={params.uniformPermX}
            uniformPermY={params.uniformPermY}
            layerPermsX={params.layerPermsX}
            layerPermsY={params.layerPermsY}
            cellDx={params.cellDx}
            cellDy={params.cellDy}
            cellDz={params.cellDz}
            wellRadius={params.well_radius}
            wellSkin={params.well_skin}
            muO={params.mu_o}
            sWc={params.s_wc}
            sOr={params.s_or}
            nO={params.n_o}
            c_o={params.c_o}
            c_w={params.c_w}
            cRock={params.rock_compressibility}
            initialPressure={params.initialPressure}
            producerBhp={params.producerBhp}
            depletionRateScale={params.analyticalDepletionRateScale}
            onAnalyticalData={(detail) => {
                if (params.analyticalSolutionMode === "depletion") {
                    runtime.analyticalProductionData = detail.production;
                }
            }}
            onAnalyticalMeta={(detail) => {
                if (params.analyticalSolutionMode === "depletion") {
                    runtime.analyticalMeta = detail;
                }
            }}
        />

        <!-- Header -->
        <header
            class="flex flex-col gap-2 sm:flex-row sm:items-center sm:justify-between"
        >
            <div>
                <h1 class="text-2xl font-bold lg:text-3xl">
                    Simplified Reservoir Simulation Model
                </h1>
                <p class="text-sm opacity-80">
                    Interactive two-phase simulation with 3D visualisation fully
                    in browser.
                </p>
            </div>
            <Button size="sm" variant="outline" onclick={toggleTheme}>
                {theme === "dark" ? "☀ Light" : "🌙 Dark"}
            </Button>
        </header>

        <section class="space-y-2">
        <ScenarioPicker
            activeScenarioKey={scenario.activeScenarioKey}
            activeSensitivityDimensionKey={scenario.activeSensitivityDimensionKey}
            activeVariantKeys={scenario.activeVariantKeys}
            isCustom={scenario.isCustomMode}
            activeMode={scenario.activeMode}
            {params}
            toggles={scenario.toggles}
            disabledOptions={scenario.disabledOptions}
            validationErrors={params.validationErrors}
            warningPolicy={runtime.warningPolicy}
            basePreset={scenario.basePreset}
            navigationState={scenario.navigationState}
            referenceProvenance={scenario.referenceProvenance}
            referenceSweepRunning={runtime.referenceSweepRunning}
            onSelectScenario={(key) => scenario.selectScenario(key)}
            onSelectSensitivityDimension={(key) => scenario.selectSensitivityDimension(key)}
            onToggleVariant={(key) => scenario.toggleScenarioVariant(key)}
            onEnterCustomMode={() => scenario.enterCustomMode()}
            onCloneReferenceToCustom={() => scenario.cloneActiveReferenceToCustom()}
            onActivateLibraryEntry={(key) => scenario.activateLibraryEntry(key)}
            onToggleChange={(dimKey, value) => scenario.handleToggleChange(dimKey, value)}
            onParamEdit={() => scenario.handleParamEdit()}
        />
        </section>

        <section class="space-y-2">
        <RunControls
            wasmReady={runtime.wasmReady}
            workerRunning={runtime.workerRunning || runtime.referenceSweepRunning}
            runCompleted={runtime.runCompleted}
            simTime={runtime.simTime}
            historyLength={runtime.history.length}
            hasValidationErrors={params.hasValidationErrors}
            numSensitivities={!scenario.isCustomMode ? scenario.activeVariantKeys.length : 0}
            runProgress={runtime.referenceSweepRunning
                ? runtime.referenceSweepProgressLabel
                : runtime.workerRunning && runtime.currentRunTotalSteps > 0
                    ? `${runtime.currentRunStepsCompleted}/${runtime.currentRunTotalSteps} steps`
                    : ""}
            bind:steps={params.steps}
            stopPending={runtime.stopPending}
            onRunSteps={handleRun}
            onInitSimulator={() => runtime.initSimulator()}
            onStopRun={() => runtime.stopRun()}
            fieldErrors={params.validationErrors}
            warningPolicy={runtime.warningPolicy}
        />
        {#if runtime.referenceSweepError}
            <div class="rounded-md border border-destructive/40 bg-destructive/10 px-3 py-2 text-xs text-destructive">
                {runtime.referenceSweepError}
            </div>
        {/if}

        {#if scenario.activeReferenceFamily?.key}
            <div class="space-y-1">
                <div class="ui-section-kicker">Reference Run Status</div>
                <ReferenceExecutionCard
                    referenceFamilyKey={scenario.activeReferenceFamily?.key ?? null}
                    isModified={scenario.isModified}
                    referenceSweepRunning={runtime.referenceSweepRunning}
                    onRunReferenceSelection={(keys) => runtime.runActiveReferenceSelection(keys)}
                    onStopReferenceSweep={() => runtime.stopRun()}
                />
            </div>
        {/if}
        </section>

        <!-- Error / Warning banners -->
        {#if runtime.runtimeError}
            <div
                class="rounded-md border border-destructive bg-card text-destructive p-3 text-xs font-medium"
            >
                {runtime.runtimeError}
            </div>
        {/if}

        <div class="hidden">
            {#if params.analyticalSolutionMode === 'depletion'}
                <span>Depletion Reference Solution</span>
            {:else if params.analyticalSolutionMode === 'waterflood'}
                <span>Waterflood Reference Solution · Reference solution: Buckley-Leverett fractional flow</span>
            {/if}
        </div>

        <section class="space-y-2 mt-2">
            <div>
                <div class="ui-section-kicker">
                    Results
                </div>
            </div>

        <div class="grid grid-cols-1 gap-4 xl:grid-cols-2 xl:items-start">
            <div class="space-y-4">
                <ReferenceResultsCard
                    family={activeReferenceFamily}
                    results={activeReferenceResults}
                    selectedResultKey={activePrimaryComparisonResultKey}
                    onSelectResult={handleSelectComparisonResult}
                    onClearSelection={clearComparisonSelection}
                />

                <Card class="overflow-hidden">
                    {#if activeReferenceFamily && ReferenceComparisonChartComponent}
                        <ReferenceComparisonChartComponent
                            results={activeReferenceResults}
                            family={activeReferenceFamily}
                            layoutConfig={activeRateChartLayoutConfig}
                            {analyticalPerVariant}
                            {theme}
                            previewVariantParams={activeReferenceResults.length === 0 ? previewVariantParams : undefined}
                            previewBaseParams={activeReferenceResults.length === 0 && !previewVariantParams?.length ? (params as Record<string, any>) : undefined}
                            previewScenarioClass={activeReferenceFamily.scenarioClass}
                        />
                    {:else if RateChartComponent}
                        <RateChartComponent
                            rateHistory={runtime.rateHistory}
                            analyticalProductionData={runtime.analyticalProductionData}
                            avgReservoirPressureSeries={runtime.avgReservoirPressureSeries}
                            avgWaterSaturationSeries={runtime.avgWaterSaturationSeries}
                            ooipM3={params.ooipM3}
                            poreVolumeM3={params.poreVolumeM3}
                            activeMode={scenario.activeMode}
                            activeCase={scenario.activeCase}
                            {theme}
                            analyticalMeta={runtime.analyticalMeta}
                            layoutConfig={activeRateChartLayoutConfig}
                            rockProps={outputProfileRockProps}
                            fluidProps={outputProfileFluidProps}
                            layerPermeabilities={params.permMode === 'perLayer' && params.layerPermsX.length > 1
                                ? params.layerPermsX
                                : params.nz > 1
                                    ? Array.from({ length: params.nz }, () => params.uniformPermX)
                                    : [params.uniformPermX]}
                            layerThickness={params.cellDz}
                            {showSweepPanel}
                            {sweepEfficiencySimSeries}
                            {sweepRFAnalytical}
                        />
                    {:else}
                        <div
                            class="p-4 md:p-5 text-sm text-muted-foreground w-full text-center"
                        >
                            Loading output chart…
                        </div>
                    {/if}
                </Card>

                

                
            </div>

            <div class="space-y-4">
                <Card>
                    <div class="p-4 md:p-5">
                        {#if ThreeDViewComponent}
                            {#key `${output3DNx}-${output3DNy}-${output3DNz}-${runtime.vizRevision}-${activeSelectedReferenceResult?.key ?? "live"}`}
                                <ThreeDViewComponent
                                    nx={output3DNx}
                                    ny={output3DNy}
                                    nz={output3DNz}
                                    cellDx={output3DCellDx}
                                    cellDy={output3DCellDy}
                                    cellDz={output3DCellDz}
                                    {theme}
                                    sourceLabel={output3DSourceLabel}
                                    gridState={output3DGridState}
                                    bind:showProperty
                                    bind:legendFixedMin
                                    bind:legendFixedMax
                                    s_wc={outputProfileRockProps.s_wc}
                                    s_or={outputProfileRockProps.s_or}
                                    currentIndex={output3DCurrentIndex}
                                    replayTime={output3DReplayTime}
                                    onApplyHistoryIndex={handleApplyOutputHistoryIndex}
                                    history={output3DHistory}
                                    wellState={output3DWellState}
                                />
                            {/key}
                        {:else}
                            <div
                                class="flex items-center justify-center rounded border border-border bg-muted/20"
                                style="height: clamp(240px, 35vh, 420px);"
                            >
                                {#if loadingThreeDView}
                                    <div class="flex items-center space-x-2">
                                        <div
                                            class="h-4 w-4 animate-spin rounded-full border-b-2 border-primary"
                                        ></div>
                                        <span class="text-sm font-medium"
                                            >Loading 3D output...</span
                                        >
                                    </div>
                                {:else}
                                    <Button
                                        size="sm"
                                        variant="default"
                                        onclick={loadThreeDViewModule}
                                        >Open 3D View</Button
                                    >
                                {/if}
                            </div>
                        {/if}
                    </div>
                    <!-- <div class="p-4 md:p-5">
                        <SwProfileChart
                            gridState={outputProfileGridState}
                            nx={outputProfileNx}
                            ny={outputProfileNy}
                            nz={outputProfileNz}
                            cellDx={outputProfileCellDx}
                            cellDy={outputProfileCellDy}
                            cellDz={outputProfileCellDz}
                            simTime={outputProfileSimTime}
                            producerJ={outputProfileProducerJ}
                            initialSaturation={outputProfileInitialSaturation}
                            injectionRate={outputProfileInjectionRate}
                            scenarioMode={outputProfileScenarioMode}
                            sourceLabel={outputProfileSourceLabel}
                            rockProps={outputProfileRockProps}
                            fluidProps={outputProfileFluidProps}
                        />
                    </div> -->
                </Card>

            </div>
        </div>
        </section>

        <!-- Debug State -->
        {#if showDebugState}
            <Card class="mt-4">
                <div class="grid gap-4 p-4 lg:grid-cols-2">
                    <div>
                        <h4 class="mb-2 text-sm font-semibold">
                            Grid State (current)
                        </h4>
                        <pre
                            class="max-h-100 overflow-auto rounded border border-border bg-muted/20 p-2 text-xs">{JSON.stringify(
                                runtime.gridStateRaw,
                                null,
                                2,
                            )}</pre>
                    </div>
                    <div>
                        <h4 class="mb-2 text-sm font-semibold">
                            Well State (current)
                        </h4>
                        <pre
                            class="max-h-105 overflow-auto rounded border border-border bg-muted p-2 text-xs">{JSON.stringify(
                                runtime.wellStateRaw,
                                null,
                                2,
                            )}</pre>
                    </div>
                </div>
            </Card>
        {/if}
    </div>
</main>
