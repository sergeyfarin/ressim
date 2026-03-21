<script lang="ts">
  import Collapsible from "../controls/Collapsible.svelte";
  import Input from "../controls/Input.svelte";
  import Select from "../controls/Select.svelte";
  import ValidatedInput from "../controls/ValidatedInput.svelte";

  let {
    s_gc = $bindable(0.05),
    s_gr = $bindable(0.05),
    n_g = $bindable(1.5),
    k_rg_max = $bindable(1.0),
    mu_g = $bindable(0.02),
    c_g = $bindable(1e-4),
    rho_g = $bindable(10.0),
    pcogEnabled = $bindable(false),
    pcogPEntry = $bindable(3.0),
    pcogLambda = $bindable(2.0),
    initialGasSaturation = $bindable(0.0),
    injectedFluid = $bindable<"water" | "gas">("gas"),
    pvtMode = "constant",
    fieldErrors = {},
  }: {
    s_gc?: number;
    s_gr?: number;
    n_g?: number;
    k_rg_max?: number;
    mu_g?: number;
    c_g?: number;
    rho_g?: number;
    initialGasSaturation?: number;
    pcogEnabled?: boolean;
    pcogPEntry?: number;
    pcogLambda?: number;
    injectedFluid?: "water" | "gas";
    pvtMode?: string;
    fieldErrors?: Record<string, string>;
  } = $props();

  const hasError = $derived(
    !!fieldErrors.s_gc || !!fieldErrors.s_gr || !!fieldErrors.n_g || !!fieldErrors.mu_g || !!fieldErrors.c_g,
  );

  // Gas compressibility scale: display as coefficient × 10⁻⁴
  const CG_SCALE = 1e4;
  let c_g_scaled = $derived(Math.round(c_g * CG_SCALE * 1e4) / 1e4);
  function setCgScaled(e: Event) {
    const v = Number((e.currentTarget as HTMLInputElement).value);
    if (Number.isFinite(v)) c_g = v / CG_SCALE;
  }
</script>

<Collapsible title="Gas Phase" {hasError}>
  <div class="space-y-2 px-2.5 py-2">
    <div class="flex items-center gap-3">
      <span class="text-[10px] font-medium text-muted-foreground uppercase tracking-wide">Inject:</span>
      <Select class="h-6 text-xs px-1.5 w-20" bind:value={injectedFluid}>
        <option value="gas">Gas</option>
        <option value="water">Water</option>
      </Select>
      <label class="flex items-center gap-1 text-xs ml-auto">
        <span class="text-[10px] text-muted-foreground">S_gi</span>
        <Input type="number" min="0" max="1" step="0.01" class="w-14 h-6 px-1 text-xs" bind:value={initialGasSaturation} />
      </label>
    </div>

    <!-- Gas rel perm + PVT in one table -->
    <div class="overflow-x-auto rounded-md border border-border">
      <table class="compact-table w-full text-left">
        <thead class="border-b border-border bg-muted/50 text-[10px] uppercase tracking-wide text-muted-foreground">
          <tr>
            <th class="px-1 py-0.5 font-medium">S_gc</th>
            <th class="px-1 py-0.5 font-medium">S_gr</th>
            <th class="px-1 py-0.5 font-medium">n_g</th>
            <th class="px-1 py-0.5 font-medium">kr_max</th>
            <th class="px-1 py-0.5 font-medium">μ_g (cP)</th>
            <th class="px-1 py-0.5 font-medium" title="Gas compressibility (×10⁻⁴ per bar)">c_g (×10⁻⁴)</th>
            <th class="px-1 py-0.5 font-medium">ρ_g (kg/m³)</th>
          </tr>
        </thead>
        <tbody>
          <tr class="text-xs">
            <td class="px-0.5 py-0.5"><ValidatedInput type="number" min="0" max="1" step="0.01" class="w-full h-6 px-1 text-xs" bind:value={s_gc} error={fieldErrors.s_gc} /></td>
            <td class="px-0.5 py-0.5"><ValidatedInput type="number" min="0" max="1" step="0.01" class="w-full h-6 px-1 text-xs" bind:value={s_gr} error={fieldErrors.s_gr} /></td>
            <td class="px-0.5 py-0.5"><ValidatedInput type="number" min="0.1" step="0.1" class="w-full h-6 px-1 text-xs" bind:value={n_g} error={fieldErrors.n_g} /></td>
            <td class="px-0.5 py-0.5"><Input type="number" min="0.01" max="1" step="0.05" class="w-full h-6 px-1 text-xs" bind:value={k_rg_max} /></td>
            <td class="px-0.5 py-0.5"><ValidatedInput type="number" min="0.001" step="0.001" class="w-full h-6 px-1 text-xs" disabled={pvtMode === 'black-oil'} bind:value={mu_g} error={fieldErrors.mu_g} /></td>
            <td class="px-0.5 py-0.5"><ValidatedInput type="number" min="0" step="0.1" class="w-full h-6 px-1 text-xs" disabled={pvtMode === 'black-oil'} value={c_g_scaled} oninput={setCgScaled} error={fieldErrors.c_g} /></td>
            <td class="px-0.5 py-0.5"><Input type="number" min="0.1" step="1" class="w-full h-6 px-1 text-xs" disabled={pvtMode === 'black-oil'} bind:value={rho_g} /></td>
          </tr>
        </tbody>
      </table>
    </div>

    <!-- Gas-oil capillary -->
    <div class="flex items-center gap-3">
      <label class="flex items-center gap-1.5 cursor-pointer">
        <input type="checkbox" class="h-3.5 w-3.5 rounded border-input accent-primary" bind:checked={pcogEnabled} />
        <span class="text-xs font-medium">Gas-Oil Pc</span>
      </label>
      {#if pcogEnabled}
        <label class="flex items-center gap-1 text-xs">
          <span class="text-[10px] text-muted-foreground">P_e (bar)</span>
          <Input type="number" min="0" step="0.5" class="w-16 h-6 px-1 text-xs" bind:value={pcogPEntry} />
        </label>
        <label class="flex items-center gap-1 text-xs">
          <span class="text-[10px] text-muted-foreground">λ</span>
          <Input type="number" min="0.1" step="0.1" class="w-14 h-6 px-1 text-xs" bind:value={pcogLambda} />
        </label>
      {/if}
    </div>
  </div>
</Collapsible>
