<script lang="ts">
  import Button from "../controls/Button.svelte";
  import Card from "../controls/Card.svelte";
  import WarningPolicyPanel from "../feedback/WarningPolicyPanel.svelte";
  import ScenarioSectionsPanel from "../sections/ScenarioSectionsPanel.svelte";
  import { SCENARIOS, getScenario, type Scenario, type ScenarioDomain } from "../../catalog/scenarios";
  import type { CaseMode, ToggleState } from "../../catalog/caseCatalog";
  import type { ModePanelParameterBindings } from "../modePanelTypes";
  import type { WarningPolicy } from "../../warningPolicy";
  import type {
    BasePresetProfile,
    ScenarioNavigationState,
    ReferenceProvenance,
  } from "../../stores/phase2PresetContract";

  let {
    activeScenarioKey = null,
    activeSensitivityDimensionKey = null,
    activeVariantKeys = [],
    isCustom = false,
    activeMode = "wf",
    params,
    toggles,
    disabledOptions,
    validationErrors = {},
    warningPolicy = undefined,
    basePreset = null,
    navigationState = null,
    referenceProvenance = null,
    referenceSweepRunning = false,
    onSelectScenario = () => {},
    onSelectSensitivityDimension = () => {},
    onToggleVariant = () => {},
    onEnterCustomMode = () => {},
    onCloneReferenceToCustom = () => {},
    onActivateLibraryEntry = () => false,
    onToggleChange = () => {},
    onParamEdit = () => {},
  }: {
    activeScenarioKey?: string | null;
    activeSensitivityDimensionKey?: string | null;
    activeVariantKeys?: string[];
    isCustom?: boolean;
    activeMode?: CaseMode;
    params: ModePanelParameterBindings;
    toggles: ToggleState;
    disabledOptions: Record<string, Record<string, string>>;
    validationErrors?: Record<string, string>;
    warningPolicy?: WarningPolicy;
    basePreset?: BasePresetProfile | null;
    navigationState?: ScenarioNavigationState | null;
    referenceProvenance?: ReferenceProvenance | null;
    referenceSweepRunning?: boolean;
    onSelectScenario?: (key: string) => void;
    onSelectSensitivityDimension?: (key: string) => void;
    onToggleVariant?: (variantKey: string) => void;
    onEnterCustomMode?: () => void;
    onCloneReferenceToCustom?: () => void;
    onActivateLibraryEntry?: (entryKey: string) => boolean;
    onToggleChange?: (key: string, value: string) => void;
    onParamEdit?: () => void;
  } = $props();

  // ── Derived scenario state ──────────────────────────────────────────────────

  const activeScenario = $derived(
    !isCustom && activeScenarioKey ? getScenario(activeScenarioKey) : null,
  );

  // Active sensitivity dimension, resolved from the scenario's sensitivities array.
  const activeDimension = $derived.by(() => {
    if (!activeScenario) return null;
    if (!activeSensitivityDimensionKey) return activeScenario.sensitivities[0] ?? null;
    return activeScenario.sensitivities.find((d) => d.key === activeSensitivityDimensionKey) ?? null;
  });

  // Guard: only include variant keys that actually belong to the active dimension.
  // Prevents stale keys from a previous scenario/dimension from lingering in the UI.
  const validActiveVariantKeys = $derived.by(() => {
    if (!activeDimension) return [];
    const validKeys = new Set(activeDimension.variants.map((v) => v.key));
    return activeVariantKeys.filter((k) => validKeys.has(k));
  });

  // True if any selected variant in the active dimension updates the analytical solution.
  const dimensionAffectsAnalytical = $derived.by(() => {
    if (!activeDimension) return false;
    return activeDimension.variants
      .filter((v) => validActiveVariantKeys.includes(v.key))
      .some((v) => v.affectsAnalytical);
  });

  // True if any variant in the dimension is analytical-affecting (used for footer text).
  const anyVariantAffectsAnalytical = $derived(
    activeDimension?.variants.some((v) => v.affectsAnalytical) ?? false,
  );

  // Scenario groups by domain, ordered for display.
  const DOMAIN_GROUPS: { domain: ScenarioDomain; label: string }[] = [
    { domain: 'waterflood', label: 'Waterflood' },
    { domain: 'sweep',      label: 'Sweep' },
    { domain: 'depletion',  label: 'Depletion' },
    { domain: 'gas',        label: 'Gas' },
  ];

  function formatParamSummary(scenario: Scenario): string {
    const p = scenario.params;
    const nx = Number(p.nx ?? 1);
    const ny = Number(p.ny ?? 1);
    const nz = Number(p.nz ?? 1);
    const dx = Number(p.cellDx ?? 1);
    const lengthM = nx * dx;
    const mu_o = Number(p.mu_o ?? 1).toFixed(1);
    const mu_w = Number(p.mu_w ?? 0.5).toFixed(1);
    const injEnabled = Boolean(p.injectorEnabled);
    const pBhp = Number(p.producerBhp ?? 0);
    const iBhp = Number(p.injectorBhp ?? 0);
    const perm = Number(p.uniformPermX ?? 0);

    const gridStr = `${nx}×${ny}×${nz} cells, ${lengthM}m`;
    const fluidStr = `μ_o=${mu_o} μ_w=${mu_w} cp`;
    const permStr = `k=${perm} mD`;
    const wellStr = injEnabled
      ? `BHP ${iBhp}→${pBhp} bar`
      : `BHP ${pBhp} bar`;

    return [gridStr, fluidStr, permStr, wellStr].join("  ·  ");
  }

</script>

<Card class="p-0">
  <!-- ── Scenario selector row ── -->
  <div class="p-3 space-y-2">
    <div class="ui-panel-kicker text-muted-foreground">Scenario</div>
    <div class="flex flex-wrap items-start gap-2">

      {#each DOMAIN_GROUPS as group}
        {@const groupScenarios = SCENARIOS.filter((s) => s.domain === group.domain)}
        {#if groupScenarios.length > 0 && (group.domain !== 'gas' || activeMode === '3p')}
          {#each groupScenarios as scenario}
            <Button
              size="sm"
              variant={!isCustom && activeScenarioKey === scenario.key ? "default" : "outline"}
              onclick={() => onSelectScenario(scenario.key)}
            >
              {scenario.label}
            </Button>
          {/each}
        {/if}
      {/each}

      <!-- Custom -->
      <Button
        size="sm"
        variant={isCustom ? "default" : "outline"}
        onclick={onEnterCustomMode}
      >
        Custom
      </Button>
    </div>
  </div>

  {#if !isCustom && activeScenario}
    <!-- ── Concise parameter summary ── -->
    <div class="border-t border-border/50 px-3 py-2">
      <div class="flex items-start justify-between gap-2">
        <div class="space-y-1">
        <p class="ui-microcopy mt-0.5 font-mono text-muted-foreground/70">
        {formatParamSummary(activeScenario)}
      </p>
          <p class="ui-microcopy text-foreground">{activeScenario.description}</p>
          <p class="ui-microcopy text-foreground">
          <span class="text-foreground font-semibold">Analytical:</span>
            {activeScenario.analyticalMethodSummary}
            <span class="text-foreground"> Ref: {activeScenario.analyticalMethodReference}</span>
          </p>
        </div>
        
      </div>
      
      <!-- <Button size="sm" variant="ghost" onclick={onEnterCustomMode} class="h-6 shrink-0 px-2 text-[10px]">
          Customize →
        </Button> -->
    </div>

    <!-- ── Sensitivity panel ── -->
    {#if activeScenario.sensitivities.length > 0}
      <div class="border-t border-border/50 px-3 py-2 space-y-2">

        <!-- Dimension selector — only shown when there are multiple dimensions -->
        {#if activeScenario.sensitivities.length > 1}
          <p class="ui-subsection-kicker text-muted-foreground">Sensitivities</p>
          <div class="flex flex-wrap items-center gap-2">
            
            {#each activeScenario.sensitivities as dim}
              <Button
                size="sm"
                variant={activeDimension?.key === dim.key ? "default" : "outline"}
                onclick={() => onSelectSensitivityDimension(dim.key)}
              >
                {dim.label}
              </Button>
            {/each}
          </div>
        {/if}

        <!-- Variant chips for the active dimension -->
        {#if activeDimension}
          <div class="flex flex-wrap items-center gap-2">
            {#if activeScenario.sensitivities.length === 1}
              <span class="ui-subsection-kicker">{activeDimension.label}:</span>
            {/if}
            {#each activeDimension.variants as variant}
              <button
                type="button"
                class={`ui-chip cursor-pointer transition-colors ${
                  validActiveVariantKeys.includes(variant.key)
                    ? "border-primary/60 bg-primary/10 text-foreground"
                    : "border-border/60 bg-muted/20 text-muted-foreground hover:border-primary/40 hover:text-foreground"
                }`}
                title={variant.description}
                onclick={() => onToggleVariant(variant.key)}
              >
                {variant.label}
              </button>
            {/each}
          </div>
          {#if !anyVariantAffectsAnalytical}
            <div class="rounded border border-info/70 bg-info/10 px-2.5 py-1.5 text-xs text-info">
              Analytical solution is fixed — only simulation results change.
            </div>
          {/if}
        {/if}

      </div>
    {/if}

  {/if}
  {#if warningPolicy}
  <div class="px-3 pb-3 space-y-2">
    <WarningPolicyPanel
      policy={warningPolicy}
      groups={["blockingValidation", "nonPhysical", "referenceCaveat", "advisory"]}
      groupSources={{
        blockingValidation: ["validation"],
        nonPhysical: ["validation"],
        referenceCaveat: ["analytical"],
        advisory: ["validation"],
      }}
    /></div>
  {/if}
  {#if isCustom}
    <!-- ── Custom mode: full parameter form ── -->
    <div class="border-t border-border/50 pt-2">
      <ScenarioSectionsPanel
        {activeMode}
        {toggles}
        {disabledOptions}
        {onToggleChange}
        {onParamEdit}
        {params}
        {validationErrors}
      />
    </div>
  {/if}
    
  
</Card>
