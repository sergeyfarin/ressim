<script lang="ts">
  import Collapsible from "../controls/Collapsible.svelte";
  import Input from "../controls/Input.svelte";
  import Select from "../controls/Select.svelte";
  import ValidatedInput from "../controls/ValidatedInput.svelte";
  import type { ModePanelParameterBindings } from "../modePanelTypes";

  let {
    bindings,
    onNzOrPermModeChange = () => {},
    fieldErrors = {},
  }: {
    bindings: ModePanelParameterBindings;
    onNzOrPermModeChange?: () => void;
    fieldErrors?: Record<string, string>;
  } = $props();

  const permSummary = $derived(
    bindings.permMode === "uniform"
      ? `Uniform ${bindings.uniformPermX}/${bindings.uniformPermY}/${bindings.uniformPermZ} mD`
      : bindings.permMode === "random"
        ? `Random ${bindings.minPerm}-${bindings.maxPerm} mD`
        : `Per Layer (${bindings.nz} layers)`,
  );
  const hasError = $derived(
    Object.keys(fieldErrors).some(
      (key) =>
        key.includes("perm") ||
        key.includes("saturation") ||
        key.includes("initial"),
    ),
  );
</script>

<Collapsible title="Reservoir Properties" {hasError}>
  <div class="space-y-2 px-2.5 py-2">
    <!-- Initial Conditions — dense table -->
    <div class="overflow-x-auto rounded-md border border-border">
      <table class="compact-table w-full text-left">
        <thead class="border-b border-border bg-muted/50 text-[10px] uppercase tracking-wide text-muted-foreground">
          <tr>
            <th class="px-2 py-1 font-medium">P (bar)</th>
            <th class="px-2 py-1 font-medium">Sw_init</th>
            <th class="px-2 py-1 font-medium">Porosity</th>
            <th class="px-2 py-1 font-medium">Depth (m)</th>
            <th class="px-2 py-1 font-medium">c_rock (1/bar)</th>
          </tr>
        </thead>
        <tbody>
          <tr>
            <td class="px-1 py-1">
              <Input type="number" step="10" class="w-full h-7 px-2 text-xs" bind:value={bindings.initialPressure} />
            </td>
            <td class="px-1 py-1">
              <ValidatedInput type="number" min="0" max="1" step="0.05" class="w-full h-7 px-2 text-xs" bind:value={bindings.initialSaturation} error={fieldErrors.initialSaturation} />
            </td>
            <td class="px-1 py-1">
              <Input type="number" min="0.01" max="1.0" step="0.01" class="w-full h-7 px-2 text-xs" bind:value={bindings.reservoirPorosity} />
            </td>
            <td class="px-1 py-1">
              <Input type="number" step="1" class="w-full h-7 px-2 text-xs" bind:value={bindings.depth_reference} />
            </td>
            <td class="px-1 py-1">
              <ValidatedInput type="number" min="0" step="1e-6" class="w-full h-7 px-2 text-xs" bind:value={bindings.rock_compressibility} error={fieldErrors.rock_compressibility} />
            </td>
          </tr>
        </tbody>
      </table>
    </div>

    <!-- Fluid PVT — dense table -->
    <div class="overflow-x-auto rounded-md border border-border">
      <table class="compact-table w-full text-left">
        <thead class="border-b border-border bg-muted/50 text-[10px] uppercase tracking-wide text-muted-foreground">
          <tr>
            <th class="px-2 py-1 font-medium">Phase</th>
            <th class="px-2 py-1 font-medium">μ (cP)</th>
            <th class="px-2 py-1 font-medium">ρ (kg/m³)</th>
            <th class="px-2 py-1 font-medium">c (1/bar)</th>
            <th class="px-2 py-1 font-medium">B_f</th>
          </tr>
        </thead>
        <tbody class="divide-y divide-border text-xs">
          <tr>
            <td class="px-2 py-1 font-medium text-muted-foreground">Water</td>
            <td class="px-1 py-1">
              <ValidatedInput type="number" min="0.1" step="0.1" class="w-full h-7 px-2 text-xs" bind:value={bindings.mu_w} error={fieldErrors.mu_w} />
            </td>
            <td class="px-1 py-1">
              <Input type="number" min="1" step="1" class="w-full h-7 px-2 text-xs" bind:value={bindings.rho_w} />
            </td>
            <td class="px-1 py-1">
              <ValidatedInput type="number" min="0" step="1e-6" class="w-full h-7 px-2 text-xs" bind:value={bindings.c_w} error={fieldErrors.c_w} />
            </td>
            <td class="px-1 py-1">
              <ValidatedInput type="number" min="0.1" step="0.1" class="w-full h-7 px-2 text-xs" bind:value={bindings.volume_expansion_w} error={fieldErrors.volume_expansion_w} />
            </td>
          </tr>
          <tr>
            <td class="px-2 py-1 font-medium text-muted-foreground">Oil</td>
            <td class="px-1 py-1">
              <ValidatedInput type="number" min="0.1" step="0.1" class="w-full h-7 px-2 text-xs" bind:value={bindings.mu_o} error={fieldErrors.mu_o} />
            </td>
            <td class="px-1 py-1">
              <Input type="number" min="1" step="1" class="w-full h-7 px-2 text-xs" bind:value={bindings.rho_o} />
            </td>
            <td class="px-1 py-1">
              <ValidatedInput type="number" min="0" step="1e-6" class="w-full h-7 px-2 text-xs" bind:value={bindings.c_o} error={fieldErrors.c_o} />
            </td>
            <td class="px-1 py-1">
              <ValidatedInput type="number" min="0.1" step="0.1" class="w-full h-7 px-2 text-xs" bind:value={bindings.volume_expansion_o} error={fieldErrors.volume_expansion_o} />
            </td>
          </tr>
        </tbody>
      </table>
    </div>

    <!-- Gravity + Permeability row -->
    <div class="flex items-center gap-3">
      <label class="flex items-center gap-1.5 cursor-pointer">
        <input type="checkbox" class="h-3.5 w-3.5 rounded border-input accent-primary" bind:checked={bindings.gravityEnabled} />
        <span class="text-xs font-medium">Gravity</span>
      </label>
      <div class="flex items-center gap-1.5">
        <span class="text-[10px] font-medium text-muted-foreground uppercase tracking-wide">Perm:</span>
        <Select class="h-7 text-xs px-1.5" bind:value={bindings.permMode} onchange={onNzOrPermModeChange}>
          <option value="uniform">Uniform</option>
          <option value="random">Random</option>
          <option value="perLayer">Per Layer</option>
        </Select>
      </div>
      <span class="text-[10px] text-muted-foreground ml-auto">{permSummary}</span>
    </div>

    <!-- Permeability values -->
    {#if bindings.permMode === "uniform"}
      <div class="overflow-x-auto rounded-md border border-border">
        <table class="compact-table w-full text-left">
          <thead class="border-b border-border bg-muted/50 text-[10px] uppercase tracking-wide text-muted-foreground">
            <tr>
              <th class="px-2 py-1 font-medium">kX (mD)</th>
              <th class="px-2 py-1 font-medium">kY (mD)</th>
              <th class="px-2 py-1 font-medium">kZ (mD)</th>
            </tr>
          </thead>
          <tbody>
            <tr>
              <td class="px-1 py-1"><Input type="number" min="1" class="w-full h-7 px-2 text-xs" bind:value={bindings.uniformPermX} /></td>
              <td class="px-1 py-1"><Input type="number" min="1" class="w-full h-7 px-2 text-xs" bind:value={bindings.uniformPermY} /></td>
              <td class="px-1 py-1"><Input type="number" min="1" class="w-full h-7 px-2 text-xs" bind:value={bindings.uniformPermZ} /></td>
            </tr>
          </tbody>
        </table>
      </div>
    {:else if bindings.permMode === "random"}
      <div class="flex items-center gap-3">
        <label class="flex items-center gap-1.5 cursor-pointer">
          <input type="checkbox" class="h-3.5 w-3.5 rounded border-input accent-primary" bind:checked={bindings.useRandomSeed} />
          <span class="text-xs font-medium">Seeded</span>
        </label>
        {#if bindings.useRandomSeed}
          <Input type="number" step="1" class="w-20 h-7 px-2 text-xs" bind:value={bindings.randomSeed} />
        {/if}
      </div>
      <div class="overflow-x-auto rounded-md border border-border">
        <table class="compact-table w-full text-left">
          <thead class="border-b border-border bg-muted/50 text-[10px] uppercase tracking-wide text-muted-foreground">
            <tr>
              <th class="px-2 py-1 font-medium">Min k (mD)</th>
              <th class="px-2 py-1 font-medium">Max k (mD)</th>
            </tr>
          </thead>
          <tbody>
            <tr>
              <td class="px-1 py-1"><ValidatedInput type="number" min="1" class="w-full h-7 px-2 text-xs" bind:value={bindings.minPerm} error={fieldErrors.permBounds} /></td>
              <td class="px-1 py-1"><ValidatedInput type="number" min="1" class="w-full h-7 px-2 text-xs" bind:value={bindings.maxPerm} error={fieldErrors.permBounds} /></td>
            </tr>
          </tbody>
        </table>
      </div>
    {:else}
      <div class="overflow-x-auto rounded-md border border-border">
        <table class="compact-table w-full text-left">
          <thead class="border-b border-border bg-muted/50 text-[10px] uppercase tracking-wide text-muted-foreground">
            <tr>
              <th class="px-2 py-1 font-medium w-10">Lyr</th>
              <th class="px-2 py-1 font-medium">kX (mD)</th>
              <th class="px-2 py-1 font-medium">kY (mD)</th>
              <th class="px-2 py-1 font-medium">kZ (mD)</th>
            </tr>
          </thead>
          <tbody class="divide-y divide-border">
            {#each Array.from({ length: bindings.nz }) as _, i}
              <tr>
                <td class="px-2 py-0.5 text-xs font-medium text-muted-foreground text-center">{i + 1}</td>
                <td class="px-1 py-0.5"><Input type="number" min="1" class="w-full h-6 px-1.5 text-xs" bind:value={bindings.layerPermsX[i]} /></td>
                <td class="px-1 py-0.5"><Input type="number" min="1" class="w-full h-6 px-1.5 text-xs" bind:value={bindings.layerPermsY[i]} /></td>
                <td class="px-1 py-0.5"><Input type="number" min="1" class="w-full h-6 px-1.5 text-xs" bind:value={bindings.layerPermsZ[i]} /></td>
              </tr>
            {/each}
          </tbody>
        </table>
      </div>
    {/if}
  </div>
</Collapsible>
