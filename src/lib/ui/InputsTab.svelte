<script lang="ts">
  import { tick } from "svelte";
  import type {
    AnalyticalStatus,
    BasePresetProfile,
    BenchmarkProvenance,
  } from "../stores/phase2PresetContract";
  import type { ValidationWarning } from "../validateInputs";
  import Button from "../components/ui/Button.svelte";
  import StaticPropertiesPanel from "./StaticPropertiesPanel.svelte";
  import ReservoirPropertiesPanel from "./ReservoirPropertiesPanel.svelte";
  import RelativeCapillaryPanel from "./RelativeCapillaryPanel.svelte";
  import WellPropertiesPanel from "./WellPropertiesPanel.svelte";
  import TimestepControlsPanel from "./TimestepControlsPanel.svelte";
  import AnalyticalInputsPanel from "./AnalyticalInputsPanel.svelte";
  import PresetCustomizeShell from "./PresetCustomizeShell.svelte";

  // All the same bindings that the sidebar panels used to have
  let {
    nx = $bindable(15),
    ny = $bindable(10),
    nz = $bindable(10),
    cellDx = $bindable(10),
    cellDy = $bindable(10),
    cellDz = $bindable(1),
    initialPressure = $bindable(300),
    initialSaturation = $bindable(0.2),
    reservoirPorosity = $bindable(0.2),
    mu_w = $bindable(0.5),
    mu_o = $bindable(1.0),
    c_o = $bindable(1e-5),
    c_w = $bindable(3e-6),
    rho_w = $bindable(1000),
    rho_o = $bindable(800),
    rock_compressibility = $bindable(1e-6),
    depth_reference = $bindable(0),
    volume_expansion_o = $bindable(1),
    volume_expansion_w = $bindable(1),
    gravityEnabled = $bindable(false),
    permMode = $bindable<"uniform" | "random" | "perLayer">("uniform"),
    uniformPermX = $bindable(100),
    uniformPermY = $bindable(100),
    uniformPermZ = $bindable(10),
    useRandomSeed = $bindable(true),
    randomSeed = $bindable(12345),
    minPerm = $bindable(50),
    maxPerm = $bindable(200),
    layerPermsX = $bindable<number[]>([]),
    layerPermsY = $bindable<number[]>([]),
    layerPermsZ = $bindable<number[]>([]),
    s_wc = $bindable(0.2),
    s_or = $bindable(0.1),
    n_w = $bindable(2),
    n_o = $bindable(2),
    k_rw_max = $bindable(1.0),
    k_ro_max = $bindable(1.0),
    capillaryEnabled = $bindable(true),
    capillaryPEntry = $bindable(5),
    capillaryLambda = $bindable(2),
    well_radius = $bindable(0.1),
    well_skin = $bindable(0),
    injectorEnabled = $bindable(true),
    injectorControlMode = $bindable<"rate" | "pressure">("pressure"),
    producerControlMode = $bindable<"rate" | "pressure">("pressure"),
    injectorBhp = $bindable(500),
    producerBhp = $bindable(100),
    targetInjectorRate = $bindable(350),
    targetProducerRate = $bindable(350),
    injectorI = $bindable(0),
    injectorJ = $bindable(0),
    producerI = $bindable(14),
    producerJ = $bindable(0),
    delta_t_days = $bindable(0.25),
    max_sat_change_per_step = $bindable(0.1),
    max_pressure_change_per_step = $bindable(75),
    max_well_rate_change_fraction = $bindable(0.75),
    analyticalSolutionMode = $bindable<"waterflood" | "depletion">(
      "waterflood",
    ),
    analyticalDepletionRateScale = $bindable(1.0),
    onAnalyticalSolutionModeChange = (_mode: "waterflood" | "depletion") => {},
    onNzOrPermModeChange = () => {},
    validationErrors = {},
    validationWarnings = [],
    basePreset = null,
    benchmarkProvenance = null,
    parameterOverrideCount = 0,
    parameterOverrideGroups = {},
    analyticalStatus = {
      level: "off",
      mode: "none",
      warningSeverity: "none",
      reasonDetails: [
        {
          code: "analytical-disabled",
          message: "Analytical overlay is disabled for this scenario.",
          severity: "notice",
        },
      ],
      reasons: ["Analytical overlay is disabled for this scenario."],
    },
    customizeSectionTarget = null,
    customizeSectionNonce = 0,
    activeCustomizeGroup = null,
    showChangedFields = false,
    onToggleShowChangedFields = () => {},
    onCustomizeGroup = (_groupKey: string) => {},
    onResetGroup = (_groupKey: string) => {},
    onDoneCustomize = () => {},
    readOnly = false,
  }: {
    nx?: number;
    ny?: number;
    nz?: number;
    cellDx?: number;
    cellDy?: number;
    cellDz?: number;
    initialPressure?: number;
    initialSaturation?: number;
    reservoirPorosity?: number;
    mu_w?: number;
    mu_o?: number;
    c_o?: number;
    c_w?: number;
    rho_w?: number;
    rho_o?: number;
    rock_compressibility?: number;
    depth_reference?: number;
    volume_expansion_o?: number;
    volume_expansion_w?: number;
    gravityEnabled?: boolean;
    permMode?: "uniform" | "random" | "perLayer";
    uniformPermX?: number;
    uniformPermY?: number;
    uniformPermZ?: number;
    useRandomSeed?: boolean;
    randomSeed?: number;
    minPerm?: number;
    maxPerm?: number;
    layerPermsX?: number[];
    layerPermsY?: number[];
    layerPermsZ?: number[];
    s_wc?: number;
    s_or?: number;
    n_w?: number;
    n_o?: number;
    k_rw_max?: number;
    k_ro_max?: number;
    capillaryEnabled?: boolean;
    capillaryPEntry?: number;
    capillaryLambda?: number;
    well_radius?: number;
    well_skin?: number;
    injectorEnabled?: boolean;
    injectorControlMode?: "rate" | "pressure";
    producerControlMode?: "rate" | "pressure";
    injectorBhp?: number;
    producerBhp?: number;
    targetInjectorRate?: number;
    targetProducerRate?: number;
    injectorI?: number;
    injectorJ?: number;
    producerI?: number;
    producerJ?: number;
    delta_t_days?: number;
    max_sat_change_per_step?: number;
    max_pressure_change_per_step?: number;
    max_well_rate_change_fraction?: number;
    analyticalSolutionMode?: "waterflood" | "depletion";
    analyticalDepletionRateScale?: number;
    onAnalyticalSolutionModeChange?: (mode: "waterflood" | "depletion") => void;
    onNzOrPermModeChange?: () => void;
    validationErrors?: Record<string, string>;
    validationWarnings?: ValidationWarning[];
    basePreset?: BasePresetProfile | null;
    benchmarkProvenance?: BenchmarkProvenance | null;
    parameterOverrideCount?: number;
    parameterOverrideGroups?: Record<string, string[]>;
    analyticalStatus?: AnalyticalStatus;
    customizeSectionTarget?:
      | "shell"
      | "static"
      | "timestep"
      | "reservoir"
      | "relcap"
      | "well"
      | "analytical"
      | null;
    customizeSectionNonce?: number;
    activeCustomizeGroup?: string | null;
    showChangedFields?: boolean;
    onToggleShowChangedFields?: () => void;
    onCustomizeGroup?: (groupKey: string) => void;
    onResetGroup?: (groupKey: string) => void;
    onDoneCustomize?: () => void;
    readOnly?: boolean;
  } = $props();

  let shellSectionEl: HTMLDivElement | null = null;
  let staticSectionEl: HTMLDivElement | null = null;
  let timestepSectionEl: HTMLDivElement | null = null;
  let reservoirSectionEl: HTMLDivElement | null = null;
  let relcapSectionEl: HTMLDivElement | null = null;
  let wellSectionEl: HTMLDivElement | null = null;
  let analyticalSectionEl: HTMLDivElement | null = null;
  let highlightedSection = $state<string | null>(null);

  async function focusCustomizeSection(
    target:
      | "shell"
      | "static"
      | "timestep"
      | "reservoir"
      | "relcap"
      | "well"
      | "analytical"
      | null,
  ) {
    if (!target) return;
    await tick();
    const el =
      target === "shell"
        ? shellSectionEl
        : target === "static"
          ? staticSectionEl
          : target === "timestep"
            ? timestepSectionEl
            : target === "reservoir"
              ? reservoirSectionEl
              : target === "relcap"
                ? relcapSectionEl
                : target === "well"
                  ? wellSectionEl
                  : analyticalSectionEl;
    if (!el) return;
    el.scrollIntoView({ behavior: "smooth", block: "start" });
    highlightedSection = target;
    setTimeout(() => {
      if (highlightedSection === target) highlightedSection = null;
    }, 1600);
  }

  $effect(() => {
    const nonce = customizeSectionNonce;
    if (!nonce) return;
    if (!customizeSectionTarget) return;
    void focusCustomizeSection(customizeSectionTarget);
  });
</script>

<div
  class={`mb-4 rounded-lg transition-shadow ${highlightedSection === "shell" ? "ring-2 ring-primary/35" : ""}`}
  bind:this={shellSectionEl}
>
  <PresetCustomizeShell
    {basePreset}
    {benchmarkProvenance}
    {parameterOverrideCount}
    {parameterOverrideGroups}
    {activeCustomizeGroup}
    {showChangedFields}
    {onToggleShowChangedFields}
    {onCustomizeGroup}
    {onResetGroup}
    {analyticalStatus}
  />
</div>

<div class="grid grid-cols-1 gap-4 lg:grid-cols-2 xl:grid-cols-3">
  <div class="space-y-3">
    <div
      class={`rounded-lg transition-shadow ${highlightedSection === "static" ? "ring-2 ring-primary/35" : ""}`}
      bind:this={staticSectionEl}
    >
      <StaticPropertiesPanel
        bind:nx
        bind:ny
        bind:nz
        bind:cellDx
        bind:cellDy
        bind:cellDz
        fieldErrors={validationErrors}
        {onNzOrPermModeChange}
      />
    </div>
    <div
      class={`rounded-lg transition-shadow ${highlightedSection === "timestep" ? "ring-2 ring-primary/35" : ""}`}
      bind:this={timestepSectionEl}
    >
      <TimestepControlsPanel
        bind:delta_t_days
        bind:max_sat_change_per_step
        bind:max_pressure_change_per_step
        bind:max_well_rate_change_fraction
        fieldErrors={validationErrors}
      />
    </div>
  </div>

  <div class="space-y-3">
    <div
      class={`rounded-lg transition-shadow ${highlightedSection === "reservoir" ? "ring-2 ring-primary/35" : ""}`}
      bind:this={reservoirSectionEl}
    >
      <ReservoirPropertiesPanel
        bind:initialPressure
        bind:initialSaturation
        bind:reservoirPorosity
        bind:mu_w
        bind:mu_o
        bind:c_o
        bind:c_w
        bind:rho_w
        bind:rho_o
        bind:rock_compressibility
        bind:depth_reference
        bind:volume_expansion_o
        bind:volume_expansion_w
        bind:gravityEnabled
        bind:permMode
        bind:uniformPermX
        bind:uniformPermY
        bind:uniformPermZ
        bind:useRandomSeed
        bind:randomSeed
        bind:minPerm
        bind:maxPerm
        bind:nz
        bind:layerPermsX
        bind:layerPermsY
        bind:layerPermsZ
        {onNzOrPermModeChange}
        fieldErrors={validationErrors}
      />
    </div>
  </div>

  <div class="space-y-3">
    <div
      class={`rounded-lg transition-shadow ${highlightedSection === "relcap" ? "ring-2 ring-primary/35" : ""}`}
      bind:this={relcapSectionEl}
    >
      <RelativeCapillaryPanel
        bind:s_wc
        bind:s_or
        bind:n_w
        bind:n_o
        bind:k_rw_max
        bind:k_ro_max
        bind:capillaryEnabled
        bind:capillaryPEntry
        bind:capillaryLambda
        fieldErrors={validationErrors}
      />
    </div>
    <div
      class={`rounded-lg transition-shadow ${highlightedSection === "well" ? "ring-2 ring-primary/35" : ""}`}
      bind:this={wellSectionEl}
    >
      <WellPropertiesPanel
        bind:well_radius
        bind:well_skin
        bind:nx
        bind:ny
        bind:injectorEnabled
        bind:injectorControlMode
        bind:producerControlMode
        bind:injectorBhp
        bind:producerBhp
        bind:targetInjectorRate
        bind:targetProducerRate
        bind:injectorI
        bind:injectorJ
        bind:producerI
        bind:producerJ
        fieldErrors={validationErrors}
      />
    </div>
    <div
      class={`rounded-lg transition-shadow ${highlightedSection === "analytical" ? "ring-2 ring-primary/35" : ""}`}
      bind:this={analyticalSectionEl}
    >
      <AnalyticalInputsPanel
        bind:analyticalSolutionMode
        bind:analyticalDepletionRateScale
        {onAnalyticalSolutionModeChange}
      />
    </div>
  </div>
</div>

{#if activeCustomizeGroup}
  <div
    class="mt-3 rounded-lg border border-primary/25 bg-primary/5 px-3 py-2 flex items-center justify-between gap-3"
  >
    <div class="text-xs">
      <span class="font-semibold text-foreground">Customizing group:</span>
      <span class="ml-1 text-primary font-medium">{activeCustomizeGroup}</span>
      <span class="ml-2 text-muted-foreground"
        >Press <strong>OK</strong> to collapse this customize session.</span
      >
    </div>
    <Button size="sm" variant="outline" onclick={onDoneCustomize}>OK</Button>
  </div>
{/if}

{#if validationWarnings.length > 0}
  <div
    class="mt-3 rounded-xl border border-warning bg-card text-card-foreground shadow-sm"
  >
    <div class="p-3 text-xs">
      {#each validationWarnings as warning}
        <div class="text-warning font-medium">⚠ {warning.message}</div>
      {/each}
    </div>
  </div>
{/if}

{#if readOnly}
  <div class="mt-3 text-xs text-muted-foreground text-center">
    Viewing locked preset parameters. Use a facet-level
    <strong class="text-foreground">Customize</strong>
    action to edit.
  </div>
{/if}
