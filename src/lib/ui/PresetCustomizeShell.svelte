<script lang="ts">
  type BasePreset = {
    key: string;
    mode: "dep" | "wf" | "sim" | "benchmark";
    source: "facet" | "benchmark" | "custom";
    label: string;
    toggles: Record<string, string>;
    benchmarkId: string | null;
  };

  type BenchmarkProvenance = {
    sourceBenchmarkId: string;
    sourceCaseKey: string;
    sourceLabel: string;
    clonedAtIso: string;
  };

  type AnalyticalStatus = {
    level: "reference" | "approximate" | "off";
    mode: "waterflood" | "depletion" | "none";
    warningSeverity: "none" | "notice" | "warning" | "critical";
    reasonDetails: Array<{
      code: string;
      message: string;
      severity: "notice" | "warning" | "critical";
    }>;
    reasons: string[];
  };

  let {
    basePreset = null,
    benchmarkProvenance = null,
    parameterOverrideCount = 0,
    parameterOverrideGroups = {},
    activeCustomizeGroup = null,
    showChangedFields = false,
    onToggleShowChangedFields = () => {},
    onCustomizeGroup = (_groupKey: string) => {},
    onResetGroup = (_groupKey: string) => {},
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
  }: {
    basePreset?: BasePreset | null;
    benchmarkProvenance?: BenchmarkProvenance | null;
    parameterOverrideCount?: number;
    parameterOverrideGroups?: Record<string, string[]>;
    activeCustomizeGroup?: string | null;
    showChangedFields?: boolean;
    onToggleShowChangedFields?: () => void;
    onCustomizeGroup?: (groupKey: string) => void;
    onResetGroup?: (groupKey: string) => void;
    analyticalStatus?: AnalyticalStatus;
  } = $props();

  const nonEmptyGroups = $derived(
    Object.entries(parameterOverrideGroups).filter(([, keys]) =>
      Array.isArray(keys) && keys.length > 0,
    ),
  );

  const sourceTone = $derived(
    basePreset?.source === "custom"
      ? "text-amber-700 border-amber-300 bg-amber-50"
      : basePreset?.source === "benchmark"
        ? "text-sky-700 border-sky-300 bg-sky-50"
        : "text-emerald-700 border-emerald-300 bg-emerald-50",
  );

  const analyticalTone = $derived(
    analyticalStatus.level === "reference"
      ? "text-emerald-700 border-emerald-300 bg-emerald-50"
      : analyticalStatus.level === "approximate"
        ? "text-amber-700 border-amber-300 bg-amber-50"
        : "text-muted-foreground border-border bg-muted/20",
  );

  function prettyMode(mode: string | undefined): string {
    if (mode === "dep") return "Depletion";
    if (mode === "wf") return "Waterflood";
    if (mode === "sim") return "Simulation";
    if (mode === "benchmark") return "Benchmark";
    return "Unknown";
  }

  function prettySource(source: string | undefined): string {
    if (source === "facet") return "Facet Preset";
    if (source === "benchmark") return "Benchmark Preset";
    if (source === "custom") return "Custom";
    return "Unknown";
  }

  function prettyGroup(groupKey: string): string {
    if (groupKey === "grid") return "Grid";
    if (groupKey === "initial") return "Initial";
    if (groupKey === "fluids") return "Fluids";
    if (groupKey === "permeability") return "Permeability";
    if (groupKey === "relperm") return "RelPerm";
    if (groupKey === "wells") return "Wells";
    if (groupKey === "stability") return "Stability";
    if (groupKey === "physics") return "Physics";
    if (groupKey === "analytical") return "Analytical";
    return groupKey;
  }
</script>

<div class="rounded-xl border border-border bg-card text-card-foreground shadow-sm">
  <div class="border-b border-border/70 px-4 py-3">
    <div class="flex flex-wrap items-center justify-between gap-2">
      <div>
        <h3 class="text-sm font-semibold">Preset + Customize</h3>
        <p class="text-xs text-muted-foreground">
          P2.2 shell: choose a base preset, then inspect and customize tracked overrides.
        </p>
      </div>
      <span class="rounded-md border px-2 py-1 text-[11px] font-medium {sourceTone}">
        {prettySource(basePreset?.source)}
      </span>
    </div>
  </div>

  <div class="grid gap-3 p-4 md:grid-cols-3">
    <section class="rounded-lg border border-border/70 bg-muted/20 p-3">
      <div class="text-[11px] font-semibold uppercase tracking-wide text-muted-foreground">
        Step 1: Preset Base
      </div>
      <div class="mt-2 space-y-1 text-xs">
        <div>
          <span class="text-muted-foreground">Mode:</span>
          <strong class="ml-1">{prettyMode(basePreset?.mode)}</strong>
        </div>
        <div>
          <span class="text-muted-foreground">Label:</span>
          <strong class="ml-1">{basePreset?.label || "N/A"}</strong>
        </div>
        {#if basePreset?.benchmarkId}
          <div>
            <span class="text-muted-foreground">Benchmark:</span>
            <code class="ml-1 text-[11px]">{basePreset.benchmarkId}</code>
          </div>
        {/if}
      </div>
    </section>

    <section class="rounded-lg border border-border/70 bg-muted/20 p-3">
      <div class="text-[11px] font-semibold uppercase tracking-wide text-muted-foreground">
        Step 2: Generated Profile
      </div>
      <div class="mt-2 text-xs">
        <div>
          <span class="text-muted-foreground">Tracked overrides:</span>
          <strong class="ml-1">{parameterOverrideCount}</strong>
        </div>
        <div>
          <span class="text-muted-foreground">Groups with changes:</span>
          <strong class="ml-1">{nonEmptyGroups.length}</strong>
        </div>
      </div>
      {#if parameterOverrideCount > 0}
        <button
          class="mt-2 rounded border border-border/80 bg-card px-2 py-1 text-[11px] font-medium text-foreground transition-colors hover:bg-muted/60"
          onclick={onToggleShowChangedFields}
        >
          {showChangedFields ? "Hide changed fields" : "Show changed fields"}
        </button>
      {/if}
      {#if benchmarkProvenance}
        <div class="mt-2 rounded border border-border/70 bg-card px-2 py-1 text-[11px]">
          Cloned from <strong>{benchmarkProvenance.sourceLabel}</strong>
        </div>
      {/if}
    </section>

    <section class="rounded-lg border border-border/70 bg-muted/20 p-3">
      <div class="text-[11px] font-semibold uppercase tracking-wide text-muted-foreground">
        Step 3: Analytical Status
      </div>
      <div class="mt-2">
        <span class="rounded-md border px-2 py-1 text-[11px] font-medium {analyticalTone}">
          {analyticalStatus.level.toUpperCase()} · {analyticalStatus.mode}
          {#if analyticalStatus.warningSeverity !== "none"}
            · {analyticalStatus.warningSeverity}
          {/if}
        </span>
      </div>
      {#if analyticalStatus.reasons.length > 0}
        <div class="mt-2 text-[11px] text-muted-foreground">
          {analyticalStatus.reasons[0]}
        </div>
      {/if}
    </section>
  </div>

  {#if nonEmptyGroups.length > 0}
    <div class="border-t border-border/70 px-4 py-3">
      <div class="mb-2 text-[11px] font-semibold uppercase tracking-wide text-muted-foreground">
        Changed Fields by Group
      </div>
      {#if !showChangedFields}
        <div class="flex flex-wrap gap-2">
          {#each nonEmptyGroups as [group, keys]}
            <span class="rounded-full border border-border bg-card px-2 py-1 text-[11px]">
              <strong>{prettyGroup(group)}</strong>: {keys.length}
            </span>
          {/each}
        </div>
      {:else}
        <div class="space-y-2">
          {#each nonEmptyGroups as [group, keys]}
            <div class="rounded-md border border-border/80 bg-card px-2 py-2">
              <div class="flex flex-wrap items-center justify-between gap-2">
                <div class="text-[11px] font-medium text-foreground">
                  {prettyGroup(group)}
                  <span class="ml-1 text-muted-foreground">({keys.length})</span>
                </div>
                <div class="flex items-center gap-1">
                  <button
                    class={`rounded px-1.5 py-0.5 text-[10px] font-medium transition-colors ${
                      activeCustomizeGroup === group
                        ? "bg-primary/15 text-primary"
                        : "text-muted-foreground hover:text-foreground hover:bg-muted/60"
                    }`}
                    onclick={() => onCustomizeGroup(group)}
                    title={`Customize ${prettyGroup(group)} fields`}
                  >
                    Customize
                  </button>
                  <button
                    class="rounded px-1.5 py-0.5 text-[10px] font-medium text-muted-foreground transition-colors hover:text-foreground hover:bg-muted/60"
                    onclick={() => onResetGroup(group)}
                    title={`Reset ${prettyGroup(group)} fields back to preset values`}
                  >
                    Reset
                  </button>
                </div>
              </div>
              <div class="mt-1 flex flex-wrap gap-1">
                {#each keys as key}
                  <code class="rounded border border-border/80 bg-muted/40 px-1.5 py-0.5 text-[10px]">
                    {key}
                  </code>
                {/each}
              </div>
            </div>
          {/each}
        </div>
      {/if}
    </div>
  {/if}
</div>
