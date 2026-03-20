<script lang="ts">
  import Collapsible from "../controls/Collapsible.svelte";
  import Input from "../controls/Input.svelte";
  import Select from "../controls/Select.svelte";

  let {
    analyticalSolutionMode = $bindable<"waterflood" | "depletion">("waterflood"),
    analyticalDepletionRateScale = $bindable(1.0),
    onAnalyticalSolutionModeChange = (_mode: "waterflood" | "depletion") => {},
  }: {
    analyticalSolutionMode?: "waterflood" | "depletion";
    analyticalDepletionRateScale?: number;
    onAnalyticalSolutionModeChange?: (mode: "waterflood" | "depletion") => void;
  } = $props();
</script>

<Collapsible title="Reference Solution">
  <div class="flex items-center gap-3 px-2.5 py-2">
    <Select class="h-7 text-xs px-1.5" bind:value={analyticalSolutionMode}
      onchange={() => onAnalyticalSolutionModeChange(analyticalSolutionMode)}>
      <option value="depletion">Depletion</option>
      <option value="waterflood">Buckley-Leverett</option>
    </Select>
    {#if analyticalSolutionMode === "depletion"}
      <label class="flex items-center gap-1 text-xs">
        <span class="text-[10px] text-muted-foreground">Rate scale</span>
        <Input type="number" min="0" step="0.01" class="w-16 h-7 px-1.5 text-xs" bind:value={analyticalDepletionRateScale} />
      </label>
    {/if}
  </div>
</Collapsible>
