<script lang="ts">
  import Collapsible from "../controls/Collapsible.svelte";
  import Input from "../controls/Input.svelte";
  import ToggleGroup from "../controls/ToggleGroup.svelte";
  import ValidatedInput from "../controls/ValidatedInput.svelte";
  import { FLUID_PRESETS } from "../../catalog/reservoirPresets";
  import type { ModePanelParameterBindings } from "../modePanelTypes";

  let {
    bindings,
    fieldErrors = {},
    onParamEdit = () => {},
  }: {
    bindings: ModePanelParameterBindings;
    fieldErrors?: Record<string, string>;
    onParamEdit?: () => void;
  } = $props();

  const hasError = $derived(
    !!fieldErrors.mu_w ||
    !!fieldErrors.mu_o ||
    !!fieldErrors.c_o ||
    !!fieldErrors.c_w ||
    !!fieldErrors.volume_expansion_w ||
    !!fieldErrors.volume_expansion_o
  );

  // Compressibility scale factor: display as coefficient × 10⁻⁶
  const C_SCALE = 1e6;
  let c_w_scaled = $derived(Math.round(bindings.c_w * C_SCALE * 1e4) / 1e4);
  let c_o_scaled = $derived(Math.round(bindings.c_o * C_SCALE * 1e4) / 1e4);

  function setCwScaled(e: Event) {
    const v = Number((e.currentTarget as HTMLInputElement).value);
    if (Number.isFinite(v)) bindings.c_w = v / C_SCALE;
  }
  function setCoScaled(e: Event) {
    const v = Number((e.currentTarget as HTMLInputElement).value);
    if (Number.isFinite(v)) bindings.c_o = v / C_SCALE;
  }

  // Fluid preset tracking
  let activeFluidPreset = $state<string | null>(null);

  function applyFluidPreset(key: string | number) {
    const preset = FLUID_PRESETS.find(p => p.key === key);
    if (!preset) return;
    const p = preset.params;
    for (const [k, value] of Object.entries(p)) {
      if (k in bindings && typeof value !== 'function') {
        (bindings as any)[k] = value;
      }
    }
    activeFluidPreset = String(key);
    onParamEdit();
  }

  const pvtModeOptions = [
    { value: 'constant', label: 'Constant' },
    { value: 'black-oil', label: 'Black-Oil' },
  ];

  function onPvtModeChange(val: string | number) {
    bindings.pvtMode = val as 'constant' | 'black-oil';
    activeFluidPreset = null;
    onParamEdit();
  }
</script>

<Collapsible title="Fluid Properties" {hasError}>
  <div class="space-y-2 px-2.5 py-2">
    <!-- PVT Mode toggle -->
    <div class="flex items-center gap-2">
      <span class="text-[10px] font-medium text-muted-foreground uppercase tracking-wide shrink-0">PVT</span>
      <ToggleGroup options={pvtModeOptions} value={bindings.pvtMode} onChange={onPvtModeChange} />
    </div>

    <!-- Fluid presets -->
    <div class="flex items-center gap-1 flex-wrap">
      <span class="text-[10px] font-medium text-muted-foreground uppercase tracking-wide shrink-0 mr-0.5">Preset</span>
      {#each FLUID_PRESETS as preset}
        <button
          type="button"
          class="px-1.5 py-0.5 text-[10px] font-medium rounded border transition-colors cursor-pointer
            {activeFluidPreset === preset.key
              ? 'border-primary/60 bg-primary/10 text-foreground'
              : 'border-border/60 bg-muted/20 text-muted-foreground hover:border-primary/40 hover:text-foreground'}"
          title={preset.description}
          onclick={() => applyFluidPreset(preset.key)}
        >
          {preset.label}
        </button>
      {/each}
    </div>

    <!-- Fluid phase table -->
    <div class="overflow-x-auto rounded-md border border-border">
      <table class="compact-table w-full text-left">
        <thead class="border-b border-border bg-muted/50 text-[10px] uppercase tracking-wide text-muted-foreground">
          <tr>
            <th class="px-1 py-0.5 font-medium">Phase</th>
            <th class="px-1 py-0.5 font-medium">μ (cP)</th>
            <th class="px-1 py-0.5 font-medium">ρ (kg/m³)</th>
            <th class="px-1 py-0.5 font-medium" title="Compressibility (×10⁻⁶ per bar)">c (×10⁻⁶)</th>
            <th class="px-1 py-0.5 font-medium">B_f</th>
          </tr>
        </thead>
        <tbody class="divide-y divide-border text-xs">
          <tr>
            <td class="px-1 py-0.5 font-medium text-muted-foreground">Water</td>
            <td class="px-0.5 py-0.5">
              <ValidatedInput type="number" min="0.1" step="0.1" class="w-full h-6 px-1 text-xs" bind:value={bindings.mu_w} error={fieldErrors.mu_w} />
            </td>
            <td class="px-0.5 py-0.5">
              <Input type="number" min="1" step="1" class="w-full h-6 px-1 text-xs" bind:value={bindings.rho_w} />
            </td>
            <td class="px-0.5 py-0.5">
               <ValidatedInput type="number" min="0" step="1" class="w-full h-6 px-1 text-xs" value={c_w_scaled} oninput={setCwScaled} error={fieldErrors.c_w} />
            </td>
            <td class="px-0.5 py-0.5">
               <ValidatedInput type="number" min="0.1" step="0.1" class="w-full h-6 px-1 text-xs" bind:value={bindings.volume_expansion_w} error={fieldErrors.volume_expansion_w} />
            </td>
          </tr>
          <tr>
            <td class="px-1 py-0.5 font-medium text-muted-foreground">Oil</td>
            <td class="px-0.5 py-0.5">
              <ValidatedInput type="number" min="0.1" step="0.1" class="w-full h-6 px-1 text-xs" disabled={bindings.pvtMode === 'black-oil'} bind:value={bindings.mu_o} error={fieldErrors.mu_o} />
            </td>
            <td class="px-0.5 py-0.5">
              <Input type="number" min="1" step="1" class="w-full h-6 px-1 text-xs" disabled={bindings.pvtMode === 'black-oil'} bind:value={bindings.rho_o} />
            </td>
            <td class="px-0.5 py-0.5">
              <ValidatedInput type="number" min="0" step="1" class="w-full h-6 px-1 text-xs" disabled={bindings.pvtMode === 'black-oil'} value={c_o_scaled} oninput={setCoScaled} error={fieldErrors.c_o} />
            </td>
            <td class="px-0.5 py-0.5">
              <ValidatedInput type="number" min="0.1" step="0.1" class="w-full h-6 px-1 text-xs" disabled={bindings.pvtMode === 'black-oil'} bind:value={bindings.volume_expansion_o} error={fieldErrors.volume_expansion_o} />
            </td>
          </tr>
        </tbody>
      </table>
    </div>
  </div>
</Collapsible>
