<script lang="ts">
  let {
    nx = $bindable(15),
    ny = $bindable(10),
    nz = $bindable(10),
    cellDx = $bindable(10),
    cellDy = $bindable(10),
    cellDz = $bindable(1),
    onNzOrPermModeChange = () => {},
  }: {
    nx?: number;
    ny?: number;
    nz?: number;
    cellDx?: number;
    cellDy?: number;
    cellDz?: number;
    onNzOrPermModeChange?: () => void;
  } = $props();

  const modelSizeX = $derived(nx * cellDx);
  const modelSizeY = $derived(ny * cellDy);
  const modelSizeZ = $derived(nz * cellDz);
  const groupSummary = $derived(
    `${nx}×${ny}×${nz} · ${modelSizeX}×${modelSizeY}×${modelSizeZ} m`,
  );

  function handleTotalXChange(e: Event) {
    const val = parseFloat((e.target as HTMLInputElement).value);
    if (!isNaN(val) && nx > 0) cellDx = parseFloat((val / nx).toFixed(3));
  }

  function handleTotalYChange(e: Event) {
    const val = parseFloat((e.target as HTMLInputElement).value);
    if (!isNaN(val) && ny > 0) cellDy = parseFloat((val / ny).toFixed(3));
  }

  function handleTotalZChange(e: Event) {
    const val = parseFloat((e.target as HTMLInputElement).value);
    if (!isNaN(val) && nz > 0) cellDz = parseFloat((val / nz).toFixed(3));
  }
</script>

<details class="rounded-lg border border-base-300 bg-base-100 shadow-sm">
  <summary
    class="flex cursor-pointer list-none items-center justify-between px-4 py-3 md:px-5"
  >
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

    <div class="overflow-x-auto rounded-md border border-base-300">
      <table class="table table-xs compact-table w-full">
        <thead>
          <tr class="bg-base-200/50">
            <th>Dimension</th>
            <th>Cells (n)</th>
            <th>Cell Size (Δ, m)</th>
            <th>Total Length (m)</th>
          </tr>
        </thead>
        <tbody>
          <tr>
            <td class="font-semibold text-center align-middle">X</td>
            <td
              ><input
                type="number"
                min="1"
                class="input input-bordered input-xs w-full max-w-24"
                bind:value={nx}
              /></td
            >
            <td
              ><input
                type="number"
                min="0.1"
                step="0.1"
                class="input input-bordered input-xs w-full max-w-24"
                bind:value={cellDx}
              /></td
            >
            <td
              ><input
                type="number"
                min="0.1"
                step="0.1"
                class="input input-bordered input-xs w-full max-w-24"
                value={modelSizeX}
                oninput={handleTotalXChange}
              /></td
            >
          </tr>
          <tr>
            <td class="font-semibold text-center align-middle">Y</td>
            <td
              ><input
                type="number"
                min="1"
                class="input input-bordered input-xs w-full max-w-24"
                bind:value={ny}
              /></td
            >
            <td
              ><input
                type="number"
                min="0.1"
                step="0.1"
                class="input input-bordered input-xs w-full max-w-24"
                bind:value={cellDy}
              /></td
            >
            <td
              ><input
                type="number"
                min="0.1"
                step="0.1"
                class="input input-bordered input-xs w-full max-w-24"
                value={modelSizeY}
                oninput={handleTotalYChange}
              /></td
            >
          </tr>
          <tr>
            <td class="font-semibold text-center align-middle">Z</td>
            <td
              ><input
                type="number"
                min="1"
                class="input input-bordered input-xs w-full max-w-24"
                bind:value={nz}
                oninput={onNzOrPermModeChange}
              /></td
            >
            <td
              ><input
                type="number"
                min="0.1"
                step="0.1"
                class="input input-bordered input-xs w-full max-w-24"
                bind:value={cellDz}
              /></td
            >
            <td
              ><input
                type="number"
                min="0.1"
                step="0.1"
                class="input input-bordered input-xs w-full max-w-24"
                value={modelSizeZ}
                oninput={handleTotalZChange}
              /></td
            >
          </tr>
        </tbody>
      </table>
    </div>
  </div>
</details>

<style>
  details[open] .collapse-chevron {
    transform: rotate(90deg);
  }
  .collapse-chevron {
    transition: transform 0.15s ease;
    display: inline-block;
  }
  details[open] .collapse-label-open {
    display: inline;
  }
  details[open] .collapse-label-closed {
    display: none;
  }
</style>
