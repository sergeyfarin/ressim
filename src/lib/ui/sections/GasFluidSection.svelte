<script lang="ts">
  import Collapsible from "../controls/Collapsible.svelte";
  import Input from "../controls/Input.svelte";
  import Select from "../controls/Select.svelte";
  import PanelTable from "../controls/PanelTable.svelte";
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
    injectedFluid = $bindable<"water" | "gas">("gas"),
    fieldErrors = {},
  }: {
    s_gc?: number;
    s_gr?: number;
    n_g?: number;
    k_rg_max?: number;
    mu_g?: number;
    c_g?: number;
    rho_g?: number;
    pcogEnabled?: boolean;
    pcogPEntry?: number;
    pcogLambda?: number;
    injectedFluid?: "water" | "gas";
    fieldErrors?: Record<string, string>;
  } = $props();

  const hasError = $derived(
    !!fieldErrors.s_gc ||
      !!fieldErrors.s_gr ||
      !!fieldErrors.n_g ||
      !!fieldErrors.mu_g ||
      !!fieldErrors.c_g,
  );
  const groupSummary = $derived(
    `S_gc=${s_gc.toFixed(2)}, S_gr=${s_gr.toFixed(2)}, n_g=${n_g.toFixed(1)}, μ_g=${mu_g.toFixed(3)} cP · Inject: ${injectedFluid}`,
  );
</script>

<Collapsible title="Gas Phase" {hasError}>
  <div class="space-y-2 p-3">
    <p class="text-[11px] text-muted-foreground">{groupSummary}</p>

    <label class="flex flex-col gap-1.5">
      <span class="text-xs font-medium">Injected Fluid</span>
      <Select class="w-full max-w-40" bind:value={injectedFluid}>
        <option value="gas">Gas</option>
        <option value="water">Water</option>
      </Select>
    </label>

    <p class="text-[11px] font-medium text-muted-foreground pt-1">Gas Relative Permeability</p>
    <PanelTable columns={["S_gc", "S_gr", "n_g", "k_rg_max"]}>
      <tr>
        <td class="p-2 align-top">
          <ValidatedInput
            type="number"
            min="0"
            max="1"
            step="0.01"
            class="w-full h-7 px-2"
            bind:value={s_gc}
            error={fieldErrors.s_gc}
          />
        </td>
        <td class="p-2 align-top">
          <ValidatedInput
            type="number"
            min="0"
            max="1"
            step="0.01"
            class="w-full h-7 px-2"
            bind:value={s_gr}
            error={fieldErrors.s_gr}
          />
        </td>
        <td class="p-2 align-top">
          <ValidatedInput
            type="number"
            min="0.1"
            step="0.1"
            class="w-full h-7 px-2"
            bind:value={n_g}
            error={fieldErrors.n_g}
          />
        </td>
        <td class="p-2 align-top">
          <Input
            type="number"
            min="0.01"
            max="1"
            step="0.05"
            class="w-full h-7 px-2"
            bind:value={k_rg_max}
          />
        </td>
      </tr>
    </PanelTable>

    <p class="text-[11px] font-medium text-muted-foreground pt-1">Gas Fluid Properties</p>
    <div class="grid grid-cols-3 gap-2">
      <label class="flex flex-col gap-1.5">
        <span class="text-xs font-medium">Viscosity (cP)</span>
        <ValidatedInput
          type="number"
          min="0.001"
          step="0.001"
          class="w-full"
          bind:value={mu_g}
          error={fieldErrors.mu_g}
        />
      </label>
      <label class="flex flex-col gap-1.5">
        <span class="text-xs font-medium">Compress. (1/bar)</span>
        <ValidatedInput
          type="number"
          min="0"
          step="1e-5"
          class="w-full"
          bind:value={c_g}
          error={fieldErrors.c_g}
        />
      </label>
      <label class="flex flex-col gap-1.5">
        <span class="text-xs font-medium">Density (kg/m³)</span>
        <Input
          type="number"
          min="0.1"
          step="1"
          class="w-full"
          bind:value={rho_g}
        />
      </label>
    </div>

    <label class="flex items-center gap-2 cursor-pointer pt-1">
      <input
        type="checkbox"
        class="h-4 w-4 rounded border-input text-primary accent-primary"
        bind:checked={pcogEnabled}
      />
      <span class="text-sm font-medium leading-none">Enable Gas-Oil Capillary Pressure</span>
    </label>

    {#if pcogEnabled}
      <div class="grid grid-cols-2 gap-2">
        <label class="flex flex-col gap-1.5">
          <span class="text-xs font-medium">P_entry (bar)</span>
          <Input
            type="number"
            min="0"
            step="0.5"
            class="w-full"
            bind:value={pcogPEntry}
          />
        </label>
        <label class="flex flex-col gap-1.5">
          <span class="text-xs font-medium">Lambda</span>
          <Input
            type="number"
            min="0.1"
            step="0.1"
            class="w-full"
            bind:value={pcogLambda}
          />
        </label>
      </div>
    {/if}
  </div>
</Collapsible>
