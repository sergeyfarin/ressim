<script lang="ts">
  type ShowProperty = 'pressure' | 'saturation_water' | 'saturation_oil' | 'permeability_x' | 'permeability_y' | 'permeability_z' | 'porosity';

  let {
    showProperty = $bindable<ShowProperty>('pressure'),
    legendFixedMin = $bindable(0),
    legendFixedMax = $bindable(1),
    autoLegendMin = $bindable(true),
    autoLegendMax = $bindable(true),
    historyLength = 0,
    currentIndex = $bindable(-1),
    replayTime = null,
    playing = $bindable(false),
    playSpeed = $bindable(2),
    showDebugState = $bindable(false),
    onApplyHistoryIndex = () => {},
    onPrev = () => {},
    onNext = () => {},
    onTogglePlay = () => {},
  }: {
    showProperty?: ShowProperty;
    legendFixedMin?: number;
    legendFixedMax?: number;
    autoLegendMin?: boolean;
    autoLegendMax?: boolean;
    historyLength?: number;
    currentIndex?: number;
    replayTime?: number | null;
    playing?: boolean;
    playSpeed?: number;
    showDebugState?: boolean;
    onApplyHistoryIndex?: (index: number) => void;
    onPrev?: () => void;
    onNext?: () => void;
    onTogglePlay?: () => void;
  } = $props();

  $effect(() => {
    const minValue = Number(legendFixedMin);
    const maxValue = Number(legendFixedMax);

    if (!Number.isFinite(minValue)) {
      legendFixedMin = 0;
    }
    if (!Number.isFinite(maxValue)) {
      legendFixedMax = 1;
    }

    if (Number.isFinite(legendFixedMin) && Number.isFinite(legendFixedMax) && legendFixedMin > legendFixedMax) {
      const tmp = legendFixedMin;
      legendFixedMin = legendFixedMax;
      legendFixedMax = tmp;
    }
  });

  function onLegendMinInput(event: Event) {
    const input = event.currentTarget as HTMLInputElement;
    legendFixedMin = Number(input.value);
    autoLegendMin = false;
  }

  function onLegendMaxInput(event: Event) {
    const input = event.currentTarget as HTMLInputElement;
    legendFixedMax = Number(input.value);
    autoLegendMax = false;
  }

  function applySliderValue(event: Event) {
    const input = event.currentTarget as HTMLInputElement;
    currentIndex = Number(input.value);
    onApplyHistoryIndex(currentIndex);
  }

  const showPropertyOptions: Array<{ value: ShowProperty; label: string }> = [
    { value: 'pressure', label: 'Pressure' },
    { value: 'saturation_water', label: 'Water Sat' },
    { value: 'saturation_oil', label: 'Oil Sat' },
    { value: 'permeability_x', label: 'Perm X' },
    { value: 'permeability_y', label: 'Perm Y' },
    { value: 'permeability_z', label: 'Perm Z' },
    { value: 'porosity', label: 'Porosity' },
  ];

  const groupSummary = $derived(`${showProperty.replace('_', ' ')} · step ${Math.max(0, currentIndex)}`);
</script>

<details class="rounded-lg border border-base-300 bg-base-100 shadow-sm" open>
  <summary class="flex cursor-pointer list-none items-center justify-between px-4 py-3 md:px-5">
    <div>
      <div class="font-semibold">Visualization and Replay</div>
      <div class="text-xs opacity-70">{groupSummary}</div>
    </div>
    <div class="flex items-center gap-2 text-xs opacity-70">
      <span class="collapse-label-open hidden">Collapse</span>
      <span class="collapse-label-closed">Expand</span>
      <span class="collapse-chevron">▸</span>
    </div>
  </summary>
  <div class="space-y-3 border-t border-base-300 p-4 md:p-5">
    <p class="text-xs opacity-70">Display selection, legend behavior, and timeline navigation.</p>

    <label class="form-control">
      <span class="label-text text-xs">Property</span>
      <div class="flex flex-wrap gap-1">
        {#each showPropertyOptions as option}
          <button
            type="button"
            class="btn btn-xs {showProperty === option.value ? 'btn-primary' : 'btn-outline'}"
            onclick={() => showProperty = option.value}
          >
            {option.label}
          </button>
        {/each}
      </div>
    </label>

    <div class="grid grid-cols-1 gap-2 sm:grid-cols-2">
      <label class="form-control">
        <span class="label-text text-xs">Legend Min</span>
        <div
          class="flex items-center gap-2 rounded-md border p-1 transition-colors"
          class:border-base-300={autoLegendMin}
          class:bg-base-100={autoLegendMin}
          class:border-primary={!autoLegendMin}
          class:bg-base-200={!autoLegendMin}
        >
          <label class="label cursor-pointer gap-2 py-0">
            <input type="checkbox" class="toggle toggle-xs" bind:checked={autoLegendMin} />
            <span class="label-text text-xs">Auto min</span>
          </label>
          <input
            type="number"
            step="any"
            class="input input-bordered input-sm w-full"
            class:input-primary={!autoLegendMin}
            value={legendFixedMin}
            oninput={onLegendMinInput}
          />
        </div>
        {#if !autoLegendMin}
          <span class="label-text-alt text-[11px] text-primary/80">Manual mode</span>
        {/if}
      </label>
      <label class="form-control">
        <span class="label-text text-xs">Legend Max</span>
        <div
          class="flex items-center gap-2 rounded-md border p-1 transition-colors"
          class:border-base-300={autoLegendMax}
          class:bg-base-100={autoLegendMax}
          class:border-primary={!autoLegendMax}
          class:bg-base-200={!autoLegendMax}
        >
          <label class="label cursor-pointer gap-2 py-0">
            <input type="checkbox" class="toggle toggle-xs" bind:checked={autoLegendMax} />
            <span class="label-text text-xs">Auto max</span>
          </label>
          <input
            type="number"
            step="any"
            class="input input-bordered input-sm w-full"
            class:input-primary={!autoLegendMax}
            value={legendFixedMax}
            oninput={onLegendMaxInput}
          />
        </div>
        {#if !autoLegendMax}
          <span class="label-text-alt text-[11px] text-primary/80">Manual mode</span>
        {/if}
      </label>
    </div>

    <div class="space-y-1">
      <input
        type="range"
        class="range range-sm"
        min="0"
        max={Math.max(0, historyLength - 1)}
        bind:value={currentIndex}
        oninput={applySliderValue}
        onchange={applySliderValue}
      />
      <div class="text-xs opacity-80">Step: {currentIndex} / {Math.max(0, historyLength - 1)}</div>
      {#if replayTime !== null}
        <div class="text-xs opacity-80">Replay Time: {replayTime.toFixed(2)} days</div>
      {/if}
    </div>

    <div class="grid grid-cols-3 gap-2">
      <button type="button" class="btn btn-xs" onclick={onPrev} disabled={historyLength === 0}>Prev</button>
      <button type="button" class="btn btn-xs" onclick={onTogglePlay} disabled={historyLength === 0}>{playing ? 'Stop' : 'Play'}</button>
      <button type="button" class="btn btn-xs" onclick={onNext} disabled={historyLength === 0}>Next</button>
    </div>
    <label class="form-control">
      <span class="label-text text-xs">Playback Speed</span>
      <input type="number" min="0.1" step="0.1" class="input input-bordered input-sm w-full max-w-32" bind:value={playSpeed} />
    </label>

    <label class="label cursor-pointer justify-start gap-2">
      <input type="checkbox" class="checkbox checkbox-sm" bind:checked={showDebugState} />
      <span class="label-text text-sm">Show Raw Debug State</span>
    </label>
  </div>
</details>

<style>
  details[open] .collapse-chevron { transform: rotate(90deg); }
  .collapse-chevron { transition: transform 0.15s ease; display: inline-block; }
  details[open] .collapse-label-open { display: inline; }
  details[open] .collapse-label-closed { display: none; }
</style>
