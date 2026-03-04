<script lang="ts">
  import Button from "../components/ui/Button.svelte";
  import FilterCard from "./FilterCard.svelte";
  import {
    type CaseMode,
    type CaseEntry,
    caseCatalog,
    FACET_OPTIONS,
    FOCUS_OPTIONS,
  } from "../caseCatalog";

  let {
    activeMode = "depletion" as CaseMode,
    activeCase = "",
    isCustomMode = false,
    customSubCase = null,
    toggles,
    matchingCases = [] as CaseEntry[],
    onModeChange,
    onCaseChange,
    onCustomMode,
    onToggleChange,
  } = $props<{
    activeMode: CaseMode;
    activeCase: string;
    isCustomMode?: boolean;
    customSubCase?: { key: string; label: string } | null;
    toggles: {
      geometry: string;
      wellPosition: string;
      permeability: string;
      gravity: boolean;
      capillary: boolean;
      fluids: string;
      focus: string;
    };
    matchingCases: CaseEntry[];
    onModeChange: (mode: CaseMode) => void;
    onCaseChange: (key: string) => void;
    onCustomMode: () => void;
    onToggleChange: (key: string, value: any) => void;
  }>();

  let isCollapsed = $state(false);

  // ── Disability Rules ──
  const disabledOptions = $derived.by(() => {
    const d: Record<
      string,
      { options: string[]; reasons: Record<string, string> }
    > = {};
    const is1D = toggles.geometry === "1D";

    if (is1D) {
      d.gravity = {
        options: ["on"],
        reasons: { on: "Gravity requires 2D or 3D geometry" },
      };
      d.permeability = {
        options: ["layered"],
        reasons: { layered: "Layered rock needs nz > 1" },
      };
      d.wellPosition = {
        options: ["center", "off-center"],
        reasons: {
          center: "1D has no areal position",
          "off-center": "1D has no areal position",
        },
      };
    }
    if (activeMode === "waterflood" && is1D) {
      d.wellPosition = {
        options: ["corner", "center", "off-center"],
        reasons: {
          corner: "WF 1D is always end-to-end",
          center: "WF 1D is always end-to-end",
          "off-center": "WF 1D is always end-to-end",
        },
      };
    }
    return d;
  });

  function getDisabled(card: string): string[] {
    return disabledOptions[card]?.options ?? [];
  }
  function getDisabledReasons(card: string): Record<string, string> {
    return disabledOptions[card]?.reasons ?? {};
  }

  // ── Focus options for the active mode ──
  const focusOptions = $derived(
    (FOCUS_OPTIONS[activeMode] ?? []).map((o) => o.label),
  );
  const focusValues = $derived(
    (FOCUS_OPTIONS[activeMode] ?? []).map((o) => o.value),
  );
  function focusValueToLabel(v: string): string {
    const opt = (FOCUS_OPTIONS[activeMode] ?? []).find((o) => o.value === v);
    return opt?.label ?? v;
  }
  function focusLabelToValue(label: string): string {
    const opt = (FOCUS_OPTIONS[activeMode] ?? []).find(
      (o) => o.label === label,
    );
    return opt?.value ?? label;
  }

  // ── Variant grouping ──
  const hasVariants = $derived(
    matchingCases.length > 1 && matchingCases[0]?.variantGroup,
  );
  const resolvedCase = $derived(
    matchingCases.length === 1
      ? matchingCases[0]
      : (matchingCases.find((c) => c.key === activeCase) ??
          matchingCases[0] ??
          null),
  );

  const activeModeLabel = $derived(
    activeMode === "depletion"
      ? "Depletion"
      : activeMode === "waterflood"
        ? "Waterflood"
        : activeMode === "benchmark"
          ? "Benchmarks"
          : "Simulation",
  );

  // ── Auto-select first matching case when toggles resolve ──
  $effect(() => {
    if (isCustomMode || activeMode === "benchmark") return;
    if (matchingCases.length > 0) {
      const currentStillValid = matchingCases.some((c) => c.key === activeCase);
      if (!currentStillValid) {
        onCaseChange(matchingCases[0].key);
      }
    }
  });
</script>

<div class="case-selector" class:collapsed={isCollapsed}>
  <!-- Row 1: Mode tabs + collapse toggle -->
  <div class="flex items-center gap-2 flex-wrap">
    {#each [["depletion", "Depletion"], ["waterflood", "Waterflood"], ["simulation", "Simulation"], ["benchmark", "Benchmarks"]] as [mode, label]}
      <Button
        size="sm"
        variant={activeMode === mode && !isCustomMode ? "default" : "outline"}
        onclick={() => {
          onModeChange(mode as CaseMode);
          isCollapsed = false;
        }}
      >
        {label}
      </Button>
    {/each}
    <Button
      size="sm"
      variant={isCustomMode ? "default" : "outline"}
      onclick={onCustomMode}
    >
      ⚙ Custom
    </Button>

    <button
      class="ml-auto text-xs text-muted-foreground hover:text-foreground transition-colors flex items-center gap-1 font-medium px-2 py-1 rounded"
      onclick={() => (isCollapsed = !isCollapsed)}
    >
      {isCollapsed ? "▸ Expand" : "▾ Collapse"}
    </button>
  </div>

  {#if isCollapsed && !isCustomMode}
    <!-- Collapsed summary -->
    <div
      class="mt-2 flex items-center gap-2 text-xs text-muted-foreground flex-wrap"
    >
      <span class="font-medium text-foreground">{activeModeLabel}</span>
      {#if resolvedCase}
        <span>·</span>
        <span class="text-foreground font-semibold">{resolvedCase.label}</span>
        <span class="text-[10px] opacity-60">({resolvedCase.description})</span>
      {/if}
    </div>
  {:else if !isCollapsed && !isCustomMode && activeMode !== "benchmark"}
    <!-- Row 2: Toggle cards -->
    <div
      class="flex flex-wrap gap-2 mt-3 animate-in fade-in slide-in-from-top-1 duration-200"
    >
      <FilterCard
        label="Geometry"
        options={["1D", "2D", "3D"]}
        selected={toggles.geometry}
        onchange={(v) => onToggleChange("geometry", v)}
      />
      <FilterCard
        label="Well Position"
        options={["end-to-end", "corner", "center", "off-center"]}
        selected={toggles.wellPosition}
        disabled={getDisabled("wellPosition")}
        disabledReasons={getDisabledReasons("wellPosition")}
        onchange={(v) => onToggleChange("wellPosition", v)}
      />
      <FilterCard
        label="Rock"
        options={["uniform", "layered", "random"]}
        selected={toggles.permeability}
        disabled={getDisabled("permeability")}
        disabledReasons={getDisabledReasons("permeability")}
        onchange={(v) => onToggleChange("permeability", v)}
      />
      <FilterCard
        label="Gravity"
        options={["off", "on"]}
        selected={toggles.gravity ? "on" : "off"}
        disabled={getDisabled("gravity")}
        disabledReasons={getDisabledReasons("gravity")}
        onchange={(v) => onToggleChange("gravity", v === "on")}
      />
      <FilterCard
        label="Capillary"
        options={["off", "on"]}
        selected={toggles.capillary ? "on" : "off"}
        onchange={(v) => onToggleChange("capillary", v === "on")}
      />
      <FilterCard
        label="Fluids"
        options={FACET_OPTIONS.fluids}
        selected={toggles.fluids}
        onchange={(v) => onToggleChange("fluids", v)}
      />
      <FilterCard
        label="Focus"
        options={focusOptions}
        selected={focusValueToLabel(toggles.focus)}
        onchange={(v) => onToggleChange("focus", focusLabelToValue(v))}
      />
    </div>

    <!-- Row 3: Variant pills (if multiple cases share the toggle combo) -->
    {#if hasVariants}
      <div
        class="flex flex-wrap gap-1.5 items-center mt-3 pt-2 border-t border-border/40"
      >
        <span
          class="text-[10px] font-semibold text-muted-foreground uppercase mr-1"
          >Variants:</span
        >
        {#each matchingCases as c}
          <Button
            size="xs"
            variant={activeCase === c.key ? "default" : "outline"}
            onclick={() => onCaseChange(c.key)}
            title={c.description}
            class={activeCase === c.key
              ? "ring-1 ring-primary"
              : "opacity-80 hover:opacity-100"}
          >
            <span class="text-[11px] font-medium"
              >{c.variantLabel ?? c.label}</span
            >
          </Button>
        {/each}
      </div>
    {/if}

    <!-- Row 4: End-state panel (always visible) -->
    {#if resolvedCase}
      <div class="end-state-panel mt-3 pt-3 border-t border-border/50">
        <div class="flex items-start gap-2">
          <div
            class="w-2 h-2 rounded-full mt-1 shrink-0
                      {resolvedCase.runTimeEstimate === 'fast'
              ? 'bg-emerald-500'
              : resolvedCase.runTimeEstimate === 'medium'
                ? 'bg-amber-500'
                : 'bg-rose-500'}"
            title="Est. runtime: {resolvedCase.runTimeEstimate}"
          ></div>
          <div class="flex-1 min-w-0">
            <div class="text-sm font-semibold text-foreground">
              {resolvedCase.label}
            </div>
            <div class="text-[11px] text-muted-foreground mt-0.5">
              {resolvedCase.description}
            </div>
            <div
              class="flex flex-wrap gap-x-3 gap-y-0.5 mt-1.5 text-[10px] text-muted-foreground"
            >
              <span
                >Geometry: <strong class="text-foreground"
                  >{toggles.geometry}</strong
                ></span
              >
              <span
                >Well: <strong class="text-foreground"
                  >{toggles.wellPosition}</strong
                ></span
              >
              <span
                >Rock: <strong class="text-foreground"
                  >{toggles.permeability}</strong
                ></span
              >
              <span
                >Gravity: <strong class="text-foreground"
                  >{toggles.gravity ? "On" : "Off"}</strong
                ></span
              >
              <span
                >Capillary: <strong class="text-foreground"
                  >{toggles.capillary ? "On" : "Off"}</strong
                ></span
              >
              <span
                >Fluids: <strong class="text-foreground"
                  >{toggles.fluids}</strong
                ></span
              >
              <span
                >Focus: <strong class="text-foreground"
                  >{focusValueToLabel(toggles.focus)}</strong
                ></span
              >
            </div>
          </div>
        </div>
      </div>
    {:else if matchingCases.length === 0}
      <div class="mt-3 pt-3 border-t border-border/50">
        <div class="text-xs text-muted-foreground italic">
          No case matches this combination. Try adjusting your toggles.
        </div>
      </div>
    {/if}
  {:else if activeMode === "benchmark" && !isCustomMode}
    <!-- Benchmark mode: simple list -->
    <div class="flex flex-wrap gap-2 mt-3">
      {#each caseCatalog.filter((c) => c.facets.mode === "benchmark") as c}
        <Button
          size="sm"
          variant={activeCase === c.key ? "default" : "outline"}
          onclick={() => onCaseChange(c.key)}
        >
          {c.label}
        </Button>
      {/each}
    </div>
    {#if resolvedCase}
      <div class="text-xs text-muted-foreground mt-2">
        {resolvedCase.description}
      </div>
    {/if}
  {:else if isCustomMode}
    <div class="mt-3 text-sm text-muted-foreground px-2">
      Editing simulation parameters directly. Select a mode above to return to
      defined cases.
      {#if customSubCase}
        <div class="mt-1 font-medium text-foreground">
          {customSubCase.label}
        </div>
      {/if}
    </div>
  {/if}
</div>

<style>
  .case-selector {
    background-color: hsl(var(--card));
    border: 1px solid hsl(var(--border) / 0.8);
    border-radius: var(--radius);
    padding: 12px 16px;
    box-shadow: 0 1px 3px rgba(0, 0, 0, 0.1);
    transition: all 0.2s ease-in-out;
  }
  .end-state-panel {
    background: hsl(var(--muted) / 0.3);
    border-radius: var(--radius);
    padding: 8px 10px;
    margin-top: 12px;
  }
</style>
