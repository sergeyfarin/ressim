<script lang="ts">
  let {
    delta_t_days = $bindable(0.25),
    max_sat_change_per_step = $bindable(0.1),
    max_pressure_change_per_step = $bindable(75),
    max_well_rate_change_fraction = $bindable(0.75),
    fieldErrors = {},
  }: {
    delta_t_days?: number;
    max_sat_change_per_step?: number;
    max_pressure_change_per_step?: number;
    max_well_rate_change_fraction?: number;
    fieldErrors?: Record<string, string>;
  } = $props();

  const hasError = $derived(
    Object.keys(fieldErrors).some(
      (key) =>
        key.includes("well") ||
        key.includes("pressure") ||
        key.includes("saturation"),
    ),
  );
  const groupSummary = $derived(
    `Δt=${delta_t_days} d · max ΔS=${max_sat_change_per_step} · max ΔP=${max_pressure_change_per_step} bar`,
  );
</script>

<details
  class="rounded-lg border bg-base-100 shadow-sm"
  class:border-error={hasError}
  class:border-base-300={!hasError}
>
  <summary
    class="flex cursor-pointer list-none items-center justify-between px-4 py-3 md:px-5"
  >
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
    <div class="overflow-x-auto rounded-md border border-base-300 mt-2">
      <table class="table table-xs w-full">
        <thead>
          <tr class="bg-base-200/50">
            <th>Δt (Days)</th>
            <th>Max ΔS</th>
            <th>Max ΔP (bar)</th>
            <th>Max ΔRate (Rel)</th>
          </tr>
        </thead>
        <tbody>
          <tr>
            <td
              ><input
                type="number"
                step="0.1"
                class="input input-bordered input-xs w-full min-w-16 max-w-24"
                class:input-error={Boolean(fieldErrors.deltaT)}
                bind:value={delta_t_days}
              /></td
            >
            <td
              ><input
                type="number"
                min="0.01"
                max="1"
                step="0.01"
                class="input input-bordered input-xs w-full min-w-16 max-w-24"
                class:input-error={Boolean(fieldErrors.saturationEndpoints)}
                bind:value={max_sat_change_per_step}
              /></td
            >
            <td
              ><input
                type="number"
                min="1"
                step="1"
                class="input input-bordered input-xs w-full min-w-16 max-w-24"
                class:input-error={Boolean(fieldErrors.wellPressureOrder)}
                bind:value={max_pressure_change_per_step}
              /></td
            >
            <td
              ><input
                type="number"
                min="0.01"
                step="0.05"
                class="input input-bordered input-xs w-full min-w-16 max-w-24"
                class:input-error={Boolean(fieldErrors.injectorRate) ||
                  Boolean(fieldErrors.producerRate)}
                bind:value={max_well_rate_change_fraction}
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
