<script lang="ts">
  import Button from "../controls/Button.svelte";
  import Card from "../controls/Card.svelte";
  import { getCaseLibraryEntries, getCaseLibraryEntry, type CaseMode } from "../../catalog/caseCatalog";
  import { shouldShowModePanelStatusRow } from "../../stores/phase2PresetContract";
  import WarningPolicyPanel from "../feedback/WarningPolicyPanel.svelte";
  import type { ModePanelProps } from "../modePanelTypes";
  import ScenarioSectionsPanel from "../sections/ScenarioSectionsPanel.svelte";
  import BenchmarkPanel from "./BenchmarkPanel.svelte";

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
    benchmarkProvenance = null,
    benchmarkSweepRunning = false,
    benchmarkSweepProgressLabel = "",
    benchmarkSweepError = "",
    benchmarkRunResults = [],
    onCloneBenchmarkToCustom = () => {},
    onActivateLibraryEntry = () => {},
    onRunBenchmarkSelection = () => {},
    onStopBenchmarkSweep = () => {},
    params,
    validationErrors = {},
    warningPolicy = undefined,
  }: ModePanelProps = $props();

  const sourceTone = $derived(
    basePreset?.source === "custom"
      ? "border-amber-300 bg-amber-50 text-amber-700 dark:border-amber-700/70 dark:bg-amber-950/40 dark:text-amber-300"
      : basePreset?.source === "benchmark"
        ? "border-sky-300 bg-sky-50 text-sky-700 dark:border-sky-700/70 dark:bg-sky-950/40 dark:text-sky-300"
        : "border-emerald-300 bg-emerald-50 text-emerald-700 dark:border-emerald-700/70 dark:bg-emerald-950/40 dark:text-emerald-300",
  );

  const shouldShowStatusRow = $derived(
    shouldShowModePanelStatusRow({
      benchmarkProvenance,
      parameterOverrideCount: Number(params.parameterOverrideCount ?? 0),
    }),
  );

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

  const showReferencePanel = $derived(
    Boolean(activeLibraryEntry?.benchmarkFamilyKey) || activeMode === "benchmark",
  );

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
</script>

<Card class="p-3 md:p-4">
  <div class="rounded-md border border-border/70 bg-muted/10 p-3">
    <div class="text-[10px] font-semibold uppercase tracking-[0.16em] text-muted-foreground">
      Inputs
    </div>
    <div class="mt-2 flex flex-col gap-3 lg:flex-row lg:items-start lg:justify-between">
      <div class="space-y-1">
        <div class="text-sm font-semibold text-foreground">
          {FAMILY_LABELS[activeFamily as keyof typeof FAMILY_LABELS] ?? activeFamily}
        </div>
        <div class="max-w-2xl text-[11px] text-muted-foreground">
          {FAMILY_SUBTITLES[activeFamily as keyof typeof FAMILY_SUBTITLES] ?? "Family-first inputs shell in progress."}
        </div>
      </div>

      <div class="flex flex-wrap items-center gap-2 text-[10px]">
        <span class={`rounded-md border px-2 py-1 font-medium ${sourceTone}`}>
          {navigationState?.activeSource === "custom" ? "Custom" : "Case Library"}
        </span>
        {#if navigationState?.activeLibraryGroup}
          <span class="rounded-md border border-border/70 bg-background px-2 py-1 text-muted-foreground">
            {GROUP_LABELS[navigationState.activeLibraryGroup]}
          </span>
        {/if}
        {#if isModified}
          <span
            class="inline-flex items-center gap-1 rounded-md border border-amber-300 bg-amber-50 px-2 py-1 font-medium text-amber-700 dark:border-amber-600 dark:bg-amber-900/30 dark:text-amber-400"
          >
            Customized
          </span>
        {/if}
      </div>
    </div>

    <div class="mt-3 grid gap-2 md:grid-cols-2 xl:grid-cols-4">
      {#each Object.entries(FAMILY_LABELS) as [family, label]}
        <Button
          size="sm"
          variant={activeFamily === family && !isModified ? "default" : "outline"}
          class="justify-start"
          onclick={() => handleFamilySelect(family as keyof typeof FAMILY_LABELS)}
        >
          {label}
        </Button>
      {/each}
    </div>
  </div>

  {#if shouldShowStatusRow}
    <div class="mt-3 flex flex-wrap items-center gap-2 text-[11px]">
      {#if Number(params.parameterOverrideCount ?? 0) > 0}
        <span class={`rounded-md border px-2 py-1 font-medium ${sourceTone}`}>
          {params.parameterOverrideCount} changed field{params.parameterOverrideCount === 1 ? "" : "s"}
        </span>
      {/if}
      {#if benchmarkProvenance}
        <span class="rounded-md border border-border/70 bg-muted/25 px-2 py-1 text-muted-foreground">
          Cloned from <strong class="text-foreground">{benchmarkProvenance.sourceLabel}</strong>
        </span>
      {/if}
    </div>
  {/if}

  {#if navigationState}
    <div class="mt-3 rounded-md border border-border/70 bg-muted/10 p-3">
      <div class="text-[10px] font-semibold uppercase tracking-[0.16em] text-muted-foreground">
        Library Context
      </div>

      {#if activeLibraryEntry}
        <div class="mt-2 space-y-1 text-[11px]">
          <div class="text-foreground">
            <strong>{activeLibraryEntry.label}</strong>
          </div>
          {#if navigationState.sourceLabel}
            <div class="text-muted-foreground">
              Source: <strong class="text-foreground">{navigationState.sourceLabel}</strong>
            </div>
          {/if}
          {#if navigationState.referenceSourceLabel}
            <div class="text-muted-foreground">
              Reference: <strong class="text-foreground">{navigationState.referenceSourceLabel}</strong>
            </div>
          {/if}
          {#if navigationState.provenanceSummary}
            <div class="text-[10px] text-muted-foreground">
              {navigationState.provenanceSummary}
            </div>
          {/if}
        </div>
      {:else if !isModified && navigationState.activeSource === "case-library"}
        <div class="mt-2 text-[10px] text-muted-foreground">
          Current facet combination is not mapped to a curated library case yet.
        </div>
      {/if}

      {#if librarySelectorSections.length > 0}
        <div class="mt-3 border-t border-border/50 pt-3">
          <div class="text-[10px] font-semibold uppercase tracking-[0.16em] text-muted-foreground">
            Case Library
          </div>
          <div class="mt-3 space-y-3">
            {#each librarySelectorSections as section}
              <div class="space-y-2">
                <div class="flex flex-wrap items-center justify-between gap-2 text-[10px]">
                  <span class="font-semibold uppercase tracking-[0.14em] text-foreground">{section.familyLabel}</span>
                  <span class="text-muted-foreground">{section.groupLabel}</span>
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
                      <div class="flex items-center justify-between gap-2 text-[11px]">
                        <strong class="font-semibold text-foreground">{entry.label}</strong>
                        {#if navigationState.activeLibraryCaseKey === entry.key}
                          <span class="text-[10px] text-primary">Active</span>
                        {/if}
                      </div>
                      <div class="mt-1 text-[10px] text-muted-foreground">
                        {entry.description}
                      </div>
                    </button>
                  {/each}
                </div>
              </div>
            {/each}
          </div>
        </div>
      {/if}
    </div>
  {/if}

  <div class="mt-4">
    {#if showReferencePanel}
      <BenchmarkPanel
        {navigationState}
        {toggles}
        {disabledOptions}
        {isModified}
        {benchmarkProvenance}
        {benchmarkSweepRunning}
        {benchmarkSweepProgressLabel}
        {benchmarkSweepError}
        {benchmarkRunResults}
        {onToggleChange}
        {onCloneBenchmarkToCustom}
        {onRunBenchmarkSelection}
        {onStopBenchmarkSweep}
      />
    {:else}
      <ScenarioSectionsPanel
        activeMode={activeMode}
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
