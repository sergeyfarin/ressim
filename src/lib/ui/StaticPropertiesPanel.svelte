<script lang="ts">
  let {
    nx = $bindable(15),
    ny = $bindable(10),
    nz = $bindable(10),
    cellDx = $bindable(10),
    cellDy = $bindable(10),
    cellDz = $bindable(1),
  }: {
    nx?: number;
    ny?: number;
    nz?: number;
    cellDx?: number;
    cellDy?: number;
    cellDz?: number;
  } = $props();

  const modelSizeX = $derived(nx * cellDx);
  const modelSizeY = $derived(ny * cellDy);
  const modelSizeZ = $derived(nz * cellDz);
  const groupSummary = $derived(`${nx}×${ny}×${nz} · ${modelSizeX}×${modelSizeY}×${modelSizeZ} m`);
</script>

<details class="rounded-lg border border-base-300 bg-base-100 shadow-sm">
  <summary class="flex cursor-pointer list-none items-center justify-between px-4 py-3 md:px-5">
    <div>
      <div class="font-semibold">Grid Parameters</div>
      <div class="text-xs opacity-70">{groupSummary}</div>
    </div>
    <div class="flex items-center gap-2 text-xs opacity-70">
      <span class="collapse-label-open hidden">Collapse</span>
      <span class="collapse-label-closed">Expand</span>
      <span class="collapse-chevron">▸</span>
    </div>
  </summary>
  <div class="space-y-3 border-t border-base-300 p-4 md:p-5">
    <p class="text-xs opacity-70">Grid dimensions and physical model extent.</p>

    <div class="grid grid-cols-3 gap-2">
      <label class="form-control">
        <span class="label-text text-xs">nx</span>
        <input type="number" min="1" class="input input-bordered input-sm w-full" bind:value={nx} />
      </label>
      <label class="form-control">
        <span class="label-text text-xs">ny</span>
        <input type="number" min="1" class="input input-bordered input-sm w-full" bind:value={ny} />
      </label>
      <label class="form-control">
        <span class="label-text text-xs">nz</span>
        <input type="number" min="1" class="input input-bordered input-sm w-full" bind:value={nz} />
      </label>
    </div>

    <div class="grid grid-cols-3 gap-2">
      <label class="form-control">
        <span class="label-text text-xs">Δx (m)</span>
        <input type="number" min="0.1" step="0.1" class="input input-bordered input-sm w-full" bind:value={cellDx} />
      </label>
      <label class="form-control">
        <span class="label-text text-xs">Δy (m)</span>
        <input type="number" min="0.1" step="0.1" class="input input-bordered input-sm w-full" bind:value={cellDy} />
      </label>
      <label class="form-control">
        <span class="label-text text-xs">Δz (m)</span>
        <input type="number" min="0.1" step="0.1" class="input input-bordered input-sm w-full" bind:value={cellDz} />
      </label>
    </div>

    <div class="rounded-md border border-base-300 bg-base-200/40 p-2 text-xs">
      <div>Model Size (m): {modelSizeX} × {modelSizeY} × {modelSizeZ}</div>
    </div>
  </div>
</details>

<style>
  details[open] .collapse-chevron { transform: rotate(90deg); }
  .collapse-chevron { transition: transform 0.15s ease; display: inline-block; }
  details[open] .collapse-label-open { display: inline; }
  details[open] .collapse-label-closed { display: none; }
</style>
