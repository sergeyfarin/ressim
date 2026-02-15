<script lang="ts">
  export let delta_t_days = 0.25;
  export let max_sat_change_per_step = 0.1;

  $: groupSummary = `Δt=${delta_t_days} d · max ΔS=${max_sat_change_per_step}`;
</script>

<details class="rounded-lg border border-base-300 bg-base-100 shadow-sm">
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
        <input type="number" step="0.1" class="input input-bordered input-sm w-full" bind:value={delta_t_days} />
      </label>
      <label class="form-control col-span-2 md:col-span-1">
        <span class="label-text text-xs">Max Saturation Change per Step</span>
        <input type="number" min="0.01" max="1" step="0.01" class="input input-bordered input-sm w-full max-w-40" bind:value={max_sat_change_per_step} />
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
