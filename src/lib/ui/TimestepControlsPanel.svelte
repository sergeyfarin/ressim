<script lang="ts">
  export let delta_t_days = 0.25;
  export let max_sat_change_per_step = 0.1;
  export let max_pressure_change_per_step = 75;
  export let max_well_rate_change_fraction = 0.75;
  export let fieldErrors: Record<string, string> = {};

  $: hasError = Object.keys(fieldErrors).some((key) => key.includes('well') || key.includes('pressure') || key.includes('saturation'));
  $: groupSummary = `Δt=${delta_t_days} d · max ΔS=${max_sat_change_per_step} · max ΔP=${max_pressure_change_per_step} bar`;
</script>

<details class="rounded-lg border bg-base-100 shadow-sm" class:border-error={hasError} class:border-base-300={!hasError}>
  <summary class="flex cursor-pointer list-none items-center justify-between px-4 py-3 md:px-5">
    <div>
      <div class="font-semibold">Timestep Controls</div>
      <div class="text-xs opacity-70">{groupSummary}</div>
    </div>
    <div class="flex items-center gap-2 text-xs opacity-70">
      <span class="collapse-label-open hidden">Collapse</span>
      <span class="collapse-label-closed">Expand</span>
      <span class="collapse-chevron">▸</span>
    </div>
  </summary>

  <div class="space-y-3 border-t border-base-300 p-4 md:p-5">
    <p class="text-xs opacity-70">Adjust timestep and run-size settings.</p>
    <div class="grid grid-cols-2 gap-2">
      <label class="form-control">
        <span class="label-text text-xs">Δt (Days)</span>
        <input type="number" step="0.1" class="input input-bordered input-sm w-full" class:input-error={Boolean(fieldErrors.deltaT)} bind:value={delta_t_days} />
      </label>
      <label class="form-control col-span-2 md:col-span-1">
        <span class="label-text text-xs">Max Saturation Change per Step</span>
        <input type="number" min="0.01" max="1" step="0.01" class="input input-bordered input-sm w-full max-w-40" class:input-error={Boolean(fieldErrors.saturationEndpoints)} bind:value={max_sat_change_per_step} />
      </label>
      <label class="form-control col-span-2 md:col-span-1">
        <span class="label-text text-xs">Max Pressure Change per Step (bar)</span>
        <input type="number" min="1" step="1" class="input input-bordered input-sm w-full max-w-40" class:input-error={Boolean(fieldErrors.wellPressureOrder)} bind:value={max_pressure_change_per_step} />
      </label>
      <label class="form-control col-span-2 md:col-span-1">
        <span class="label-text text-xs">Max Relative Well-Rate Change</span>
        <input type="number" min="0.01" step="0.05" class="input input-bordered input-sm w-full max-w-40" class:input-error={Boolean(fieldErrors.injectorRate) || Boolean(fieldErrors.producerRate)} bind:value={max_well_rate_change_fraction} />
      </label>
    </div>
  </div>
</details>

<style>
  details[open] .collapse-chevron { transform: rotate(90deg); }
  .collapse-chevron { transition: transform 0.15s ease; display: inline-block; }
  details[open] .collapse-label-open { display: inline; }
  details[open] .collapse-label-closed { display: none; }
</style>
