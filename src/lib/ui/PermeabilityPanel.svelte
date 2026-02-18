<script lang="ts">
  let {
    permMode = $bindable('default'),
    useRandomSeed = $bindable(true),
    randomSeed = $bindable(12345),
    minPerm = $bindable(50),
    maxPerm = $bindable(200),
    layerPermsXStr = $bindable(''),
    layerPermsYStr = $bindable(''),
    layerPermsZStr = $bindable(''),
  }: {
    permMode?: string;
    useRandomSeed?: boolean;
    randomSeed?: number;
    minPerm?: number;
    maxPerm?: number;
    layerPermsXStr?: string;
    layerPermsYStr?: string;
    layerPermsZStr?: string;
  } = $props();
</script>

<section class="card border border-base-300 bg-base-100 shadow-sm">
  <div class="card-body space-y-3 p-4 md:p-5">
    <h2 class="card-title text-base">Permeability</h2>
    <p class="-mt-2 text-xs opacity-70">Distribution controls for permeability fields.</p>
    <label class="form-control">
      <span class="label-text text-xs">Permeability Mode</span>
      <select class="select select-bordered select-sm w-full" bind:value={permMode}>
        <option value="default">Default</option>
        <option value="random">Random</option>
        <option value="perLayer">Per Layer</option>
      </select>
    </label>

    {#if permMode === 'random'}
      <label class="label cursor-pointer justify-start gap-2">
        <input type="checkbox" class="checkbox checkbox-sm" bind:checked={useRandomSeed} />
        <span class="label-text text-sm">Use Seeded Randomness</span>
      </label>
      <div class="grid grid-cols-2 gap-2">
        {#if useRandomSeed}
          <label class="form-control col-span-2">
            <span class="label-text text-xs">Random Seed</span>
            <input type="number" step="1" class="input input-bordered input-sm w-full max-w-40" bind:value={randomSeed} />
          </label>
        {/if}
        <label class="form-control">
          <span class="label-text text-xs">Min Permeability (mD)</span>
          <input type="number" class="input input-bordered input-sm w-full" bind:value={minPerm} />
        </label>
        <label class="form-control">
          <span class="label-text text-xs">Max Permeability (mD)</span>
          <input type="number" class="input input-bordered input-sm w-full" bind:value={maxPerm} />
        </label>
      </div>
    {:else if permMode === 'perLayer'}
      <label class="form-control">
        <span class="label-text text-xs">Permeability X by Layer (CSV)</span>
        <input type="text" class="input input-bordered input-sm w-full" bind:value={layerPermsXStr} />
      </label>
      <label class="form-control">
        <span class="label-text text-xs">Permeability Y by Layer (CSV)</span>
        <input type="text" class="input input-bordered input-sm w-full" bind:value={layerPermsYStr} />
      </label>
      <label class="form-control">
        <span class="label-text text-xs">Permeability Z by Layer (CSV)</span>
        <input type="text" class="input input-bordered input-sm w-full" bind:value={layerPermsZStr} />
      </label>
    {/if}
  </div>
</section>
