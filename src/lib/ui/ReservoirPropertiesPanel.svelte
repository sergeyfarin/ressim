<script lang="ts">
  export let initialPressure = 300;
  export let initialSaturation = 0.2;
  export let mu_w = 0.5;
  export let mu_o = 1.0;
  export let c_o = 1e-5;
  export let c_w = 3e-6;
  export let rho_w = 1000;
  export let rho_o = 800;
  export let rock_compressibility = 1e-6;
  export let depth_reference = 0;
  export let volume_expansion_o = 1;
  export let volume_expansion_w = 1;
  export let gravityEnabled = false;

  export let permMode: 'uniform' | 'random' | 'perLayer' = 'uniform';
  export let uniformPermX = 100;
  export let uniformPermY = 100;
  export let uniformPermZ = 10;

  export let minPerm = 50;
  export let maxPerm = 200;
  export let useRandomSeed = true;
  export let randomSeed = 12345;

  export let nz = 10;
  export let layerPermsX: number[] = [];
  export let layerPermsY: number[] = [];
  export let layerPermsZ: number[] = [];
  export let fieldErrors: Record<string, string> = {};

  $: permSummary =
    permMode === 'uniform'
      ? `Uniform ${uniformPermX}/${uniformPermY}/${uniformPermZ} mD`
      : permMode === 'random'
        ? `Random ${minPerm}-${maxPerm} mD`
        : `Per Layer (${nz} layers)`;
  $: hasError = Object.keys(fieldErrors).some((key) => key.includes('perm') || key.includes('saturation') || key.includes('initial'));
  $: groupSummary = `P=${initialPressure.toFixed(0)} bar · Sw=${initialSaturation.toFixed(2)} · μw/μo=${mu_w.toFixed(2)}/${mu_o.toFixed(2)} · ${permSummary}`;
</script>

<details class="rounded-lg border bg-base-100 shadow-sm" class:border-error={hasError} class:border-base-300={!hasError}>
  <summary class="flex cursor-pointer list-none items-center justify-between px-4 py-3 md:px-5">
    <div>
      <div class="font-semibold">Reservoir Properties</div>
      <div class="text-xs opacity-70">{groupSummary}</div>
    </div>
    <div class="flex items-center gap-2 text-xs opacity-70">
      <span class="collapse-label-open hidden">Collapse</span>
      <span class="collapse-label-closed">Expand</span>
      <span class="collapse-chevron">▸</span>
    </div>
  </summary>

  <div class="space-y-3 border-t border-base-300 p-4 md:p-5">
    <div class="grid grid-cols-2 gap-2">
      <label class="form-control">
        <span class="label-text text-xs">Pressure (bar)</span>
        <input type="number" step="10" class="input input-bordered input-sm w-full" bind:value={initialPressure} />
      </label>
      <label class="form-control">
        <span class="label-text text-xs">Water Saturation</span>
        <input type="number" min="0" max="1" step="0.05" class="input input-bordered input-sm w-full" class:input-error={Boolean(fieldErrors.initialSaturation)} bind:value={initialSaturation} />
      </label>
    </div>

    <div class="grid grid-cols-2 gap-2">
      <label class="form-control">
        <span class="label-text text-xs">Water Viscosity (cP)</span>
        <input type="number" min="0.01" step="0.01" class="input input-bordered input-sm w-full" bind:value={mu_w} />
      </label>
      <label class="form-control">
        <span class="label-text text-xs">Oil Viscosity (cP)</span>
        <input type="number" min="0.01" step="0.01" class="input input-bordered input-sm w-full" bind:value={mu_o} />
      </label>
      <label class="form-control">
        <span class="label-text text-xs">Water Compressibility (1/bar)</span>
        <input type="number" min="0" step="1e-6" class="input input-bordered input-sm w-full" bind:value={c_w} />
      </label>
      <label class="form-control">
        <span class="label-text text-xs">Oil Compressibility (1/bar)</span>
        <input type="number" min="0" step="1e-6" class="input input-bordered input-sm w-full" bind:value={c_o} />
      </label>
      <label class="form-control">
        <span class="label-text text-xs">Water Density (kg/m³)</span>
        <input type="number" min="1" step="1" class="input input-bordered input-sm w-full" bind:value={rho_w} />
      </label>
      <label class="form-control">
        <span class="label-text text-xs">Oil Density (kg/m³)</span>
        <input type="number" min="1" step="1" class="input input-bordered input-sm w-full" bind:value={rho_o} />
      </label>
      <label class="form-control">
        <span class="label-text text-xs">Rock Compressibility (1/bar)</span>
        <input type="number" min="0" step="1e-6" class="input input-bordered input-sm w-full" bind:value={rock_compressibility} />
      </label>
      <label class="form-control">
        <span class="label-text text-xs">Depth Reference (m)</span>
        <input type="number" step="1" class="input input-bordered input-sm w-full" bind:value={depth_reference} />
      </label>
      <label class="form-control">
        <span class="label-text text-xs">Oil Volume Expansion Factor (B_o)</span>
        <input type="number" min="0.01" step="0.01" class="input input-bordered input-sm w-full" bind:value={volume_expansion_o} />
      </label>
      <label class="form-control">
        <span class="label-text text-xs">Water Volume Expansion Factor (B_w)</span>
        <input type="number" min="0.01" step="0.01" class="input input-bordered input-sm w-full" bind:value={volume_expansion_w} />
      </label>
    </div>

    <label class="label cursor-pointer justify-start gap-2">
      <input type="checkbox" class="checkbox checkbox-sm" bind:checked={gravityEnabled} />
      <span class="label-text text-sm">Enable Gravity</span>
    </label>

    <label class="form-control">
      <span class="label-text text-xs">Permeability Mode</span>
      <select class="select select-bordered select-sm w-full" bind:value={permMode}>
        <option value="uniform">Uniform</option>
        <option value="random">Random</option>
        <option value="perLayer">Per Layer</option>
      </select>
    </label>

    {#if permMode === 'uniform'}
      <div class="grid grid-cols-3 gap-2">
        <label class="form-control">
          <span class="label-text text-xs">kX (mD)</span>
          <input type="number" min="1" class="input input-bordered input-sm w-full" bind:value={uniformPermX} />
        </label>
        <label class="form-control">
          <span class="label-text text-xs">kY (mD)</span>
          <input type="number" min="1" class="input input-bordered input-sm w-full" bind:value={uniformPermY} />
        </label>
        <label class="form-control">
          <span class="label-text text-xs">kZ (mD)</span>
          <input type="number" min="1" class="input input-bordered input-sm w-full" bind:value={uniformPermZ} />
        </label>
      </div>
    {:else if permMode === 'random'}
      <label class="label cursor-pointer justify-start gap-2">
        <input type="checkbox" class="checkbox checkbox-sm" bind:checked={useRandomSeed} />
        <span class="label-text text-sm">Use Seeded Randomness</span>
      </label>

      {#if useRandomSeed}
        <label class="form-control">
          <span class="label-text text-xs">Random Seed</span>
          <input type="number" step="1" class="input input-bordered input-sm w-full max-w-40" bind:value={randomSeed} />
        </label>
      {/if}

      <div class="grid grid-cols-2 gap-2">
        <label class="form-control">
          <span class="label-text text-xs">Min Permeability (mD)</span>
          <input type="number" min="1" class="input input-bordered input-sm w-full" class:input-error={Boolean(fieldErrors.permBounds)} bind:value={minPerm} />
        </label>
        <label class="form-control">
          <span class="label-text text-xs">Max Permeability (mD)</span>
          <input type="number" min="1" class="input input-bordered input-sm w-full" class:input-error={Boolean(fieldErrors.permBounds)} bind:value={maxPerm} />
        </label>
      </div>
    {:else}
      <div class="overflow-x-auto rounded-md border border-base-300">
        <table class="table table-xs w-full">
          <thead>
            <tr>
              <th>Layer</th>
              <th>kX (mD)</th>
              <th>kY (mD)</th>
              <th>kZ (mD)</th>
            </tr>
          </thead>
          <tbody>
            {#each Array.from({ length: nz }) as _, i}
              <tr>
                <td>{i + 1}</td>
                <td><input type="number" min="1" class="input input-bordered input-xs w-20" bind:value={layerPermsX[i]} /></td>
                <td><input type="number" min="1" class="input input-bordered input-xs w-20" bind:value={layerPermsY[i]} /></td>
                <td><input type="number" min="1" class="input input-bordered input-xs w-20" bind:value={layerPermsZ[i]} /></td>
              </tr>
            {/each}
          </tbody>
        </table>
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
