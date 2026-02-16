<script lang="ts">
  export let well_radius = 0.1;
  export let well_skin = 0;
  export let nx = 15;
  export let ny = 10;
  export let injectorEnabled = true;
  export let injectorControlMode: 'rate' | 'pressure' = 'pressure';
  export let producerControlMode: 'rate' | 'pressure' = 'pressure';
  export let injectorBhp = 400;
  export let producerBhp = 100;
  export let targetInjectorRate = 350;
  export let targetProducerRate = 350;
  export let fieldErrors: Record<string, string> = {};

  export let injectorI = 0;
  export let injectorJ = 0;
  export let producerI = 14;
  export let producerJ = 0;

  function inBounds(i: number, j: number) {
    return i >= 0 && i < nx && j >= 0 && j < ny;
  }

  $: injectorValid = inBounds(injectorI, injectorJ);
  $: producerValid = inBounds(producerI, producerJ);
  $: hasError = injectorValid === false || producerValid === false || Object.keys(fieldErrors).some((key) => key.includes('well') || key.includes('injector') || key.includes('producer'));
  $: groupSummary = `Inj(${injectorI},${injectorJ}) ${injectorEnabled ? 'on' : 'off'} · Prod(${producerI},${producerJ}) · r=${well_radius.toFixed(2)} m`;
</script>

<details class="rounded-lg border bg-base-100 shadow-sm" class:border-error={hasError} class:border-base-300={!hasError}>
  <summary class="flex cursor-pointer list-none items-center justify-between px-4 py-3 md:px-5">
    <div>
      <div class="font-semibold">Well Controls</div>
      <div class="text-xs opacity-70">{groupSummary}</div>
    </div>
    <div class="flex items-center gap-2 text-xs opacity-70">
      <span class="collapse-label-open hidden">Collapse</span>
      <span class="collapse-label-closed">Expand</span>
      <span class="collapse-chevron">▸</span>
    </div>

  </summary>

  <div class="space-y-3 border-t border-base-300 p-4 md:p-5">
    <p class="text-xs opacity-70">Well geometry and XY wellhead locations across all layers.</p>
    <div class="grid grid-cols-2 gap-2">
      <label class="form-control">
        <span class="label-text text-xs">Well Radius (m)</span>
        <input type="number" min="0.01" step="0.01" class="input input-bordered input-sm w-full" class:input-error={Boolean(fieldErrors.wellRadius)} bind:value={well_radius} />
      </label>
      <label class="form-control">
        <span class="label-text text-xs">Skin</span>
        <input type="number" step="0.1" class="input input-bordered input-sm w-full" bind:value={well_skin} />
      </label>
    </div>

    <label class="label cursor-pointer justify-start gap-2">
      <input type="checkbox" class="checkbox checkbox-sm" bind:checked={injectorEnabled} />
      <span class="label-text text-sm">Enable Injector (disable for depletion case)</span>
    </label>

    <div class="grid grid-cols-2 gap-2">
      <label class="form-control">
        <span class="label-text text-xs">Injector Control</span>
        <select class="select select-bordered select-sm w-full" bind:value={injectorControlMode}>
          <option value="pressure">Pressure (BHP)</option>
          <option value="rate">Rate</option>
        </select>
      </label>
      <label class="form-control">
        <span class="label-text text-xs">Producer Control</span>
        <select class="select select-bordered select-sm w-full" bind:value={producerControlMode}>
          <option value="pressure">Pressure (BHP)</option>
          <option value="rate">Rate</option>
        </select>
      </label>
    </div>

    <div class="grid grid-cols-2 gap-2">
      <label class="form-control">
        <span class="label-text text-xs">Injector BHP (bar)</span>
        <input type="number" step="1" class="input input-bordered input-sm w-full" class:input-error={Boolean(fieldErrors.wellPressureOrder)} bind:value={injectorBhp} disabled={injectorControlMode === 'rate'} />
      </label>
      <label class="form-control">
        <span class="label-text text-xs">Producer BHP (bar)</span>
        <input type="number" step="1" class="input input-bordered input-sm w-full" class:input-error={Boolean(fieldErrors.wellPressureOrder)} bind:value={producerBhp} disabled={producerControlMode === 'rate'} />
      </label>
      <label class="form-control">
        <span class="label-text text-xs">Injector Rate (m³/day)</span>
        <input type="number" min="0" step="1" class="input input-bordered input-sm w-full" class:input-error={Boolean(fieldErrors.injectorRate)} bind:value={targetInjectorRate} disabled={injectorControlMode === 'pressure' || !injectorEnabled} />
      </label>
      <label class="form-control">
        <span class="label-text text-xs">Producer Rate (m³/day)</span>
        <input type="number" min="0" step="1" class="input input-bordered input-sm w-full" class:input-error={Boolean(fieldErrors.producerRate)} bind:value={targetProducerRate} disabled={producerControlMode === 'pressure'} />
      </label>
    </div>

    <div class="rounded-md border border-base-300 p-2 text-xs">
      Valid i range: 0–{Math.max(0, nx - 1)} · Valid j range: 0–{Math.max(0, ny - 1)}
    </div>

    <div class="grid grid-cols-2 gap-2">
      <label class="form-control">
        <span class="label-text text-xs">Injector i</span>
        <input type="number" min="0" max={Math.max(0, nx - 1)} step="1" class="input input-bordered input-sm w-full" class:input-error={!injectorValid || Boolean(fieldErrors.wellOverlap)} bind:value={injectorI} />
      </label>
      <label class="form-control">
        <span class="label-text text-xs">Injector j</span>
        <input type="number" min="0" max={Math.max(0, ny - 1)} step="1" class="input input-bordered input-sm w-full" class:input-error={!injectorValid || Boolean(fieldErrors.wellOverlap)} bind:value={injectorJ} />
      </label>
      <label class="form-control">
        <span class="label-text text-xs">Producer i</span>
        <input type="number" min="0" max={Math.max(0, nx - 1)} step="1" class="input input-bordered input-sm w-full" class:input-error={!producerValid || Boolean(fieldErrors.wellOverlap)} bind:value={producerI} />
      </label>
      <label class="form-control">
        <span class="label-text text-xs">Producer j</span>
        <input type="number" min="0" max={Math.max(0, ny - 1)} step="1" class="input input-bordered input-sm w-full" class:input-error={!producerValid || Boolean(fieldErrors.wellOverlap)} bind:value={producerJ} />
      </label>
    </div>

    <div class="text-xs">
      <span class:opacity-70={injectorValid} class:text-error={!injectorValid}>Injector: {injectorValid ? 'valid' : 'out of bounds'}</span>
      <span class="mx-2">·</span>
      <span class:opacity-70={producerValid} class:text-error={!producerValid}>Producer: {producerValid ? 'valid' : 'out of bounds'}</span>
    </div>
  </div>
</details>

<style>
  details[open] .collapse-chevron { transform: rotate(90deg); }
  .collapse-chevron { transition: transform 0.15s ease; display: inline-block; }
  details[open] .collapse-label-open { display: inline; }
  details[open] .collapse-label-closed { display: none; }
</style>
