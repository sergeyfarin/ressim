<script lang="ts">
  import Button from "../controls/Button.svelte";
  import Card from "../controls/Card.svelte";
  import { getCaseLibraryEntries, getCaseLibraryEntry, type CaseMode } from "../../catalog/caseCatalog";
  import { shouldShowModePanelStatusRow } from "../../stores/phase2PresetContract";
  import WarningPolicyPanel from "../feedback/WarningPolicyPanel.svelte";
  import type { ModePanelProps } from "../modePanelTypes";
  import ScenarioSectionsPanel from "../sections/ScenarioSectionsPanel.svelte";

  let {
    activeMode = "dep",
    navigationState = undefined,
    isModified = false,
    toggles = {},
    disabledOptions = {},
    onModeChange,
    onParamEdit = () => {},
    onToggleChange,
    basePreset = null,
    referenceProvenance = null,
    referenceSweepRunning = false,
    onCloneReferenceToCustom = () => {},
    onActivateLibraryEntry = () => {},
    params,
    validationErrors = {},
    warningPolicy = undefined,
  }: ModePanelProps = $props();

  const sourceTone = $derived(
    basePreset?.source === "custom"
      ? "border-amber-300 bg-amber-50 text-amber-700 dark:border-amber-700/70 dark:bg-amber-950/40 dark:text-amber-300"
      : basePreset?.source === "reference"
        ? "border-sky-300 bg-sky-50 text-sky-700 dark:border-sky-700/70 dark:bg-sky-950/40 dark:text-sky-300"
        : "border-emerald-300 bg-emerald-50 text-emerald-700 dark:border-emerald-700/70 dark:bg-emerald-950/40 dark:text-emerald-300",
  );

  const shouldShowStatusRow = $derived(
    shouldShowModePanelStatusRow({
      referenceProvenance,
      parameterOverrideCount: Number(params.parameterOverrideCount ?? 0),
    }),
  );

  const activeSource = $derived(navigationState?.activeSource ?? "case-library");

  const activeLibraryEntry = $derived(
    navigationState?.activeLibraryCaseKey
      ? getCaseLibraryEntry(navigationState.activeLibraryCaseKey)
      : null,
  );

  const FAMILY_LABELS = {
    waterflood: "Waterflood",
    "depletion-analysis": "Depletion Analysis",
    "type-curves": "Type Curves",
    "scenario-builder": "Scenario Builder",
  } as const;

  const FAMILY_SUBTITLES = {
    waterflood: "Build flood cases, inspect reference families, and prepare custom runs in one input surface.",
    "depletion-analysis": "Inspect depletion references and editable depletion starters without leaving the family.",
    "type-curves": "Use Fetkovich-style reference cases as the seed flow for future type-curve workflows.",
    "scenario-builder": "Assemble exploratory scenarios that do not fit the tighter reference-family workflows.",
  } as const;

  const GROUP_LABELS = {
    "literature-reference": "Literature References",
    "internal-reference": "Internal References",
    "curated-starter": "Curated Starters",
  } as const;

  const RUN_POLICY_LABELS = {
    single: "Single locked reference run",
    sweep: "Library sensitivity run set",
    "compare-to-reference": "Reference review run",
  } as const;

  const X_AXIS_LABELS = {
    time: "time",
    pvi: "pore volume injected",
    tD: "dimensionless time",
  } as const;

  const PANEL_LABELS = {
    "watercut-breakthrough": "watercut breakthrough",
    recovery: "recovery",
    pressure: "pressure",
    rates: "rates",
    "oil-rate": "oil rate",
    "cumulative-oil": "cumulative oil",
    "decline-diagnostics": "decline diagnostics",
  } as const;

  const activeFamily = $derived(
    navigationState?.activeFamily
      ?? (activeMode === "wf"
        ? "waterflood"
        : activeMode === "sim"
          ? "scenario-builder"
          : "depletion-analysis"),
  );

  const familyDefaultEntries = $derived.by(() => {
    const entries = getCaseLibraryEntries();
    const resolveDefault = (family: keyof typeof FAMILY_LABELS, preferredKey?: string) => (
      entries.find((entry) => entry.key === preferredKey)
      ?? entries.find((entry) => entry.family === family && entry.group === "curated-starter")
      ?? entries.find((entry) => entry.family === family)
      ?? null
    );

    return {
      waterflood: resolveDefault("waterflood"),
      "depletion-analysis": resolveDefault("depletion-analysis"),
      "type-curves": resolveDefault("type-curves", "fetkovich_exp"),
      "scenario-builder": resolveDefault("scenario-builder"),
    };
  });

  const showReferencePanel = $derived(Boolean(
    activeSource === "case-library"
    && activeLibraryEntry
    && !activeLibraryEntry.editabilityPolicy.allowDirectInputEditing,
  ));
  const activeReferenceCaseKey = $derived(
    navigationState?.activeLibraryCaseKey ?? null,
  );

  const scenarioPanelMode = $derived(activeMode);

  function handleFamilySelect(family: keyof typeof FAMILY_LABELS) {
    if (family === "waterflood") {
      onModeChange("wf");
      return;
    }

    if (family === "scenario-builder") {
      onModeChange("sim");
      return;
    }

    if (family === "depletion-analysis") {
      onModeChange("dep");
      return;
    }

    const defaultEntry = familyDefaultEntries[family];
    if (defaultEntry) {
      onActivateLibraryEntry(defaultEntry.key);
      return;
    }

    onModeChange("dep");
  }

  function handleCustomCaseSelect() {
    if (!navigationState || activeSource === "custom") return;

    if (navigationState.editabilityPolicy.allowCustomizeAction) {
      onCloneReferenceToCustom();
      return;
    }

    onParamEdit();
  }

  const librarySelectorSections = $derived.by(() => {
    const visibleFamilies = [activeFamily];

    const orderedGroups = ["literature-reference", "internal-reference", "curated-starter"];
    const entries = getCaseLibraryEntries();
    const sections: Array<{
      family: string;
      familyLabel: string;
      group: string;
      groupLabel: string;
      entries: typeof entries;
    }> = [];

    for (const family of visibleFamilies) {
      const familyEntries = entries.filter((entry) => entry.family === family);
      for (const group of orderedGroups) {
        const sectionEntries = familyEntries.filter((entry) => entry.group === group);
        if (sectionEntries.length === 0) continue;

        sections.push({
          family,
          familyLabel: FAMILY_LABELS[family as keyof typeof FAMILY_LABELS] ?? family,
          group,
          groupLabel: GROUP_LABELS[group as keyof typeof GROUP_LABELS] ?? group,
          entries: sectionEntries,
        });
      }
    }

    return sections;
  });

  function formatTolerancePercent(value: number | null | undefined): string | null {
    if (!Number.isFinite(value)) return null;
    return `${(Number(value) * 100).toFixed(1)}%`;
  }

  const caseDisclosure = $derived.by(() => {
    if (activeSource === "custom") {
      const familyLabel = FAMILY_LABELS[activeFamily as keyof typeof FAMILY_LABELS] ?? activeFamily;
      const sourceItems = [
        "Custom inputs are active for this family.",
      ];

      if (referenceProvenance?.sourceLabel) {
        sourceItems.push(`Seeded from ${referenceProvenance.sourceLabel}.`);
      }

      return {
        title: `${familyLabel} Custom`,
        description: "Writable family-local scenario state. Curated case constraints no longer lock the input surface while custom is active.",
        sourceItems,
        fixedSettingsItems: [
          "Inputs are unlocked for direct editing.",
          "Select any curated case below to restore library guidance for this family.",
        ],
        sensitivityItems: [
          "No locked library sensitivity policy applies while custom is active.",
        ],
        referencePolicyItems: [
          "Reference guidance now depends on whichever curated case you restore or activate next.",
        ],
      };
    }

    if (!activeLibraryEntry) return null;

    const sourceItems = [
      `Catalog source: ${activeLibraryEntry.sourceLabel}.`,
      `Library group: ${GROUP_LABELS[activeLibraryEntry.group]}.`,
      activeLibraryEntry.provenanceSummary,
    ];

    if (activeLibraryEntry.referenceSourceLabel) {
      sourceItems.splice(1, 0, `Reference source: ${activeLibraryEntry.referenceSourceLabel}.`);
    }

    const fixedSettingsItems = [
      activeLibraryEntry.editabilityPolicy.kind === "library-reference"
        ? activeLibraryEntry.sensitivityAxes.length > 0
          ? "Base inputs stay locked; only approved sensitivity selectors remain editable until you choose Customize."
          : "Base inputs stay locked until you choose Customize."
        : "This case is already in a writable custom state.",
    ];

    if (activeLibraryEntry.runPolicy) {
      fixedSettingsItems.push(`Run approach: ${RUN_POLICY_LABELS[activeLibraryEntry.runPolicy]}.`);
    }

    if (activeLibraryEntry.displayDefaults) {
      const panelLabels = activeLibraryEntry.displayDefaults.panels
        .map((panel) => PANEL_LABELS[panel as keyof typeof PANEL_LABELS] ?? panel)
        .join(", ");
      fixedSettingsItems.push(
        `Default outputs use ${X_AXIS_LABELS[activeLibraryEntry.displayDefaults.xAxis]} on the x-axis and emphasize ${panelLabels}.`,
      );
    }

    const sensitivityItems = [activeLibraryEntry.sensitivitySummary];
    if (activeLibraryEntry.sensitivityAxes.length > 0) {
      sensitivityItems.push(
        `Available axes: ${activeLibraryEntry.sensitivityAxes.map((axis) => `${axis.label} (${axis.variantCount})`).join(", ")}.`,
      );
    }

    const referencePolicyItems = [activeLibraryEntry.referencePolicySummary];
    if (activeLibraryEntry.comparisonMetric) {
      const tolerance = formatTolerancePercent(activeLibraryEntry.comparisonMetric.tolerance);
      referencePolicyItems.push(
        `Primary review metric: breakthrough PVI relative error against ${activeLibraryEntry.comparisonMetric.target === "analytical-reference" ? "the reference solution" : "the refined numerical reference"}${tolerance ? ` with a ${tolerance} tolerance target` : ""}.`,
      );
    }

    return {
      title: activeLibraryEntry.label,
      description: activeLibraryEntry.description,
      sourceItems,
      fixedSettingsItems,
      sensitivityItems,
      referencePolicyItems,
    };
  });
</script>

<Card class="p-0">
    <div class="mt-3 grid gap-2 md:grid-cols-2 xl:grid-cols-4">
      {#each Object.entries(FAMILY_LABELS) as [family, label]}
        <Button
          size="sm"
          variant={activeFamily === family ? "default" : "outline"}
          class="justify-start"
          onclick={() => handleFamilySelect(family as keyof typeof FAMILY_LABELS)}
        >
          {label}
        </Button>
      {/each}
    </div>
  

  {#if shouldShowStatusRow}
    <div class="mt-3 flex flex-wrap items-center gap-2">
      {#if Number(params.parameterOverrideCount ?? 0) > 0}
        <span class={`ui-chip ${sourceTone}`}>
          {params.parameterOverrideCount} changed field{params.parameterOverrideCount === 1 ? "" : "s"}
        </span>
      {/if}
      {#if referenceProvenance}
        <span class="ui-chip border border-border/70 bg-muted/25 text-muted-foreground">
          Seeded from <strong class="text-foreground">{referenceProvenance.sourceLabel}</strong>
        </span>
      {/if}
    </div>
  {/if}

  {#if navigationState}
    <div class="mt-3 rounded-md border border-border/70 bg-muted/10 p-3">
      <div class="space-y-1">
        <div class="ui-card-title">Case Library</div>
        <div class="ui-support-copy max-w-3xl">
          Select <strong class="text-foreground">Custom</strong> at the end of the list to branch the active library case into a writable family-local scenario.
        </div>
      </div>

      {#if librarySelectorSections.length > 0 || navigationState}
        <div class="mt-3 border-t border-border/50 pt-3">
          <div class="mt-3 space-y-3">
            {#each librarySelectorSections as section}
              <div class="space-y-2">
                <div class="flex flex-wrap items-center justify-between gap-2">
                  <span class="ui-subsection-kicker text-foreground">{section.familyLabel}</span>
                  <span class="ui-microcopy">{section.groupLabel}</span>
                </div>
                <div class="grid gap-2 md:grid-cols-2">
                  {#each section.entries as entry}
                    <button
                      type="button"
                      class={`rounded-md border px-3 py-2 text-left transition-colors ${navigationState.activeLibraryCaseKey === entry.key
                        ? "border-primary/60 bg-primary/10"
                        : "border-border/70 bg-background hover:bg-muted/30"}`}
                      onclick={() => onActivateLibraryEntry(entry.key)}
                    >
                      <div class="flex items-center justify-between gap-2">
                        <strong class="font-semibold text-foreground">{entry.label}</strong>
                        {#if navigationState.activeLibraryCaseKey === entry.key}
                          <span class="ui-microcopy text-primary">Active</span>
                        {/if}
                      </div>
                      <div class="ui-microcopy mt-1">
                        {entry.description}
                      </div>
                    </button>
                  {/each}
                </div>
              </div>
            {/each}

            <div class="space-y-2 border-t border-border/50 pt-3">
              <div class="flex flex-wrap items-center justify-between gap-2">
                <span class="ui-subsection-kicker text-foreground">{FAMILY_LABELS[activeFamily as keyof typeof FAMILY_LABELS] ?? activeFamily}</span>
                <span class="ui-microcopy">Writable branch</span>
              </div>
              <div class="grid gap-2 md:grid-cols-2">
                <button
                  type="button"
                  class={`rounded-md border px-3 py-2 text-left transition-colors ${activeSource === "custom"
                    ? "border-primary/60 bg-primary/10"
                    : "border-border/70 bg-background hover:bg-muted/30"}`}
                  onclick={handleCustomCaseSelect}
                >
                  <div class="flex items-center justify-between gap-2">
                    <strong class="font-semibold text-foreground">Custom</strong>
                    {#if activeSource === "custom"}
                      <span class="ui-microcopy text-primary">Active</span>
                    {/if}
                  </div>
                  <div class="ui-microcopy mt-1">
                    {#if activeSource === "custom"}
                      Writable family-local scenario state is active. Choose any curated case above to restore library guidance.
                    {:else if navigationState.editabilityPolicy.allowCustomizeAction}
                      Seed the active library case into a writable custom scenario for direct edits.
                    {:else}
                      Switch this family into writable direct editing without leaving the case list.
                    {/if}
                  </div>
                </button>
              </div>
            </div>
          </div>
        </div>
      {/if}


    </div>
  {/if}

  <div class="mt-4">
    {#if showReferencePanel}
      {#if activeReferenceCaseKey}
        <div class="space-y-3 border-t border-border/50 pt-2">
          <div class="mt-2 flex flex-wrap items-center gap-2">
            <Button
              size="sm"
              variant="outline"
              disabled={isModified || referenceSweepRunning}
              onclick={onCloneReferenceToCustom}
            >
              Customize
            </Button>
            {#if referenceProvenance}
              <span class="ui-microcopy">
                Seeded source: <strong class="text-foreground">{referenceProvenance.sourceLabel}</strong>
              </span>
            {:else if isModified}
              <span class="ui-microcopy">
                Customized without source provenance
              </span>
            {/if}
          </div>
        </div>
      {/if}
    {:else}
      <ScenarioSectionsPanel
        activeMode={scenarioPanelMode}
        {toggles}
        {disabledOptions}
        {onToggleChange}
        {onParamEdit}
        {params}
        {validationErrors}
      />
    {/if}
  </div>

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
