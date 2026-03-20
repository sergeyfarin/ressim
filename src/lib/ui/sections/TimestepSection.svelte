<script lang="ts">
  import Collapsible from "../controls/Collapsible.svelte";
  import PanelTable from "../controls/PanelTable.svelte";
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
  const groupSummary = $derived(
    `Δt=${delta_t_days} d · max ΔS=${max_sat_change_per_step} · max ΔP=${max_pressure_change_per_step} bar`,
  );
</script>

<Collapsible title="Timestep Controls" {hasError}>
  <div class="space-y-2 p-3">
    <p class="text-[11px] text-muted-foreground">{groupSummary}</p>
    <PanelTable columns={["Δt (Days)", "Max ΔS", "Max ΔP (bar)", "Max ΔRate (Rel)"]}>
      <tr>
        <td class="p-2 align-top">
          <ValidatedInput type="number" step="0.1" class="w-full h-7 px-2" bind:value={delta_t_days} error={fieldErrors.deltaT} oninput={onDeltaTDaysEdit} />
        </td>
        <td class="p-2 align-top">
          <ValidatedInput type="number" min="0.01" max="1" step="0.01" class="w-full h-7 px-2" bind:value={max_sat_change_per_step} error={fieldErrors.max_sat_change_per_step} />
        </td>
        <td class="p-2 align-top">
          <ValidatedInput type="number" min="1" step="1" class="w-full h-7 px-2" bind:value={max_pressure_change_per_step} error={fieldErrors.max_pressure_change_per_step} />
        </td>
        <td class="p-2 align-top">
          <ValidatedInput type="number" min="0.01" step="0.05" class="w-full h-7 px-2" bind:value={max_well_rate_change_fraction} error={fieldErrors.max_well_rate_change_fraction} />
        </td>
      </tr>
    </PanelTable>
  </div>
</Collapsible>
