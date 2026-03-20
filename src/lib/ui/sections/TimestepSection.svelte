<script lang="ts">
  import Collapsible from "../controls/Collapsible.svelte";
  import ValidatedInput from "../controls/ValidatedInput.svelte";

  let {
    delta_t_days = $bindable(0.25),
    max_sat_change_per_step = $bindable(0.1),
    max_pressure_change_per_step = $bindable(75),
    max_well_rate_change_fraction = $bindable(0.75),
    onDeltaTDaysEdit = () => {},
    fieldErrors = {},
  }: {
    delta_t_days?: number;
    max_sat_change_per_step?: number;
    max_pressure_change_per_step?: number;
    max_well_rate_change_fraction?: number;
    onDeltaTDaysEdit?: () => void;
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
</script>

<Collapsible title="Timestep & Stability" {hasError}>
  <div class="px-2.5 py-2">
    <div class="overflow-x-auto rounded-md border border-border">
      <table class="compact-table w-full text-left">
        <thead class="border-b border-border bg-muted/50 text-[10px] uppercase tracking-wide text-muted-foreground">
          <tr>
            <th class="px-2 py-1 font-medium">Δt (d)</th>
            <th class="px-2 py-1 font-medium">Max ΔS</th>
            <th class="px-2 py-1 font-medium">Max ΔP (bar)</th>
            <th class="px-2 py-1 font-medium">Max ΔQ (rel)</th>
          </tr>
        </thead>
        <tbody>
          <tr>
            <td class="px-1 py-1">
              <ValidatedInput type="number" step="0.1" class="w-full h-7 px-2 text-xs" bind:value={delta_t_days} error={fieldErrors.deltaT} oninput={onDeltaTDaysEdit} />
            </td>
            <td class="px-1 py-1">
              <ValidatedInput type="number" min="0.01" max="1" step="0.01" class="w-full h-7 px-2 text-xs" bind:value={max_sat_change_per_step} error={fieldErrors.max_sat_change_per_step} />
            </td>
            <td class="px-1 py-1">
              <ValidatedInput type="number" min="1" step="1" class="w-full h-7 px-2 text-xs" bind:value={max_pressure_change_per_step} error={fieldErrors.max_pressure_change_per_step} />
            </td>
            <td class="px-1 py-1">
              <ValidatedInput type="number" min="0.01" step="0.05" class="w-full h-7 px-2 text-xs" bind:value={max_well_rate_change_fraction} error={fieldErrors.max_well_rate_change_fraction} />
            </td>
          </tr>
        </tbody>
      </table>
    </div>
  </div>
</Collapsible>
