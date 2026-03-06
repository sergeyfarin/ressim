<script lang="ts">
  import Input from "../components/ui/Input.svelte";
  import Button from "../components/ui/Button.svelte";
  import type { ModePanelParameterBindings } from "./modePanelTypes";
  import {
    getGeometryGridControlErrorMessage,
    getGeometryGridQuickPickMatch,
    type GeometryGridChangeBehavior,
    type GeometryGridParamKey,
    type GeometryGridQuickEditorDefinition,
    type GeometryGridQuickPickControl,
    isGeometryGridQuickPickControl,
  } from "./geometryGridQuickEditor";

  let {
    editor,
    bindings,
    fieldErrors = {},
    onParamEdit = () => {},
    showHeader = true,
    hideQuickPickOptions = false,
  }: {
    editor: GeometryGridQuickEditorDefinition;
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

  function setNumberParam(
    key: GeometryGridParamKey,
    rawValue: string | number,
    integer = false,
    changeBehavior?: GeometryGridChangeBehavior,
  ) {
    const parsed = integer ? parseInt(String(rawValue), 10) : Number(rawValue);
    if (!Number.isFinite(parsed)) return;

    bindings[key] = parsed;
    applyChangeBehavior(changeBehavior);
    onParamEdit();
  }

  function applyQuickPick(control: GeometryGridQuickPickControl, optionKey: string) {
    const option = control.options.find((entry) => entry.key === optionKey);
    if (!option) return;

    for (const [patchKey, patchValue] of Object.entries(option.patch)) {
      bindings[patchKey as GeometryGridParamKey] = patchValue as number;
    }
    applyChangeBehavior(control.custom.changeBehavior);
    customOpen = { ...customOpen, [control.key]: false };
    onParamEdit();
  }

  function openCustom(controlKey: string) {
    customOpen = { ...customOpen, [controlKey]: true };
  }

  function showCustomInput(control: GeometryGridQuickPickControl): boolean {
    return (
      customOpen[control.key] ||
      getGeometryGridQuickPickMatch(control, bindings[control.param]) === null
    );
  }
</script>

<div class="quick-editor-section">
  {#if showHeader}
    <div class="quick-editor-header">
      <div>
        <h4 class="text-sm font-semibold text-foreground">{editor.label}</h4>
        {#if editor.description}
          <p class="mt-1 text-xs text-muted-foreground">{editor.description}</p>
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
    {#each editor.controls as control}
      <div class="control-card compact">
        <div class="flex items-start justify-between gap-3">
          <div>
            <div class="text-sm font-medium text-foreground">{control.label}</div>
            {#if control.description}
              <div class="mt-1 text-xs text-muted-foreground">{control.description}</div>
            {/if}
          </div>
          {#if control.type === "number" && control.unit}
            <span class="text-[10px] font-medium uppercase tracking-wide text-muted-foreground"
              >{control.unit}</span
            >
          {/if}
        </div>

        {#if isGeometryGridQuickPickControl(control)}
          {@const activeOption = getGeometryGridQuickPickMatch(control, bindings[control.param])}
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
            {@const errorMessage = getGeometryGridControlErrorMessage(
              fieldErrors,
              control.fieldErrorKeys,
            )}
            <div class="mt-3 rounded-md border border-border/70 bg-muted/20 p-2.5">
              <label class="flex flex-col gap-1.5">
                <span class="text-xs font-medium text-foreground">{control.custom.label}</span>
                <Input
                  type="number"
                  min={control.custom.min}
                  max={control.custom.max}
                  step={control.custom.step}
                  class={`h-8 px-2 ${errorMessage ? "border-destructive" : ""}`}
                  value={bindings[control.param] as number}
                  oninput={(event) =>
                    setNumberParam(
                      control.param,
                      (event.currentTarget as HTMLInputElement).value,
                      control.custom.integer,
                      control.custom.changeBehavior,
                    )}
                />
                {#if errorMessage}
                  <div class="text-[10px] text-destructive leading-tight">{errorMessage}</div>
                {/if}
              </label>
            </div>
          {/if}
        {:else}
          {@const errorMessage = getGeometryGridControlErrorMessage(
            fieldErrors,
            control.fieldErrorKeys,
          )}
          <label class="mt-3 flex flex-col gap-1.5">
            <Input
              type="number"
              min={control.min}
              max={control.max}
              step={control.step}
              class={`h-8 px-2 ${errorMessage ? "border-destructive" : ""}`}
              value={bindings[control.param] as number}
              oninput={(event) =>
                setNumberParam(
                  control.param,
                  (event.currentTarget as HTMLInputElement).value,
                  control.integer,
                  control.changeBehavior,
                )}
            />
            {#if errorMessage}
              <div class="text-[10px] text-destructive leading-tight">{errorMessage}</div>
            {/if}
          </label>
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
