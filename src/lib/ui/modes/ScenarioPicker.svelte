<script lang="ts">
  import Button from "../controls/Button.svelte";
  import Card from "../controls/Card.svelte";
  import WarningPolicyPanel from "../feedback/WarningPolicyPanel.svelte";
  import ScenarioSectionsPanel from "../sections/ScenarioSectionsPanel.svelte";
  import { SCENARIOS, getScenario, type Scenario } from "../../catalog/scenarios";
  import type { CaseMode, ToggleState } from "../../catalog/caseCatalog";
  import type { ModePanelParameterBindings } from "../modePanelTypes";
  import type { WarningPolicy } from "../../warningPolicy";

  let {
    activeScenarioKey = null,
    activeVariantKeys = [],
    isCustom = false,
    activeMode = "wf",
    params,
    toggles,
    disabledOptions,
    validationErrors = {},
    warningPolicy = undefined,
    onSelectScenario = () => {},
    onToggleVariant = () => {},
    onEnterCustomMode = () => {},
    onToggleChange = () => {},
    onParamEdit = () => {},
  }: {
    activeScenarioKey?: string | null;
    activeVariantKeys?: string[];
    isCustom?: boolean;
    activeMode?: CaseMode;
    params: ModePanelParameterBindings;
    toggles: ToggleState;
    disabledOptions: Record<string, Record<string, string>>;
    validationErrors?: Record<string, string>;
    warningPolicy?: WarningPolicy;
    onSelectScenario?: (key: string) => void;
    onToggleVariant?: (variantKey: string) => void;
    onEnterCustomMode?: () => void;
    onToggleChange?: (key: string, value: string) => void;
    onParamEdit?: () => void;
  } = $props();

  const activeScenario = $derived(
    !isCustom && activeScenarioKey ? getScenario(activeScenarioKey) : null,
  );

  const sensitivity = $derived(activeScenario?.sensitivity ?? null);

  // True if any variant in this sensitivity updates the analytical solution.
  const sensitivityAffectsAnalytical = $derived(
    sensitivity?.variants.some((v) => v.affectsAnalytical) ?? false,
  );

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

  function handleSelectScenario(key: string) {
    onSelectScenario(key);
  }

  function handleEnterCustomMode() {
    onEnterCustomMode();
  }
</script>

<Card class="p-0">
  <!-- ── Scenario selector row ── -->
  <div class="flex flex-wrap items-start gap-2 p-3">
    <!-- Waterflood group -->
    <div class="flex flex-wrap gap-1.5 rounded border border-border/50 p-1.5">
      {#each SCENARIOS.filter((s) => s.scenarioClass === "waterflood") as scenario}
        <Button
          size="sm"
          variant={!isCustom && activeScenarioKey === scenario.key ? "default" : "outline"}
          onclick={() => handleSelectScenario(scenario.key)}
        >
          {scenario.label}
        </Button>
      {/each}
    </div>
    <!-- Depletion group -->
    <div class="flex flex-wrap gap-1.5 rounded border border-border/50 p-1.5">
      {#each SCENARIOS.filter((s) => s.scenarioClass === "depletion") as scenario}
        <Button
          size="sm"
          variant={!isCustom && activeScenarioKey === scenario.key ? "default" : "outline"}
          onclick={() => handleSelectScenario(scenario.key)}
        >
          {scenario.label}
        </Button>
      {/each}
    </div>
    <!-- Custom -->
    <Button
      size="sm"
      variant={isCustom ? "default" : "outline"}
      onclick={handleEnterCustomMode}
    >
      Custom
    </Button>
  </div>

  {#if !isCustom && activeScenario}
    <!-- ── Concise parameter summary ── -->
    <div class="border-t border-border/50 px-3 py-2">
      <p class="ui-microcopy text-muted-foreground">{activeScenario.description}</p>
      <p class="ui-microcopy mt-1 font-mono text-muted-foreground/70">
        {formatParamSummary(activeScenario)}
      </p>
    </div>

    <!-- ── Sensitivity selector ── -->
    {#if sensitivity}
      <div class="border-t border-border/50 px-3 py-2 space-y-1.5">
        <div class="flex flex-wrap items-center gap-2">
          <span class="ui-subsection-kicker">{sensitivity.label}:</span>
          {#each sensitivity.variants as variant}
            <button
              type="button"
              class={`ui-chip cursor-pointer transition-colors ${
                activeVariantKeys.includes(variant.key)
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
        {#if sensitivityAffectsAnalytical}
          <p class="ui-microcopy text-muted-foreground/70">
            Analytical solution updates with each variant.
          </p>
        {:else}
          <p class="ui-microcopy text-muted-foreground/70">
            Analytical reference is grid-independent — only simulation results change.
          </p>
        {/if}
      </div>
    {/if}

    <!-- ── Customize action ── -->
    <div class="border-t border-border/50 px-3 py-2">
      <Button size="sm" variant="outline" onclick={handleEnterCustomMode}>
        Customize →
      </Button>
    </div>
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

  {#if warningPolicy}
    <WarningPolicyPanel
      policy={warningPolicy}
      groups={["blockingValidation", "nonPhysical", "advisory"]}
      groupSources={{
        blockingValidation: ["validation"],
        nonPhysical: ["validation"],
        advisory: ["validation"],
      }}
    />
  {/if}
</Card>
