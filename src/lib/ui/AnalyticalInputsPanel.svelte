<script lang="ts">
  export let analyticalSolutionMode: 'waterflood' | 'depletion' = 'waterflood';
  export let analyticalDietzShapeFactor = 21.2;
  export let analyticalDepletionTauScale = 0.25;
  export let analyticalDepletionRateScale = 1.0;

  $: modeLabel = analyticalSolutionMode === 'depletion' ? 'Depletion' : 'Buckley-Leverett';
  $: summary = analyticalSolutionMode === 'depletion'
    ? `Mode=${modeLabel} · Shape=${analyticalDietzShapeFactor.toFixed(2)} · τ-scale=${analyticalDepletionTauScale.toFixed(3)}`
    : `Mode=${modeLabel}`;
</script>

<details class="rounded-lg border border-base-300 bg-base-100 shadow-sm">
  <summary class="flex cursor-pointer list-none items-center justify-between px-4 py-3 md:px-5">
    <div>
      <div class="font-semibold">Analytical Inputs</div>
      <div class="text-xs opacity-70">{summary}</div>
    </div>
    <div class="flex items-center gap-2 text-xs opacity-70">
      <span class="collapse-label-open hidden">Collapse</span>
      <span class="collapse-label-closed">Expand</span>
      <span class="collapse-chevron">▸</span>
    </div>
  </summary>

  <div class="space-y-3 border-t border-base-300 p-4 md:p-5">
    <label class="form-control">
      <span class="label-text text-xs">Analytical Model</span>
      <select class="select select-bordered select-sm w-full" bind:value={analyticalSolutionMode}>
        <option value="depletion">Depletion</option>
        <option value="waterflood">Buckley-Leverett</option>
      </select>
    </label>

    {#if analyticalSolutionMode === 'depletion'}
      <div class="grid grid-cols-1 gap-2">
        <label class="form-control">
          <span class="label-text text-xs">Dietz Shape Factor</span>
          <input
            type="number"
            min="0.001"
            step="0.1"
            class="input input-bordered input-sm w-full"
            bind:value={analyticalDietzShapeFactor}
          />
        </label>

        <label class="form-control">
          <span class="label-text text-xs">Depletion τ Scale</span>
          <input
            type="number"
            min="0.000001"
            step="0.01"
            class="input input-bordered input-sm w-full"
            bind:value={analyticalDepletionTauScale}
          />
        </label>

        <label class="form-control">
          <span class="label-text text-xs">Rate Scale</span>
          <input
            type="number"
            min="0"
            step="0.01"
            class="input input-bordered input-sm w-full"
            bind:value={analyticalDepletionRateScale}
          />
        </label>
      </div>
      <div class="text-[11px] opacity-70">
        Conversion factors are intentionally left as tunable placeholders in the analytical depletion component.
      </div>
    {/if}
  </div>
</details>

<style>
  details[open] .collapse-chevron { transform: rotate(90deg); }
  .collapse-chevron { transition: transform 0.15s ease; display: inline-block; }
  details[open] .collapse-label-open { display: inline; }
  details[open] .collapse-label-closed { display: none; }
</style>
