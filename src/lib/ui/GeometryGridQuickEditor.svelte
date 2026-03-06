<script lang="ts">
  import Input from "../components/ui/Input.svelte";
  import Button from "../components/ui/Button.svelte";
  import type { ModePanelParameterBindings } from "./modePanelTypes";

  type GeometryGridParamKey =
    | "nx"
    | "ny"
    | "nz"
    | "cellDx"
    | "cellDy"
    | "cellDz";

  type GeometryGridChangeBehavior = "sync-layer-arrays";

  type GeometryGridQuickPickOption = {
    key: string;
    label: string;
    patch: Partial<Record<GeometryGridParamKey, number>>;
  };

  type GeometryGridQuickPickControl = {
    key: string;
    label: string;
    description?: string;
    param: GeometryGridParamKey;
    options: readonly GeometryGridQuickPickOption[];
    customLabel: string;
    min?: number;
    max?: number;
    step?: number;
    integer?: boolean;
    unit?: string;
    fieldErrorKeys?: readonly string[];
    changeBehavior?: GeometryGridChangeBehavior;
  };

  const QUICK_EDITOR_LABEL = "Geometry + Grid Overrides";
  const QUICK_EDITOR_DESCRIPTION =
    "Preset facets choose a baseline. Use quick picks for common cell counts, then open Custom for exact grid edits.";

  // Keep this control list local to the geometry component so the UI stays Svelte-owned.
  const GEOMETRY_GRID_CONTROLS: readonly GeometryGridQuickPickControl[] = [
    {
      key: "nx-quick-picks",
      label: "X Cells",
      description:
        "Fast presets for 1D/grid-density exploration. Use Custom when you need a non-catalog value.",
      param: "nx",
      fieldErrorKeys: ["nx"],
      options: [
        { key: "12", label: "12", patch: { nx: 12 } },
        { key: "24", label: "24", patch: { nx: 24 } },
        { key: "48", label: "48", patch: { nx: 48 } },
        { key: "96", label: "96", patch: { nx: 96 } },
      ],
      customLabel: "Custom X cells",
      min: 1,
      step: 1,
      integer: true,
    },
    {
      key: "ny",
      label: "Y Cells",
      description:
        "Use a slim areal count quickly, then switch to Custom when you need an exact side resolution.",
      param: "ny",
      fieldErrorKeys: ["ny"],
      options: [
        { key: "1", label: "1", patch: { ny: 1 } },
        { key: "11", label: "11", patch: { ny: 11 } },
        { key: "21", label: "21", patch: { ny: 21 } },
        { key: "41", label: "41", patch: { ny: 41 } },
      ],
      customLabel: "Custom Y cells",
      min: 1,
      step: 1,
      integer: true,
    },
    {
      key: "nz",
      label: "Z Cells",
      description:
        "Pick a common layer count first. Use Custom when you need a specific layering depth.",
      param: "nz",
      fieldErrorKeys: ["nz"],
      options: [
        { key: "1", label: "1", patch: { nz: 1 } },
        { key: "3", label: "3", patch: { nz: 3 } },
        { key: "5", label: "5", patch: { nz: 5 } },
        { key: "10", label: "10", patch: { nz: 10 } },
      ],
      customLabel: "Custom Z cells",
      min: 1,
      step: 1,
      integer: true,
      changeBehavior: "sync-layer-arrays",
    },
    {
      key: "cellDx",
      label: "dX",
      description: "Common x-direction cell lengths for fast coarse-to-fine sweeps.",
      param: "cellDx",
      fieldErrorKeys: ["cellDx"],
      options: [
        { key: "5", label: "5 m", patch: { cellDx: 5 } },
        { key: "10", label: "10 m", patch: { cellDx: 10 } },
        { key: "20", label: "20 m", patch: { cellDx: 20 } },
        { key: "40", label: "40 m", patch: { cellDx: 40 } },
      ],
      customLabel: "Custom dX",
      min: 0.1,
      step: 0.1,
      unit: "m",
    },
    {
      key: "cellDy",
      label: "dY",
      description: "Common y-direction cell lengths for quick areal spacing adjustments.",
      param: "cellDy",
      fieldErrorKeys: ["cellDy"],
      options: [
        { key: "5", label: "5 m", patch: { cellDy: 5 } },
        { key: "10", label: "10 m", patch: { cellDy: 10 } },
        { key: "20", label: "20 m", patch: { cellDy: 20 } },
        { key: "40", label: "40 m", patch: { cellDy: 40 } },
      ],
      customLabel: "Custom dY",
      min: 0.1,
      step: 0.1,
      unit: "m",
    },
    {
      key: "cellDz",
      label: "dZ",
      description:
        "Common vertical cell thickness presets for thin-bed and coarse-layer cases.",
      param: "cellDz",
      fieldErrorKeys: ["cellDz"],
      options: [
        { key: "1", label: "1 m", patch: { cellDz: 1 } },
        { key: "5", label: "5 m", patch: { cellDz: 5 } },
        { key: "10", label: "10 m", patch: { cellDz: 10 } },
        { key: "20", label: "20 m", patch: { cellDz: 20 } },
      ],
      customLabel: "Custom dZ",
      min: 0.1,
      step: 0.1,
      unit: "m",
    },
  ] as const;

  let {
    bindings,
    fieldErrors = {},
    onParamEdit = () => {},
    showHeader = true,
    hideQuickPickOptions = false,
  }: {
    bindings: ModePanelParameterBindings;
    fieldErrors?: Record<string, string>;
    onParamEdit?: () => void;
    showHeader?: boolean;
    hideQuickPickOptions?: boolean;
  } = $props();

  let customOpen = $state<Record<string, boolean>>({});

  const volumeSummary = $derived(`${bindings.nx}x${bindings.ny}x${bindings.nz} cells`);

  const extentSummary = $derived(
    `${(bindings.nx * bindings.cellDx).toFixed(1)} x ${(bindings.ny * bindings.cellDy).toFixed(1)} x ${(bindings.nz * bindings.cellDz).toFixed(1)} m`,
  );

  function applyChangeBehavior(behavior: GeometryGridChangeBehavior | undefined) {
    if (behavior === "sync-layer-arrays") {
      bindings.handleNzOrPermModeChange();
    }
  }

  function getQuickPickMatch(
    control: GeometryGridQuickPickControl,
    currentValue: unknown,
  ): GeometryGridQuickPickOption | null {
    return (
      control.options.find((option) => {
        const patchValue = option.patch[control.param];
        return typeof patchValue === "number" && Number(currentValue) === patchValue;
      }) ?? null
    );
  }

  function getControlErrorMessage(
    keys: readonly string[] | undefined,
  ): string | null {
    if (!keys?.length) return null;
    for (const key of keys) {
      if (fieldErrors[key]) return fieldErrors[key];
    }
    return null;
  }

  function setNumberParam(control: GeometryGridQuickPickControl, rawValue: string | number) {
    const parsed = control.integer ? parseInt(String(rawValue), 10) : Number(rawValue);
    if (!Number.isFinite(parsed)) return;

    bindings[control.param] = parsed;
    applyChangeBehavior(control.changeBehavior);
    onParamEdit();
  }

  function applyQuickPick(control: GeometryGridQuickPickControl, optionKey: string) {
    const option = control.options.find((entry) => entry.key === optionKey);
    if (!option) return;

    for (const [patchKey, patchValue] of Object.entries(option.patch)) {
      bindings[patchKey as GeometryGridParamKey] = patchValue as number;
    }
    applyChangeBehavior(control.changeBehavior);
    customOpen = { ...customOpen, [control.key]: false };
    onParamEdit();
  }

  function openCustom(controlKey: string) {
    customOpen = { ...customOpen, [controlKey]: true };
  }

  function showCustomInput(control: GeometryGridQuickPickControl): boolean {
    return customOpen[control.key] || getQuickPickMatch(control, bindings[control.param]) === null;
  }
</script>

<div class="quick-editor-section">
  {#if showHeader}
    <div class="quick-editor-header">
      <div>
        <h4 class="text-sm font-semibold text-foreground">{QUICK_EDITOR_LABEL}</h4>
        {#if QUICK_EDITOR_DESCRIPTION}
          <p class="mt-1 text-xs text-muted-foreground">{QUICK_EDITOR_DESCRIPTION}</p>
        {/if}
      </div>
      <div class="quick-editor-summary">
        <span>{volumeSummary}</span>
        <span>{extentSummary}</span>
      </div>
    </div>
  {:else}
    <div class="quick-editor-inline-summary">
      <span>{volumeSummary}</span>
      <span>{extentSummary}</span>
    </div>
  {/if}

  <div class="quick-editor-grid">
    {#each GEOMETRY_GRID_CONTROLS as control}
      {@const activeOption = getQuickPickMatch(control, bindings[control.param])}
      <div class="control-card compact">
        <div class="flex items-start justify-between gap-3">
          <div>
            <div class="text-sm font-medium text-foreground">{control.label}</div>
            {#if control.description}
              <div class="mt-1 text-xs text-muted-foreground">{control.description}</div>
            {/if}
          </div>
          {#if control.unit}
            <span class="text-[10px] font-medium uppercase tracking-wide text-muted-foreground"
              >{control.unit}</span
            >
          {/if}
        </div>

        {#if !hideQuickPickOptions}
          <div class="mt-3 flex flex-wrap gap-2">
            {#each control.options as option}
              <Button
                size="sm"
                variant={activeOption?.key === option.key ? "default" : "outline"}
                onclick={() => applyQuickPick(control, option.key)}
              >
                {option.label}
              </Button>
            {/each}
            <Button
              size="sm"
              variant={showCustomInput(control) ? "secondary" : "outline"}
              onclick={() => openCustom(control.key)}
            >
              Custom
            </Button>
          </div>
        {/if}

        {#if hideQuickPickOptions || showCustomInput(control)}
          {@const errorMessage = getControlErrorMessage(control.fieldErrorKeys)}
          <div class="mt-3 rounded-md border border-border/70 bg-muted/20 p-2.5">
            <label class="flex flex-col gap-1.5">
              <span class="text-xs font-medium text-foreground">{control.customLabel}</span>
              <Input
                type="number"
                min={control.min}
                max={control.max}
                step={control.step}
                class={`h-8 px-2 ${errorMessage ? "border-destructive" : ""}`}
                value={bindings[control.param] as number}
                oninput={(event) => setNumberParam(control, (event.currentTarget as HTMLInputElement).value)}
              />
              {#if errorMessage}
                <div class="text-[10px] text-destructive leading-tight">{errorMessage}</div>
              {/if}
            </label>
          </div>
        {/if}
      </div>
    {/each}
  </div>
</div>

<style>
  .quick-editor-section {
    padding: 0.9rem 1rem 1rem;
  }

  .quick-editor-header {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 1rem;
    margin-bottom: 0.85rem;
    flex-wrap: wrap;
  }

  .quick-editor-summary {
    display: inline-flex;
    flex-direction: column;
    gap: 0.2rem;
    font-size: 11px;
    color: hsl(var(--muted-foreground));
    background: hsl(var(--muted) / 0.25);
    border: 1px solid hsl(var(--border) / 0.7);
    border-radius: var(--radius);
    padding: 0.45rem 0.6rem;
    min-width: 160px;
  }

  .quick-editor-inline-summary {
    display: flex;
    flex-wrap: wrap;
    gap: 0.5rem 1rem;
    margin-bottom: 0.8rem;
    font-size: 11px;
    color: hsl(var(--muted-foreground));
  }

  .quick-editor-grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(180px, 1fr));
    gap: 0.6rem;
  }

  .control-card {
    border: 1px solid hsl(var(--border) / 0.7);
    border-radius: var(--radius);
    background: hsl(var(--card));
    padding: 0.85rem;
    box-shadow: 0 1px 2px rgba(0, 0, 0, 0.04);
  }

  .control-card.compact {
    padding: 0.65rem 0.75rem 0.75rem;
  }
</style>
