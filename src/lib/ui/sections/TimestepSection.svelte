<script lang="ts">
  import Collapsible from "../controls/Collapsible.svelte";
  import Input from "../controls/Input.svelte";
  import {
    panelBodyClass,
    panelTableClass,
    panelTableHeadClass,
    panelTableShellClass,
  } from "../shared/panelStyles";

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
        key === "deltaT" ||
        key === "max_sat_change_per_step" ||
        key === "max_pressure_change_per_step" ||
        key === "max_well_rate_change_fraction",
    ),
  );
  const groupSummary = $derived(
    `Δt=${delta_t_days} d · max ΔS=${max_sat_change_per_step} · max ΔP=${max_pressure_change_per_step} bar`,
  );
</script>

<Collapsible title="Timestep Controls" {hasError}>
  <div class={panelBodyClass}>
    <div class="flex justify-between items-center mb-2">
      <p class="text-xs font-medium text-muted-foreground">
        Adjust timestep and run-size settings.
      </p>
      <p class="text-xs text-muted-foreground font-medium">{groupSummary}</p>
    </div>
    <div class={`${panelTableShellClass} mt-2`}>
      <table class={panelTableClass}>
        <thead class={panelTableHeadClass}>
          <tr>
            <th class="font-medium p-2">Δt (Days)</th>
            <th class="font-medium p-2">Max ΔS</th>
            <th class="font-medium p-2">Max ΔP (bar)</th>
            <th class="font-medium p-2">Max ΔRate (Rel)</th>
          </tr>
        </thead>
        <tbody class="divide-y divide-border">
          <tr>
            <td class="p-2 align-top"
              ><Input
                type="number"
                step="0.1"
                class={`w-full h-7 px-2 ${Boolean(fieldErrors.deltaT) ? "border-destructive" : ""}`}
                bind:value={delta_t_days}
              />
              {#if fieldErrors.deltaT}
                <div class="text-[10px] text-destructive mt-1 leading-tight">
                  {fieldErrors.deltaT}
                </div>
              {/if}
            </td>
            <td class="p-2 align-top"
              ><Input
                type="number"
                min="0.01"
                max="1"
                step="0.01"
                class={`w-full h-7 px-2 ${Boolean(fieldErrors.max_sat_change_per_step) ? "border-destructive" : ""}`}
                bind:value={max_sat_change_per_step}
              />
              {#if fieldErrors.max_sat_change_per_step}
                <div class="text-[10px] text-destructive mt-1 leading-tight">
                  {fieldErrors.max_sat_change_per_step}
                </div>
              {/if}
            </td>
            <td class="p-2 align-top"
              ><Input
                type="number"
                min="1"
                step="1"
                class={`w-full h-7 px-2 ${Boolean(fieldErrors.max_pressure_change_per_step) ? "border-destructive" : ""}`}
                bind:value={max_pressure_change_per_step}
              />
              {#if fieldErrors.max_pressure_change_per_step}
                <div class="text-[10px] text-destructive mt-1 leading-tight">
                  {fieldErrors.max_pressure_change_per_step}
                </div>
              {/if}
            </td>
            <td class="p-2 align-top"
              ><Input
                type="number"
                min="0.01"
                step="0.05"
                class={`w-full h-7 px-2 ${Boolean(fieldErrors.max_well_rate_change_fraction) ? "border-destructive" : ""}`}
                bind:value={max_well_rate_change_fraction}
              />
              {#if fieldErrors.max_well_rate_change_fraction}
                <div class="text-[10px] text-destructive mt-1 leading-tight">
                  {fieldErrors.max_well_rate_change_fraction}
                </div>
              {/if}
            </td>
          </tr>
        </tbody>
      </table>
    </div>
  </div>
</Collapsible>
