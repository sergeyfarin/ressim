<script lang="ts">
    import { onMount, onDestroy, tick } from "svelte";
    import ReferenceExecutionCard from "./lib/ui/cards/ReferenceExecutionCard.svelte";
    import RunControls from "./lib/ui/cards/RunControls.svelte";
    import ScenarioPicker from "./lib/ui/modes/ScenarioPicker.svelte";
    import ThreeDViewCard from "./lib/ui/cards/ThreeDViewCard.svelte";
    import Button from "./lib/ui/controls/Button.svelte";
    import Card from "./lib/ui/controls/Card.svelte";
    import { createSimulationStore } from "./lib/stores/simulationStore.svelte";

    // ---------- Stores ----------
    const { params, runtime, nav: scenario } = createSimulationStore();

    // ---------- UI-only state ----------
    let theme: "dark" | "light" = $state("light");
    let showProperty: "pressure" | "saturation_water" | "saturation_oil" | "saturation_gas" | "saturation_ternary" = $state("pressure");
    let legendFixedMin = $state(0);
    let legendFixedMax = $state(1);

    type ThreeDViewComponentType = typeof import("./lib/visualization/3dview.svelte").default;
    type RateChartComponentType = typeof import("./lib/charts/RateChart.svelte").default;
    type ReferenceComparisonChartComponentType = typeof import("./lib/charts/ReferenceComparisonChart.svelte").default;
    let ThreeDViewComponent = $state<ThreeDViewComponentType | null>(null);
    let RateChartComponent = $state<RateChartComponentType | null>(null);
    let ReferenceComparisonChartComponent = $state<ReferenceComparisonChartComponentType | null>(null);
    let loadingThreeDView = $state(false);

    // ---------- Callbacks ----------
    function handleRun() {
        const scenarioKey = scenario.activeScenarioKey;
        const dimensionKey = scenario.activeSensitivityDimensionKey;
        if (scenarioKey && dimensionKey && !scenario.isCustomMode && scenario.activeVariantKeys.length > 0) {
            runtime.runScenarioSweep(scenarioKey, dimensionKey, scenario.activeVariantKeys);
        } else {
            runtime.runSteps();
        }
    }

    function handleApplyOutputHistoryIndex(index: number) {
        if (scenario.activeSelectedReferenceResult) {
            runtime.currentIndex = index;
            return;
        }
        runtime.applyHistoryIndex(index);
    }

    function toggleTheme() {
        theme = theme === "dark" ? "light" : "dark";
    }

    // ---------- Effects ----------
    $effect(() => {
        const hasActiveResults = scenario.activeReferenceResults.length > 0;
        if (!hasActiveResults) {
            if (scenario.activeComparisonSelection.primaryResultKey ||
                scenario.activeComparisonSelection.comparedResultKeys.length > 0) {
                scenario.setComparisonSelection({ primaryResultKey: null, comparedResultKeys: [] });
            }
            return;
        }
        if (!scenario.activeComparisonSelection.primaryResultKey) return;
        if (scenario.activePrimaryComparisonResultKey) return;
        scenario.setComparisonSelection({ primaryResultKey: null, comparedResultKeys: [] });
    });

    $effect(() => {
        if (scenario.default3DProperty) {
            showProperty = scenario.default3DProperty;
        }
    });

    $effect(() => { runtime.checkConfigDiff(); });

    $effect(() => {
        if (typeof document === "undefined") return;
        document.documentElement.setAttribute("data-theme", theme);
    });
    $effect(() => {
        if (typeof localStorage === "undefined") return;
        localStorage.setItem("ressim-theme", theme);
    });

    // ---------- Lazy module loading ----------
    async function loadThreeDViewModule() {
        if (ThreeDViewComponent || loadingThreeDView) return;
        loadingThreeDView = true;
        try {
            ThreeDViewComponent = (await import("./lib/visualization/3dview.svelte")).default;
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

        try { RateChartComponent = (await import("./lib/charts/RateChart.svelte")).default; } catch {}
        try { ReferenceComparisonChartComponent = (await import("./lib/charts/ReferenceComparisonChart.svelte")).default; } catch {}
        await loadThreeDViewModule();
        await tick();

        scenario.selectScenario("wf_bl1d");
    });

    onDestroy(() => { runtime.dispose(); });
</script>

<main
    class="min-h-screen text-foreground bg-background relative"
    data-theme={theme}
>
    <div class="mx-auto w-full space-y-4 p-4 lg:p-6 2xl:px-8 relative z-2">
        <!-- Header -->
        <header class="flex flex-col gap-2 sm:flex-row sm:items-center sm:justify-between">
            <div>
                <h1 class="title-gradient text-2xl font-bold lg:text-3xl">
                    3D Three-Phase Reservoir Simulation with Analytical Reference Solutions
                </h1>
                <p class="text-sm opacity-80">
                    Compare IMPES numerical solutions against classical analytical methods with scenario-based sensitivities and 3D visualization.
                    <br /><strong>Runs entirely in your browser</strong> — no data sent to any server, no cookies.
                </p>
            </div>
            <div class="flex items-center gap-2">
                <a
                    href="https://github.com/sergeyfarin/ressim"
                    target="_blank"
                    rel="noreferrer"
                    aria-label="Open ResSim on GitHub"
                    title="Open ResSim on GitHub"
                    class="inline-flex h-9 w-9 items-center justify-center rounded-md border border-border bg-background text-foreground transition-colors hover:bg-accent hover:text-accent-foreground"
                >
                    <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="currentColor" class="h-4.5 w-4.5" aria-hidden="true">
                        <path d="M12 .5C5.65.5.5 5.66.5 12.03c0 5.1 3.3 9.42 7.88 10.95.58.11.79-.25.79-.56 0-.27-.01-1.17-.02-2.12-3.21.7-3.89-1.36-3.89-1.36-.53-1.34-1.28-1.7-1.28-1.7-1.05-.72.08-.71.08-.71 1.16.08 1.77 1.2 1.77 1.2 1.03 1.77 2.71 1.26 3.37.96.1-.75.4-1.26.72-1.55-2.56-.29-5.25-1.29-5.25-5.72 0-1.26.45-2.29 1.19-3.1-.12-.29-.52-1.48.11-3.08 0 0 .97-.31 3.19 1.18a11.1 11.1 0 0 1 5.8 0c2.22-1.49 3.19-1.18 3.19-1.18.63 1.6.23 2.79.11 3.08.74.81 1.19 1.84 1.19 3.1 0 4.44-2.69 5.42-5.26 5.71.41.36.77 1.05.77 2.12 0 1.53-.01 2.76-.01 3.14 0 .31.21.68.8.56A11.53 11.53 0 0 0 23.5 12.03C23.5 5.66 18.35.5 12 .5Z" />
                    </svg>
                </a>
                <Button size="sm" variant="outline" onclick={toggleTheme}>
                    {theme === "dark" ? "☀ Light" : "🌙 Dark"}
                </Button>
            </div>
        </header>

        <section class="space-y-2">
            <ScenarioPicker
                activeScenarioKey={scenario.activeScenarioKey}
                activeSensitivityDimensionKey={scenario.activeSensitivityDimensionKey}
                activeAnalyticalOptionKey={scenario.activeAnalyticalOptionKey}
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
                onSelectAnalyticalOption={(key) => scenario.selectAnalyticalOption(key)}
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
                totalStepsRun={runtime.rateHistory.length}
                hasValidationErrors={params.hasValidationErrors}
                numSensitivities={!scenario.isCustomMode ? scenario.activeVariantKeys.length : 0}
                runProgress={runtime.referenceSweepRunning
                    ? runtime.referenceSweepProgressLabel
                    : runtime.workerRunning && runtime.currentRunTotalSteps > 0
                        ? `${runtime.currentRunStepsCompleted}/${runtime.currentRunTotalSteps} steps`
                        : ""}
                bind:steps={params.steps}
                showStepsInput={scenario.isCustomMode}
                stopPending={runtime.stopPending}
                onStepsEdit={() => params.markStepsOverride()}
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

        {#if runtime.runtimeError}
            <div class="rounded-md border border-destructive bg-card text-destructive p-3 text-xs font-medium">
                {runtime.runtimeError}
            </div>
        {/if}

        <section class="space-y-2 mt-2">
            <div><div class="ui-section-kicker">Results</div></div>

            <div class="grid grid-cols-1 gap-4 xl:grid-cols-2 xl:items-start">
                <div class="space-y-4">
                    <Card class="overflow-hidden">
                        {#if scenario.activeChartFamily && ReferenceComparisonChartComponent}
                            <ReferenceComparisonChartComponent
                                results={scenario.activeReferenceResults}
                                family={scenario.activeChartFamily}
                                layoutConfig={scenario.activeRateChartLayoutConfig}
                                analyticalPerVariant={scenario.analyticalPerVariant}
                                {theme}
                                previewVariantParams={scenario.activeReferenceResults.length === 0 ? scenario.previewVariantParams : undefined}
                                pendingPreviewVariants={scenario.activeReferenceResults.length > 0 ? scenario.pendingPreviewVariants : undefined}
                                previewBaseParams={scenario.activeReferenceResults.length === 0 ? (params as Record<string, any>) : undefined}
                                previewAnalyticalMethod={scenario.activeChartFamily?.analyticalMethod}
                            />
                        {:else if RateChartComponent}
                            <RateChartComponent
                                panelDefs={scenario.activeScenarioObject?.liveChartPanels ?? []}
                                rateHistory={runtime.rateHistory}
                                analyticalProductionData={scenario.liveAnalyticalOutput.production}
                                avgReservoirPressureSeries={runtime.avgReservoirPressureSeries}
                                avgWaterSaturationSeries={runtime.avgWaterSaturationSeries}
                                ooipM3={params.ooipM3}
                                poreVolumeM3={params.poreVolumeM3}
                                activeMode={scenario.activeMode}
                                activeCase={scenario.activeCase}
                                {theme}
                                analyticalMeta={scenario.liveAnalyticalOutput.meta}
                                layoutConfig={scenario.activeRateChartLayoutConfig}
                                rockProps={scenario.selectedOutputProfile.rockProps}
                                fluidProps={scenario.selectedOutputProfile.fluidProps}
                                layerPermeabilities={params.permMode === 'perLayer' && params.layerPermsX.length > 1
                                    ? params.layerPermsX
                                    : params.nz > 1
                                        ? Array.from({ length: params.nz }, () => params.uniformPermX)
                                        : [params.uniformPermX]}
                                layerThickness={params.cellDz}
                                showSweepPanel={scenario.showSweepPanel}
                                sweepGeometry={scenario.sweepGeometry}
                                sweepAnalyticalMethod={scenario.sweepAnalyticalMethod}
                                sweepEfficiencySimSeries={scenario.sweepEfficiencySimSeries}
                                sweepRFAnalytical={scenario.sweepRFAnalytical}
                            />
                        {:else}
                            <div class="p-4 md:p-5 text-sm text-muted-foreground w-full text-center">
                                Loading output chart…
                            </div>
                        {/if}
                    </Card>
                </div>

                <div class="space-y-4">
                    <Card>
                        <ThreeDViewCard
                            {ThreeDViewComponent}
                            {loadingThreeDView}
                            selectedOutput3D={scenario.selectedOutput3D}
                            selectedOutputProfile={scenario.selectedOutputProfile}
                            activeReferenceResults={scenario.activeReferenceResults}
                            activePrimaryComparisonResultKey={scenario.activePrimaryComparisonResultKey}
                            {theme}
                            vizRevision={runtime.vizRevision}
                            bind:showProperty
                            bind:legendFixedMin
                            bind:legendFixedMax
                            onApplyHistoryIndex={handleApplyOutputHistoryIndex}
                            onLoadThreeDView={loadThreeDViewModule}
                            onSelectResult={(key) => scenario.setComparisonSelection({ primaryResultKey: key, comparedResultKeys: [] })}
                            onClearResult={() => scenario.setComparisonSelection({ primaryResultKey: null, comparedResultKeys: [] })}
                        />
                    </Card>
                </div>
            </div>
        </section>
    </div>
</main>
