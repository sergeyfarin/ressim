<script lang="ts">
  import Collapsible from "../../components/ui/Collapsible.svelte";
  import Input from "../../components/ui/Input.svelte";
  import Select from "../../components/ui/Select.svelte";
  import { panelBodyClass } from "../shared/panelStyles";

  let {
    analyticalSolutionMode = $bindable<"waterflood" | "depletion">(
      "waterflood",
    ),
    analyticalDepletionRateScale = $bindable(1.0),
    onAnalyticalSolutionModeChange = (_mode: "waterflood" | "depletion") => {},
  }: {
    analyticalSolutionMode?: "waterflood" | "depletion";
    analyticalDepletionRateScale?: number;
    onAnalyticalSolutionModeChange?: (mode: "waterflood" | "depletion") => void;
  } = $props();

  const modeLabel = $derived(
    analyticalSolutionMode === "depletion" ? "Depletion" : "Buckley-Leverett",
  );
  const summary = $derived(
    analyticalSolutionMode === "depletion"
      ? `Mode=${modeLabel} · Rate scale=${analyticalDepletionRateScale.toFixed(2)}`
      : `Mode=${modeLabel}`,
  );
</script>

<Collapsible title="Analytical Inputs">
  <div class={panelBodyClass}>
    <div class="text-xs opacity-70 mb-2">{summary}</div>
    <div class="grid grid-cols-2 gap-2 items-end mb-2">
      <label class="flex flex-col gap-1.5">
        <span class="text-xs font-medium">Analytical Model</span>
        <Select
          class="w-full"
          bind:value={analyticalSolutionMode}
          onchange={() =>
            onAnalyticalSolutionModeChange(analyticalSolutionMode)}
        >
          <option value="depletion">Depletion</option>
          <option value="waterflood">Buckley-Leverett</option>
        </Select>
      </label>
      {#if analyticalSolutionMode === "depletion"}
        <label class="flex flex-col gap-1.5">
          <span class="text-xs font-medium">Rate Scale</span>
          <Input
            type="number"
            min="0"
            step="0.01"
            class="w-full"
            bind:value={analyticalDepletionRateScale}
          />
        </label>
      {/if}
    </div>

    {#if analyticalSolutionMode === "depletion"}
      <div class="text-[11px] text-muted-foreground mt-2">
        Pseudo-steady-state depletion: q(t)&nbsp;=&nbsp;J_oil·ΔP·exp(−t/τ),
        τ&nbsp;=&nbsp;V_pore·c_t/J_oil. J_oil is computed from the Peaceman well
        model using reservoir/well parameters. Rate scale multiplies J_oil for
        manual calibration (default 1.0).
      </div>
    {/if}
  </div>
</Collapsible>
