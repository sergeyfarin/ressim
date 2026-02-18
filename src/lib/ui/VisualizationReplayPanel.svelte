<script lang="ts">
  type ShowProperty = 'pressure' | 'saturation_water' | 'saturation_oil' | 'permeability_x' | 'permeability_y' | 'permeability_z' | 'porosity';

  let {
    showProperty = $bindable<ShowProperty>('pressure'),
    legendRangeMode = $bindable<'fixed' | 'percentile'>('percentile'),
    legendPercentileLow = $bindable(5),
    legendPercentileHigh = $bindable(95),
    legendFixedMin = $bindable(0),
    legendFixedMax = $bindable(1),
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
    legendRangeMode?: 'fixed' | 'percentile';
    legendPercentileLow?: number;
    legendPercentileHigh?: number;
    legendFixedMin?: number;
    legendFixedMax?: number;
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
    if (legendRangeMode === 'fixed') {
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
    } else {
    const low = Number(legendPercentileLow);
    const high = Number(legendPercentileHigh);
    legendPercentileLow = Number.isFinite(low) ? Math.max(0, Math.min(99, Math.round(low))) : 5;
    legendPercentileHigh = Number.isFinite(high) ? Math.max(1, Math.min(100, Math.round(high))) : 95;
    if (legendPercentileLow >= legendPercentileHigh) {
      legendPercentileHigh = Math.min(100, legendPercentileLow + 1);
    }
    }
  });

  function applySliderValue(event: Event) {
    const input = event.currentTarget as HTMLInputElement;
    currentIndex = Number(input.value);
    onApplyHistoryIndex(currentIndex);
  }

  const groupSummary = $derived(`${showProperty.replace('_', ' ')} · ${legendRangeMode} · step ${Math.max(0, currentIndex)}`);
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
      <select class="select select-bordered select-sm w-full" bind:value={showProperty}>
        <option value="pressure">Pressure</option>
        <option value="saturation_water">Water Saturation</option>
        <option value="saturation_oil">Oil Saturation</option>
        <option value="permeability_x">Permeability X</option>
        <option value="permeability_y">Permeability Y</option>
        <option value="permeability_z">Permeability Z</option>
        <option value="porosity">Porosity</option>
      </select>
    </label>

    <label class="form-control">
      <span class="label-text text-xs">Legend Range Mode</span>
      <select class="select select-bordered select-sm w-full" bind:value={legendRangeMode}>
        <option value="percentile">Percentile (adaptive)</option>
        <option value="fixed">Fixed</option>
      </select>
    </label>

    {#if legendRangeMode === 'percentile'}
      <div class="grid grid-cols-2 gap-2">
        <label class="form-control">
          <span class="label-text text-xs">Low Percentile (%)</span>
          <input type="number" min="0" max="99" step="1" class="input input-bordered input-sm w-full" bind:value={legendPercentileLow} />
        </label>
        <label class="form-control">
          <span class="label-text text-xs">High Percentile (%)</span>
          <input type="number" min="1" max="100" step="1" class="input input-bordered input-sm w-full" bind:value={legendPercentileHigh} />
        </label>
      </div>
    {:else}
      <div class="grid grid-cols-2 gap-2">
        <label class="form-control">
          <span class="label-text text-xs">Fixed Min</span>
          <input type="number" step="any" class="input input-bordered input-sm w-full" bind:value={legendFixedMin} />
        </label>
        <label class="form-control">
          <span class="label-text text-xs">Fixed Max</span>
          <input type="number" step="any" class="input input-bordered input-sm w-full" bind:value={legendFixedMax} />
        </label>
      </div>
    {/if}

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
