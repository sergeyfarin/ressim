<script lang="ts">
  import StaticPropertiesPanel from './StaticPropertiesPanel.svelte';
  import ReservoirPropertiesPanel from './ReservoirPropertiesPanel.svelte';
  import RelativeCapillaryPanel from './RelativeCapillaryPanel.svelte';
  import WellPropertiesPanel from './WellPropertiesPanel.svelte';
  import TimestepControlsPanel from './TimestepControlsPanel.svelte';
  import AnalyticalInputsPanel from './AnalyticalInputsPanel.svelte';

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
    permMode = $bindable<'uniform' | 'random' | 'perLayer'>('uniform'),
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
    capillaryEnabled = $bindable(true),
    capillaryPEntry = $bindable(5),
    capillaryLambda = $bindable(2),
    well_radius = $bindable(0.1),
    well_skin = $bindable(0),
    injectorEnabled = $bindable(true),
    injectorControlMode = $bindable<'rate' | 'pressure'>('pressure'),
    producerControlMode = $bindable<'rate' | 'pressure'>('pressure'),
    injectorBhp = $bindable(400),
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
    analyticalSolutionMode = $bindable<'waterflood' | 'depletion'>('waterflood'),
    analyticalDietzShapeFactor = $bindable(21.2),
    analyticalDepletionTauScale = $bindable(0.25),
    analyticalDepletionRateScale = $bindable(1.0),
    validationErrors = {},
    validationWarnings = [],
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
    permMode?: 'uniform' | 'random' | 'perLayer';
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
    capillaryEnabled?: boolean;
    capillaryPEntry?: number;
    capillaryLambda?: number;
    well_radius?: number;
    well_skin?: number;
    injectorEnabled?: boolean;
    injectorControlMode?: 'rate' | 'pressure';
    producerControlMode?: 'rate' | 'pressure';
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
    analyticalSolutionMode?: 'waterflood' | 'depletion';
    analyticalDietzShapeFactor?: number;
    analyticalDepletionTauScale?: number;
    analyticalDepletionRateScale?: number;
    validationErrors?: Record<string, string>;
    validationWarnings?: string[];
    readOnly?: boolean;
  } = $props();
</script>

<div class="grid grid-cols-1 gap-4 lg:grid-cols-2 xl:grid-cols-3">
  <div class="space-y-3">
    <StaticPropertiesPanel
      bind:nx bind:ny bind:nz
      bind:cellDx bind:cellDy bind:cellDz
    />
    <TimestepControlsPanel
      bind:delta_t_days
      bind:max_sat_change_per_step
      bind:max_pressure_change_per_step
      bind:max_well_rate_change_fraction
      fieldErrors={validationErrors}
    />
  </div>

  <div class="space-y-3">
    <ReservoirPropertiesPanel
      bind:initialPressure bind:initialSaturation
      bind:mu_w bind:mu_o
      bind:c_o bind:c_w
      bind:rho_w bind:rho_o
      bind:rock_compressibility
      bind:depth_reference
      bind:volume_expansion_o bind:volume_expansion_w
      bind:gravityEnabled
      bind:permMode
      bind:uniformPermX bind:uniformPermY bind:uniformPermZ
      bind:useRandomSeed bind:randomSeed
      bind:minPerm bind:maxPerm
      bind:nz
      bind:layerPermsX bind:layerPermsY bind:layerPermsZ
      fieldErrors={validationErrors}
    />
  </div>

  <div class="space-y-3">
    <RelativeCapillaryPanel
      bind:s_wc bind:s_or bind:n_w bind:n_o
      bind:capillaryEnabled
      bind:capillaryPEntry bind:capillaryLambda
    />
    <WellPropertiesPanel
      bind:well_radius bind:well_skin
      bind:nx bind:ny
      bind:injectorEnabled
      bind:injectorControlMode bind:producerControlMode
      bind:injectorBhp bind:producerBhp
      bind:targetInjectorRate bind:targetProducerRate
      bind:injectorI bind:injectorJ
      bind:producerI bind:producerJ
      fieldErrors={validationErrors}
    />
    <AnalyticalInputsPanel
      bind:analyticalSolutionMode
      bind:analyticalDietzShapeFactor
      bind:analyticalDepletionTauScale
      bind:analyticalDepletionRateScale
    />
  </div>
</div>

{#if validationWarnings.length > 0}
  <div class="mt-3 card border border-warning bg-base-100 shadow-sm">
    <div class="card-body p-3 text-xs">
      {#each validationWarnings as warning}
        <div class="text-warning">⚠ {warning}</div>
      {/each}
    </div>
  </div>
{/if}

{#if readOnly}
  <div class="mt-3 text-xs opacity-60 text-center">
    Viewing pre-run case parameters. Switch to <strong>Custom</strong> mode to edit.
  </div>
{/if}
