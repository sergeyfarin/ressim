<script lang="ts">
  import Collapsible from "../controls/Collapsible.svelte";
  import Input from "../controls/Input.svelte";
  import Select from "../controls/Select.svelte";

  let {
    analyticalMode = $bindable<"waterflood" | "depletion" | "none">("waterflood"),
    analyticalDepletionRateScale = $bindable(1.0),
    analyticalArpsB = $bindable(0.0),
    onAnalyticalModeChange = (_mode: "waterflood" | "depletion") => {},
  }: {
    analyticalMode?: "waterflood" | "depletion" | "none";
    analyticalDepletionRateScale?: number;
    analyticalArpsB?: number;
    onAnalyticalModeChange?: (mode: "waterflood" | "depletion") => void;
  } = $props();
</script>

<Collapsible title="Reference Solution">
  <div class="flex items-center gap-3 px-2.5 py-2">
    <Select class="h-6 text-xs px-1.5" bind:value={analyticalMode}
      onchange={() => { if (analyticalMode !== 'none') onAnalyticalModeChange(analyticalMode); }}>
      <option value="depletion">Depletion</option>
      <option value="waterflood">Buckley-Leverett</option>
    </Select>
    {#if analyticalMode === "depletion"}
      <label class="flex items-center gap-1 text-xs">
        <span class="text-[10px] text-muted-foreground">Rate scale</span>
        <Input type="number" min="0" step="0.01" class="w-16 h-6 px-1 text-xs" bind:value={analyticalDepletionRateScale} />
      </label>
      <label class="flex items-center gap-1 text-xs">
        <span class="text-[10px] text-muted-foreground">Arps b</span>
        <Input type="number" min="0" max="1" step="0.1" class="w-14 h-6 px-1 text-xs" bind:value={analyticalArpsB} />
      </label>
    {/if}
  </div>
</Collapsible>
