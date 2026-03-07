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

  const showReferencePanel = $derived(Boolean(activeLibraryEntry?.benchmarkFamilyKey));
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

  function handleSourceSelect(source: "case-library" | "custom") {
    if (!navigationState || source === activeSource) return;

    if (source === "custom") {
      if (navigationState.editabilityPolicy.allowCustomizeAction) {
        onCloneReferenceToCustom();
        return;
      }

      onParamEdit();
      return;
    }

    const restoredEntry = referenceProvenance?.sourceCaseKey
      ? getCaseLibraryEntry(referenceProvenance.sourceCaseKey)
      : getCaseLibraryEntry(basePreset?.key ?? null);

    if (restoredEntry) {
      onActivateLibraryEntry(restoredEntry.key);
      return;
    }

    const familyDefaultEntry = familyDefaultEntries[activeFamily as keyof typeof familyDefaultEntries] ?? null;
    if (familyDefaultEntry) {
      onActivateLibraryEntry(familyDefaultEntry.key);
      return;
    }

    handleFamilySelect(activeFamily as keyof typeof FAMILY_LABELS);
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
          "Returning to Case Library restores a curated case for this family when one is available.",
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
        ? "Base inputs stay locked; only approved sensitivity selectors remain editable until you choose Customize."
        : activeLibraryEntry.editabilityPolicy.kind === "library-starter"
          ? "This starter stays editable immediately and transitions into custom workflow on first direct input edit."
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
        {#if navigationState?.activeLibraryGroup && activeSource === "case-library"}
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
          variant={activeFamily === family ? "default" : "outline"}
          class="justify-start"
          onclick={() => handleFamilySelect(family as keyof typeof FAMILY_LABELS)}
        >
          {label}
        </Button>
      {/each}
    </div>

    <div class="mt-3 rounded-md border border-border/60 bg-background/80 p-3">
      <div class="text-[10px] font-semibold uppercase tracking-[0.16em] text-muted-foreground">
        Source
      </div>
      <div class="mt-2 flex flex-wrap gap-2">
        <Button
          size="sm"
          variant={activeSource === "case-library" ? "default" : "outline"}
          onclick={() => handleSourceSelect("case-library")}
        >
          Case Library
        </Button>
        <Button
          size="sm"
          variant={activeSource === "custom" ? "default" : "outline"}
          onclick={() => handleSourceSelect("custom")}
        >
          Custom
        </Button>
      </div>
      <div class="mt-2 text-[10px] text-muted-foreground">
        {#if activeSource === "case-library"}
          Curated family cases keep provenance, reference guidance, and allowed sensitivities attached to the active selection.
        {:else}
          Custom keeps the current family inputs unlocked. Switch back to Case Library to restore a curated family case.
        {/if}
      </div>
    </div>
  </div>

  {#if shouldShowStatusRow}
    <div class="mt-3 flex flex-wrap items-center gap-2 text-[11px]">
      {#if Number(params.parameterOverrideCount ?? 0) > 0}
        <span class={`rounded-md border px-2 py-1 font-medium ${sourceTone}`}>
          {params.parameterOverrideCount} changed field{params.parameterOverrideCount === 1 ? "" : "s"}
        </span>
      {/if}
      {#if referenceProvenance}
        <span class="rounded-md border border-border/70 bg-muted/25 px-2 py-1 text-muted-foreground">
          Seeded from <strong class="text-foreground">{referenceProvenance.sourceLabel}</strong>
        </span>
      {/if}
    </div>
  {/if}

  {#if navigationState}
    <div class="mt-3 rounded-md border border-border/70 bg-muted/10 p-3">
      <div class="text-[10px] font-semibold uppercase tracking-[0.16em] text-muted-foreground">
        Library Context
      </div>

      {#if activeSource === "case-library" && activeLibraryEntry}
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
      {:else if !isModified && activeSource === "case-library"}
        <div class="mt-2 text-[10px] text-muted-foreground">
          Current facet combination is not mapped to a curated library case yet.
        </div>
      {:else if activeSource === "custom"}
        <div class="mt-2 space-y-1 text-[10px] text-muted-foreground">
          <div>Custom inputs are active for this family.</div>
          {#if referenceProvenance}
            <div>
              Seeded from <strong class="text-foreground">{referenceProvenance.sourceLabel}</strong>.
            </div>
          {/if}
        </div>
      {/if}

      {#if activeSource === "case-library" && librarySelectorSections.length > 0}
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

      {#if caseDisclosure}
        <div class="mt-3 border-t border-border/50 pt-3">
          <div class="text-[10px] font-semibold uppercase tracking-[0.16em] text-muted-foreground">
            Case Disclosure
          </div>
          <div class="mt-2 rounded-md border border-border/70 bg-background/80 p-3">
            <div class="text-[11px] text-foreground">
              <strong>{caseDisclosure.title}</strong>
            </div>
            <div class="mt-1 text-[10px] text-muted-foreground">
              {caseDisclosure.description}
            </div>

            <div class="mt-3 grid gap-3 xl:grid-cols-2">
              <div class="rounded-md border border-border/60 bg-muted/10 p-3">
                <div class="text-[10px] font-semibold uppercase tracking-[0.14em] text-muted-foreground">
                  Citation / Source
                </div>
                <div class="mt-2 space-y-1 text-[10px] text-muted-foreground">
                  {#each caseDisclosure.sourceItems as item}
                    <div>{item}</div>
                  {/each}
                </div>
              </div>

              <div class="rounded-md border border-border/60 bg-muted/10 p-3">
                <div class="text-[10px] font-semibold uppercase tracking-[0.14em] text-muted-foreground">
                  Fixed Settings
                </div>
                <div class="mt-2 space-y-1 text-[10px] text-muted-foreground">
                  {#each caseDisclosure.fixedSettingsItems as item}
                    <div>{item}</div>
                  {/each}
                </div>
              </div>

              <div class="rounded-md border border-border/60 bg-muted/10 p-3">
                <div class="text-[10px] font-semibold uppercase tracking-[0.14em] text-muted-foreground">
                  Allowed Sensitivities
                </div>
                <div class="mt-2 space-y-1 text-[10px] text-muted-foreground">
                  {#each caseDisclosure.sensitivityItems as item}
                    <div>{item}</div>
                  {/each}
                </div>
              </div>

              <div class="rounded-md border border-border/60 bg-muted/10 p-3">
                <div class="text-[10px] font-semibold uppercase tracking-[0.14em] text-muted-foreground">
                  Reference Guidance
                </div>
                <div class="mt-2 space-y-1 text-[10px] text-muted-foreground">
                  {#each caseDisclosure.referencePolicyItems as item}
                    <div>{item}</div>
                  {/each}
                </div>
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
              <span class="text-[10px] text-muted-foreground">
                Seeded source: <strong class="text-foreground">{referenceProvenance.sourceLabel}</strong>
              </span>
            {:else if isModified}
              <span class="text-[10px] text-muted-foreground">
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
