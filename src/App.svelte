<script lang="ts">
    import { onMount, onDestroy, tick } from "svelte";
    import FractionalFlow from "./lib/FractionalFlow.svelte";
    import DepletionAnalytical from "./lib/DepletionAnalytical.svelte";
    import TopBar from "./lib/ui/TopBar.svelte";
    import RunControls from "./lib/ui/RunControls.svelte";
    import InputsTab from "./lib/ui/InputsTab.svelte";
    import SwProfileChart from "./lib/SwProfileChart.svelte";
    import Button from "./lib/components/ui/Button.svelte";
    import Card from "./lib/components/ui/Card.svelte";
    import { findCaseByKey } from "./lib/caseCatalog";
    import { createSimulationStore } from "./lib/stores/simulationStore.svelte";

    // ---------- Store ----------
    const sim = createSimulationStore();

    // ---------- UI-only state ----------
    let theme: "dark" | "light" = $state("dark");
    let showDebugState = $state(false);
    let showProperty: "pressure" | "saturation_water" | "saturation_oil" =
        $state("pressure");
    let legendFixedMin = $state(0);
    let legendFixedMax = $state(1);

    type ThreeDViewComponentType = typeof import("./lib/3dview.svelte").default;
    type RateChartComponentType =
        typeof import("./lib/RateChart.svelte").default;
    let ThreeDViewComponent = $state<ThreeDViewComponentType | null>(null);
    let RateChartComponent = $state<RateChartComponentType | null>(null);
    let loadingThreeDView = $state(false);

    // ---------- Config diff $effect ----------
    $effect(() => {
        sim.checkConfigDiff();
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
            const rateChartModule = await import("./lib/RateChart.svelte");
            RateChartComponent = rateChartModule.default;
        } catch (error) {
            console.error("Failed to load rate chart module:", error);
        }
    }

    async function loadThreeDViewModule() {
        if (ThreeDViewComponent || loadingThreeDView) return;
        loadingThreeDView = true;
        try {
            const threeDModule = await import("./lib/3dview.svelte");
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
        sim.setupWorker();

        await loadRateChartModule();
        await loadThreeDViewModule();
        await tick();

        sim.handleCategoryChange("depletion");
    });

    onDestroy(() => {
        sim.dispose();
    });
</script>

<main
    class="min-h-screen text-foreground bg-background relative"
    data-theme={theme}
>
    <!-- Geological layers — styled by .geo-layers in app.css (opacity adapts via [data-theme]) -->
    <div class="geo-layers">
        <svg
            viewBox="0 0 100 100"
            preserveAspectRatio="none"
            class="w-full h-full"
        >
            <!-- ORDER MATTERS: shallowest (top of page) rendered FIRST, deepest LAST -->
            <!-- Each path fills from its wavy top edge down to Y=100. Later paths paint over earlier ones. -->
            <!-- So you see each layer's color only in the band between its top edge and the next deeper layer's top edge. -->

            {#if theme === "dark"}
                <!-- Dark theme: muted desaturated earth tones -->
                <path
                    fill="#3a3228"
                    d="M0,5 C24,11 76,-1 100,8 L100,100 L0,100 Z"
                />
                <path
                    fill="#4a3d2e"
                    d="M0,15 C26,20 70,8 100,17 L100,100 L0,100 Z"
                />
                <path
                    fill="#3e3326"
                    d="M0,24 C30,29 65,16 100,26 L100,100 L0,100 Z"
                />
                <path
                    fill="#554838"
                    d="M0,30 C28,36 72,21 100,32 L100,100 L0,100 Z"
                />
                <path
                    fill="#4a3f30"
                    d="M0,38 C35,43 65,29 100,41 L100,100 L0,100 Z"
                />
                <path
                    fill="#362c22"
                    d="M0,45 C30,49 60,37 100,47 L100,100 L0,100 Z"
                />
                <path
                    fill="#5a4f40"
                    d="M0,49 C25,54 75,41 100,51 L100,100 L0,100 Z"
                />
                <path
                    fill="#483e30"
                    d="M0,56 C32,62 66,50 100,58 L100,100 L0,100 Z"
                />
                <path
                    fill="#3a3228"
                    d="M0,64 C26,69 70,58 100,66 L100,100 L0,100 Z"
                />
                <path
                    fill="#5e5345"
                    d="M0,69 C28,74 68,62 100,71 L100,100 L0,100 Z"
                />
                <path
                    fill="#4a3d2e"
                    d="M0,75 C32,79 68,69 100,76 L100,100 L0,100 Z"
                />
                <path
                    fill="#362c22"
                    d="M0,79 C32,83 68,74 100,80 L100,100 L0,100 Z"
                />
                <path
                    fill="#554838"
                    d="M0,84 C30,89 70,78 100,85 L100,100 L0,100 Z"
                />
                <path
                    fill="#3e3326"
                    d="M0,91 C35,94 65,86 100,93 L100,100 L0,100 Z"
                />
                <path
                    fill="#2e2620"
                    d="M0,95 C30,98 60,93 100,97 L100,100 L0,100 Z"
                />
            {:else}
                <!-- Light theme: warm saturated earth tones -->
                <path
                    fill="#F2DFB8"
                    d="M0,5 C24,11 76,-1 100,8 L100,100 L0,100 Z"
                />
                <path
                    fill="#D9B78D"
                    d="M0,15 C26,20 70,8 100,17 L100,100 L0,100 Z"
                />
                <path
                    fill="#F2DFB8"
                    d="M0,24 C30,29 65,16 100,26 L100,100 L0,100 Z"
                />
                <path
                    fill="#7A4B29"
                    d="M0,30 C28,36 72,21 100,32 L100,100 L0,100 Z"
                />
                <path
                    fill="#BA8E68"
                    d="M0,38 C35,43 65,29 100,41 L100,100 L0,100 Z"
                />
                <path
                    fill="#4A2E1B"
                    d="M0,45 C30,49 60,37 100,47 L100,100 L0,100 Z"
                />
                <path
                    fill="#7A4B29"
                    d="M0,49 C25,54 75,41 100,51 L100,100 L0,100 Z"
                />
                <path
                    fill="#D9B78D"
                    d="M0,56 C32,62 66,50 100,58 L100,100 L0,100 Z"
                />
                <path
                    fill="#9C6B46"
                    d="M0,64 C26,69 70,58 100,66 L100,100 L0,100 Z"
                />
                <path
                    fill="#e6d5b9"
                    d="M0,69 C28,74 68,62 100,71 L100,100 L0,100 Z"
                />
                <path
                    fill="#BA8E68"
                    d="M0,75 C32,79 68,69 100,76 L100,100 L0,100 Z"
                />
                <path
                    fill="#4A2E1B"
                    d="M0,79 C32,83 68,74 100,80 L100,100 L0,100 Z"
                />
                <path
                    fill="#9C6B46"
                    d="M0,84 C30,89 70,78 100,85 L100,100 L0,100 Z"
                />
                <path
                    fill="#7A4B29"
                    d="M0,91 C35,94 65,86 100,93 L100,100 L0,100 Z"
                />
                <path
                    fill="#4A2E1B"
                    d="M0,95 C30,98 60,93 100,97 L100,100 L0,100 Z"
                />
            {/if}
        </svg>
    </div>

    <!-- Gradient overlay — tints the geo-layers area and continues down the page -->
    <div class="geo-gradient-overlay"></div>

    <!-- Main Content — z-[2] ensures it renders above both layers and gradient overlay -->
    <div class="mx-auto w-full space-y-4 p-4 lg:p-6 2xl:px-8 relative z-[2]">
        <!-- Hidden component for analytical calculations -->
        <FractionalFlow
            rockProps={{
                s_wc: sim.s_wc,
                s_or: sim.s_or,
                n_w: sim.n_w,
                n_o: sim.n_o,
                k_rw_max: sim.k_rw_max,
                k_ro_max: sim.k_ro_max,
            }}
            fluidProps={{ mu_w: sim.mu_w, mu_o: sim.mu_o }}
            initialSaturation={sim.initialSaturation}
            timeHistory={sim.rateHistory.map((point) => point.time)}
            injectionRateSeries={sim.rateHistory.map((point) =>
                Number(point.total_injection ?? 0),
            )}
            reservoir={{
                length: sim.nx * sim.cellDx,
                area: sim.ny * sim.cellDy * sim.nz * sim.cellDz,
                porosity: sim.reservoirPorosity,
            }}
            scenarioMode={sim.analyticalSolutionMode}
            onAnalyticalData={(detail) => {
                if (sim.analyticalSolutionMode === "waterflood") {
                    sim.analyticalProductionData = detail.production;
                }
            }}
            onAnalyticalMeta={(detail) => {
                if (sim.analyticalSolutionMode === "waterflood") {
                    sim.analyticalMeta = detail;
                }
            }}
        />

        <DepletionAnalytical
            enabled={sim.analyticalSolutionMode === "depletion"}
            timeHistory={sim.rateHistory.map((point) => point.time)}
            reservoir={{
                length: sim.nx * sim.cellDx,
                area: sim.ny * sim.cellDy * sim.nz * sim.cellDz,
                porosity: sim.reservoirPorosity,
            }}
            initialSaturation={sim.initialSaturation}
            nz={sim.nz}
            permMode={sim.permMode}
            uniformPermX={sim.uniformPermX}
            uniformPermY={sim.uniformPermY}
            layerPermsX={sim.layerPermsX}
            layerPermsY={sim.layerPermsY}
            cellDx={sim.cellDx}
            cellDy={sim.cellDy}
            cellDz={sim.cellDz}
            wellRadius={sim.well_radius}
            wellSkin={sim.well_skin}
            muO={sim.mu_o}
            sWc={sim.s_wc}
            sOr={sim.s_or}
            nO={sim.n_o}
            c_o={sim.c_o}
            c_w={sim.c_w}
            cRock={sim.rock_compressibility}
            initialPressure={sim.initialPressure}
            producerBhp={sim.producerBhp}
            depletionRateScale={sim.analyticalDepletionRateScale}
            onAnalyticalData={(detail) => {
                if (sim.analyticalSolutionMode === "depletion") {
                    sim.analyticalProductionData = detail.production;
                }
            }}
            onAnalyticalMeta={(detail) => {
                if (sim.analyticalSolutionMode === "depletion") {
                    sim.analyticalMeta = detail;
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

        <!-- Top Bar: category buttons + case selector -->
        <TopBar
            activeCategory={sim.activeCategory}
            activeCase={sim.activeCase}
            isCustomMode={sim.isCustomMode}
            customSubCase={sim.resolveCustomSubCase(sim.activeCategory)}
            onCategoryChange={sim.handleCategoryChange}
            onCaseChange={sim.handleCaseChange}
            onCustomMode={sim.handleCustomMode}
        />

        <!-- Run Controls -->
        <RunControls
            wasmReady={sim.wasmReady}
            workerRunning={sim.workerRunning}
            runCompleted={sim.runCompleted}
            simTime={sim.simTime}
            historyLength={sim.history.length}
            estimatedRunSeconds={sim.estimatedRunSeconds}
            longRunEstimate={sim.longRunEstimate}
            canStop={sim.workerRunning}
            runProgress={sim.workerRunning && sim.currentRunTotalSteps > 0
                ? `${sim.currentRunStepsCompleted} / ${sim.currentRunTotalSteps}`
                : ""}
            inputsAnchorHref="#inputs-section"
            bind:steps={sim.steps}
            bind:historyInterval={sim.historyInterval}
            onRunSteps={sim.runSteps}
            onStepOnce={sim.stepOnce}
            onInitSimulator={sim.initSimulator}
            onStopRun={sim.stopRun}
            fieldErrors={sim.validationErrors}
        />

        <!-- Error / Warning banners -->
        {#if sim.runtimeWarning}
            <div
                class="rounded-md border border-warning bg-card text-warning p-3 text-xs font-medium"
            >
                {sim.runtimeWarning}
            </div>
        {/if}
        {#if sim.preRunWarning}
            <div
                class="rounded-md border border-warning bg-card text-warning p-3 text-xs font-medium"
            >
                {sim.preRunWarning}
            </div>
        {/if}
        {#if sim.runtimeError}
            <div
                class="rounded-md border border-destructive bg-card text-destructive p-3 text-xs font-medium"
            >
                {sim.runtimeError}
            </div>
        {/if}
        {#if sim.preRunLoading}
            <div class="text-xs text-muted-foreground text-center mt-2">
                Loading pre-run case data…
            </div>
        {/if}

        <div class="grid grid-cols-1 gap-4 xl:grid-cols-2 xl:items-start mt-2">
            <div class="space-y-4">
                <Card class="overflow-hidden">
                    {#if RateChartComponent}
                        <RateChartComponent
                            rateHistory={sim.rateHistory}
                            analyticalProductionData={sim.analyticalProductionData}
                            avgReservoirPressureSeries={sim.avgReservoirPressureSeries}
                            avgWaterSaturationSeries={sim.avgWaterSaturationSeries}
                            ooipM3={sim.ooipM3}
                            poreVolumeM3={sim.poreVolumeM3}
                            activeCategory={sim.activeCategory}
                            activeCase={sim.activeCase}
                            {theme}
                            analyticalMeta={sim.analyticalMeta}
                            layoutConfig={findCaseByKey(sim.activeCase)?.case
                                ?.params?.layout}
                        />
                    {:else}
                        <div
                            class="p-4 md:p-5 text-sm text-muted-foreground w-full text-center"
                        >
                            Loading rate chart…
                        </div>
                    {/if}
                </Card>

                <Card>
                    <div class="p-4 md:p-5">
                        <SwProfileChart
                            gridState={sim.gridStateRaw ?? null}
                            nx={sim.nx}
                            ny={sim.ny}
                            nz={sim.nz}
                            cellDx={sim.cellDx}
                            cellDy={sim.cellDy}
                            cellDz={sim.cellDz}
                            simTime={sim.simTime}
                            producerJ={sim.producerJ}
                            initialSaturation={sim.initialSaturation}
                            injectionRate={sim.latestInjectionRate}
                            scenarioMode={sim.analyticalSolutionMode}
                            rockProps={{
                                s_wc: sim.s_wc,
                                s_or: sim.s_or,
                                n_w: sim.n_w,
                                n_o: sim.n_o,
                            }}
                            fluidProps={{ mu_w: sim.mu_w, mu_o: sim.mu_o }}
                        />
                    </div>
                </Card>

                {#if sim.analyticalMeta.mode === "depletion"}
                    <div
                        class="rounded-md border border-border bg-card p-3 text-xs shadow-sm"
                    >
                        <div class="font-semibold text-foreground">
                            Depletion Analytical Mode
                        </div>
                        <div class="text-muted-foreground mt-1">
                            Model: {sim.analyticalMeta.shapeLabel || "PSS"} — q(t)&nbsp;=&nbsp;J·ΔP·e<sup
                                >−t/τ</sup
                            >, τ&nbsp;=&nbsp;V<sub>p</sub>·c<sub>t</sub>/J
                        </div>
                    </div>
                {/if}
            </div>

            <div class="space-y-4">
                <Card>
                    <div class="p-4 md:p-5">
                        {#if ThreeDViewComponent}
                            {#key `${sim.nx}-${sim.ny}-${sim.nz}-${sim.vizRevision}`}
                                <ThreeDViewComponent
                                    nx={sim.nx}
                                    ny={sim.ny}
                                    nz={sim.nz}
                                    cellDx={sim.cellDx}
                                    cellDy={sim.cellDy}
                                    cellDz={sim.cellDz}
                                    {theme}
                                    gridState={sim.gridStateRaw}
                                    bind:showProperty
                                    bind:legendFixedMin
                                    bind:legendFixedMax
                                    s_wc={sim.s_wc}
                                    s_or={sim.s_or}
                                    bind:currentIndex={sim.currentIndex}
                                    replayTime={sim.replayTime}
                                    onApplyHistoryIndex={sim.applyHistoryIndex}
                                    history={sim.history}
                                    wellState={sim.wellStateRaw}
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
                                            >Loading...</span
                                        >
                                    </div>
                                {:else}
                                    <Button
                                        size="sm"
                                        variant="default"
                                        onclick={loadThreeDViewModule}
                                        >Load 3D view</Button
                                    >
                                {/if}
                            </div>
                        {/if}
                    </div>
                </Card>
            </div>
        </div>

        <div id="inputs-section" class="mt-4">
            <InputsTab
                bind:nx={sim.nx}
                bind:ny={sim.ny}
                bind:nz={sim.nz}
                bind:cellDx={sim.cellDx}
                bind:cellDy={sim.cellDy}
                bind:cellDz={sim.cellDz}
                bind:initialPressure={sim.initialPressure}
                bind:initialSaturation={sim.initialSaturation}
                bind:reservoirPorosity={sim.reservoirPorosity}
                bind:mu_w={sim.mu_w}
                bind:mu_o={sim.mu_o}
                bind:c_o={sim.c_o}
                bind:c_w={sim.c_w}
                bind:rho_w={sim.rho_w}
                bind:rho_o={sim.rho_o}
                bind:rock_compressibility={sim.rock_compressibility}
                bind:depth_reference={sim.depth_reference}
                bind:volume_expansion_o={sim.volume_expansion_o}
                bind:volume_expansion_w={sim.volume_expansion_w}
                bind:gravityEnabled={sim.gravityEnabled}
                bind:permMode={sim.permMode}
                bind:uniformPermX={sim.uniformPermX}
                bind:uniformPermY={sim.uniformPermY}
                bind:uniformPermZ={sim.uniformPermZ}
                bind:useRandomSeed={sim.useRandomSeed}
                bind:randomSeed={sim.randomSeed}
                bind:minPerm={sim.minPerm}
                bind:maxPerm={sim.maxPerm}
                bind:layerPermsX={sim.layerPermsX}
                bind:layerPermsY={sim.layerPermsY}
                bind:layerPermsZ={sim.layerPermsZ}
                bind:s_wc={sim.s_wc}
                bind:s_or={sim.s_or}
                bind:n_w={sim.n_w}
                bind:n_o={sim.n_o}
                bind:capillaryEnabled={sim.capillaryEnabled}
                bind:capillaryPEntry={sim.capillaryPEntry}
                bind:capillaryLambda={sim.capillaryLambda}
                bind:well_radius={sim.well_radius}
                bind:well_skin={sim.well_skin}
                bind:injectorEnabled={sim.injectorEnabled}
                bind:injectorControlMode={sim.injectorControlMode}
                bind:producerControlMode={sim.producerControlMode}
                bind:injectorBhp={sim.injectorBhp}
                bind:producerBhp={sim.producerBhp}
                bind:targetInjectorRate={sim.targetInjectorRate}
                bind:targetProducerRate={sim.targetProducerRate}
                bind:injectorI={sim.injectorI}
                bind:injectorJ={sim.injectorJ}
                bind:producerI={sim.producerI}
                bind:producerJ={sim.producerJ}
                bind:delta_t_days={sim.delta_t_days}
                bind:max_sat_change_per_step={sim.max_sat_change_per_step}
                bind:max_pressure_change_per_step={
                    sim.max_pressure_change_per_step
                }
                bind:max_well_rate_change_fraction={
                    sim.max_well_rate_change_fraction
                }
                bind:analyticalSolutionMode={sim.analyticalSolutionMode}
                bind:analyticalDepletionRateScale={
                    sim.analyticalDepletionRateScale
                }
                onAnalyticalSolutionModeChange={sim.handleAnalyticalSolutionModeChange}
                onNzOrPermModeChange={sim.handleNzOrPermModeChange}
                validationErrors={sim.validationErrors}
                validationWarnings={sim.validationWarnings}
                readOnly={false}
            />
        </div>

        <!-- Debug State -->
        {#if showDebugState}
            <Card class="mt-4">
                <div class="grid gap-4 p-4 lg:grid-cols-2">
                    <div>
                        <h4 class="mb-2 text-sm font-semibold">
                            Grid State (current)
                        </h4>
                        <pre
                            class="max-h-[400px] overflow-auto rounded border border-border bg-muted/20 p-2 text-xs">{JSON.stringify(
                                sim.gridStateRaw,
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
                                sim.wellStateRaw,
                                null,
                                2,
                            )}</pre>
                    </div>
                </div>
            </Card>
        {/if}
    </div>
</main>
