<script lang="ts">
  import Button from "../controls/Button.svelte";
  import Card from "../controls/Card.svelte";
  import ToggleGroup from "../controls/ToggleGroup.svelte";
  import WarningPolicyPanel from "../feedback/WarningPolicyPanel.svelte";
  import { SCENARIOS, SCENARIO_GROUPS, getScenario, getScenarioAnalyticalOptions, getScenarioGroup, solverLabel, type Scenario } from "../../catalog/scenarios";
  import type { WarningPolicy } from "../../warningPolicy";

  let {
    activeScenarioKey = null,
    activeSensitivityDimensionKey = null,
    activeAnalyticalOptionKey = null,
    activeVariantKeys = [],
    validationErrors = {},
    warningPolicy = undefined,
    referenceSweepRunning = false,
    onSelectScenario = () => {},
    onSelectSensitivityDimension = () => {},
    onToggleVariant = () => {},
    onSelectAnalyticalOption = () => {},
  }: {
    activeScenarioKey?: string | null;
    activeSensitivityDimensionKey?: string | null;
    activeAnalyticalOptionKey?: string | null;
    activeVariantKeys?: string[];
    validationErrors?: Record<string, string>;
    warningPolicy?: WarningPolicy;
    referenceSweepRunning?: boolean;
    onSelectScenario?: (key: string) => void;
    onSelectSensitivityDimension?: (key: string) => void;
    onToggleVariant?: (variantKey: string) => void;
    onSelectAnalyticalOption?: (optionKey: string) => void;
  } = $props();

  // ── Derived scenario state ──────────────────────────────────────────────────

  const activeScenario = $derived(
    activeScenarioKey ? getScenario(activeScenarioKey) : null,
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

  // Analytical options are derived from the scenario's capabilities, not
  // declared on it — see getScenarioAnalyticalOptions(). Empty when there is no
  // genuine choice, which is what hides the toggle.
  const analyticalOptions = $derived(getScenarioAnalyticalOptions(activeScenario));

  const activeAnalyticalOption = $derived.by(() => {
    if (analyticalOptions.length === 0) return null;
    return analyticalOptions.find((option) => option.key === activeAnalyticalOptionKey)
      ?? analyticalOptions[0]
      ?? null;
  });

  const analyticalOptionToggleOptions = $derived.by(() => analyticalOptions.map((option) => ({
    value: option.key,
    label: option.label,
    title: option.summary,
  })));

</script>

<Card class="p-0">
  <!-- ── Scenario selection ── -->
  <div class="space-y-2 p-3">
    <div class="ui-panel-kicker text-muted-foreground">Scenario Selection</div>
    <div class="grid auto-rows-fr grid-cols-[repeat(auto-fill,minmax(16rem,18rem))] gap-2">
      {#each SCENARIO_GROUPS as group}
        {@const groupScenarios = SCENARIOS.filter((s) => getScenarioGroup(s) === group.key)}
        {#if groupScenarios.length > 0}
          <section
            aria-labelledby={`scenario-group-${group.key}`}
            class="flex h-full flex-col rounded-md border border-border/60 bg-muted/10 p-2.5"
          >
            <div class="mb-1.5">
              <div id={`scenario-group-${group.key}`} class="ui-support-copy font-semibold text-foreground">{group.label}</div>
              <div class="ui-microcopy text-muted-foreground">{group.description}</div>
            </div>
            <div class="flex flex-1 flex-col gap-2">
              {#each groupScenarios as scenario}
                <Button
                  size="sm"
                  class="h-auto min-h-8 w-full whitespace-normal py-2 text-center leading-tight"
                  variant={activeScenarioKey === scenario.key ? "default" : "outline"}
                  onclick={() => onSelectScenario(scenario.key)}
                >
                  {scenario.label}
                  {#if scenario.capabilities.runMode === 'prerun-artifacts'}
                    <span class="ml-1.5 rounded-sm bg-muted/60 px-1 py-0.5 text-[10px] font-medium uppercase tracking-wide text-muted-foreground">Pre-run</span>
                  {/if}
                </Button>
              {/each}
            </div>
          </section>
        {/if}
      {/each}
    </div>
  </div>

  {#if activeScenario}
    <!--
      One prose voice per block. The description used to mix a monospaced
      summary, three weights of label, a chip and an info-tinted surface at
      10px, which read as five competing systems around text nobody could
      comfortably read. Now: the description as body copy, then one labelled
      list where every row looks the same.
    -->
    <div class="space-y-2 border-t border-border/50 px-3 py-3">
      <div class="ui-panel-kicker text-muted-foreground">Scenario Description</div>
      <div class="space-y-2.5 rounded-md border border-border/60 bg-muted/10 p-3">
        <p class="ui-body-copy">{activeScenario.description}</p>

        <dl class="ui-support-copy grid gap-x-3 gap-y-1.5 sm:grid-cols-[auto_1fr]">
          <dt class="ui-subsection-kicker sm:pt-0.5">Setup</dt>
          <dd class="text-foreground">{activeScenario.catalog.parameterSummary}</dd>

          <dt class="ui-subsection-kicker sm:pt-0.5">Solver</dt>
          <dd class="text-foreground">
            <span class="font-semibold">{solverLabel(activeScenario.solverPolicy.defaultSolver)}</span>
            — {activeScenario.solverPolicy.rationale}
          </dd>

          <dt class="ui-subsection-kicker sm:pt-0.5">Reference</dt>
          <dd class="space-y-1 text-foreground">
            {#if analyticalOptionToggleOptions.length > 1 && activeAnalyticalOption}
              <ToggleGroup
                options={analyticalOptionToggleOptions}
                value={activeAnalyticalOption.key}
                onChange={(value) => onSelectAnalyticalOption(String(value))}
              />
            {/if}
            <p>{activeAnalyticalOption?.summary ?? activeScenario.analyticalMethodSummary}</p>
            <p class="text-muted-foreground">
              {activeAnalyticalOption?.reference ?? activeScenario.analyticalMethodReference}
            </p>
          </dd>
        </dl>
      </div>
    </div>

    <!-- ── Sensitivity selections ── -->
    {#if activeScenario.sensitivities.length > 0}
      <div class="space-y-2 border-t border-border/50 px-3 py-3">
        <div class="ui-panel-kicker text-muted-foreground">Sensitivity Selections</div>

        <!-- Dimension selector — only shown when there are multiple dimensions -->
        {#if activeScenario.sensitivities.length > 1}
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

        <!--
          What the selected dimension is for, in the same voice as the scenario
          description above. It was previously reachable only as a tooltip on
          the variant chips, so the teaching point of each study was invisible
          until you hovered the right thing.
        -->
        {#if activeDimension}
          <p class="ui-body-copy rounded-md border border-border/60 bg-muted/10 p-3">
            <span class="font-semibold">{activeDimension.label}.</span>
            {activeDimension.description}
          </p>
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
        {/if}

      </div>
    {/if}

  {/if}
  {#if warningPolicy}
    <div class="space-y-1.5 px-3 pb-3">
      <WarningPolicyPanel
        policy={warningPolicy}
        groups={["blockingValidation", "nonPhysical", "advisory"]}
        groupSources={{
          blockingValidation: ["validation"],
          nonPhysical: ["validation"],
          advisory: ["validation"],
        }}
      />
    </div>
  {/if}
</Card>
