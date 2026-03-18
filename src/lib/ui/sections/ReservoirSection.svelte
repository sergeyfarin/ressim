<script lang="ts">
  import Collapsible from "../controls/Collapsible.svelte";
  import Input from "../controls/Input.svelte";
  import Select from "../controls/Select.svelte";
  import PanelTable from "../controls/PanelTable.svelte";
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
  const groupSummary = $derived(
    `P=${bindings.initialPressure.toFixed(0)} bar · Sw=${bindings.initialSaturation.toFixed(2)} · Φ=${bindings.reservoirPorosity.toFixed(2)} · μw/μo=${bindings.mu_w.toFixed(2)}/${bindings.mu_o.toFixed(2)} · ${permSummary}`,
  );
</script>

<Collapsible title="Reservoir Properties" {hasError}>
  <div class="space-y-2 p-3">
    <p class="text-[11px] text-muted-foreground">{groupSummary}</p>

    <div class="grid grid-cols-2 gap-2 md:grid-cols-5">
      <label class="flex flex-col gap-1.5">
        <span class="text-[11px] font-medium">Pressure (bar)</span>
        <Input
          type="number"
          step="10"
          class="w-full h-8"
          bind:value={bindings.initialPressure}
        />
      </label>
      <label class="flex flex-col gap-1.5">
        <span class="text-[11px] font-medium">Water Saturation</span>
        <ValidatedInput type="number" min="0" max="1" step="0.05" class="w-full h-8" bind:value={bindings.initialSaturation} error={fieldErrors.initialSaturation} />
      </label>
      <label class="flex flex-col gap-1.5">
        <span class="text-[11px] font-medium">Porosity</span>
        <Input
          type="number"
          min="0.01"
          max="1.0"
          step="0.01"
          class="w-full h-8"
          bind:value={bindings.reservoirPorosity}
        />
      </label>
      <label class="flex flex-col gap-1.5">
        <span class="text-[11px] font-medium">Depth Ref (m)</span>
        <Input
          type="number"
          step="1"
          class="w-full h-8"
          bind:value={bindings.depth_reference}
        />
      </label>
      <label class="flex flex-col gap-1.5">
        <span class="text-[11px] font-medium">Rock Compress. (1/bar)</span>
        <ValidatedInput type="number" min="0" step="1e-6" class="w-full h-8" bind:value={bindings.rock_compressibility} error={fieldErrors.rock_compressibility} />
      </label>
    </div>

    <PanelTable columns={["Phase", "Viscosity (cP)", "Density (kg/m³)", "Compress. (1/bar)", "Vol Exp Factor"]}>
          <tr>
            <td
              class="font-semibold text-center align-middle p-2 border-r border-border bg-muted/20"
              >Water</td
            >
            <td class="p-2 align-top text-center">
              <ValidatedInput type="number" min="0.1" step="0.1" class="w-full h-7 px-2" bind:value={bindings.mu_w} error={fieldErrors.mu_w} />
            </td>
            <td class="p-2"
              ><Input
                type="number"
                min="1"
                step="1"
                class="w-full h-7 px-2"
                bind:value={bindings.rho_w}
              /></td
            >
            <td class="p-2 align-top text-center">
              <ValidatedInput type="number" min="0" step="1e-6" class="w-full h-7 px-2" bind:value={bindings.c_w} error={fieldErrors.c_w} />
            </td>
            <td class="p-2 align-top text-center">
              <ValidatedInput type="number" min="0.1" step="0.1" class="w-full h-7 px-2" bind:value={bindings.volume_expansion_w} error={fieldErrors.volume_expansion_w} />
            </td>
          </tr>
          <tr>
            <td
              class="font-semibold text-center align-middle p-2 border-r border-border bg-muted/20"
              >Oil</td
            >
            <td class="p-2 align-top text-center">
              <ValidatedInput type="number" min="0.1" step="0.1" class="w-full h-7 px-2" bind:value={bindings.mu_o} error={fieldErrors.mu_o} />
            </td>
            <td class="p-2"
              ><Input
                type="number"
                min="1"
                step="1"
                class="w-full h-7 px-2"
                bind:value={bindings.rho_o}
              /></td
            >
            <td class="p-2 align-top text-center">
              <ValidatedInput type="number" min="0" step="1e-6" class="w-full h-7 px-2" bind:value={bindings.c_o} error={fieldErrors.c_o} />
            </td>
            <td class="p-2 align-top text-center">
              <ValidatedInput type="number" min="0.1" step="0.1" class="w-full h-7 px-2" bind:value={bindings.volume_expansion_o} error={fieldErrors.volume_expansion_o} />
            </td>
          </tr>
    </PanelTable>

    <label class="flex items-center gap-2 cursor-pointer">
      <input
        type="checkbox"
        class="h-4 w-4 rounded border-input text-primary accent-primary"
        bind:checked={bindings.gravityEnabled}
      />
      <span class="text-sm font-medium leading-none">Enable Gravity</span>
    </label>

    <label class="flex flex-col gap-1.5">
      <span class="text-xs font-medium">Permeability Mode</span>
      <Select
        class="w-full"
        bind:value={bindings.permMode}
        onchange={onNzOrPermModeChange}
      >
        <option value="uniform">Uniform</option>
        <option value="random">Random</option>
        <option value="perLayer">Per Layer</option>
      </Select>
    </label>

    <div>
      {#if bindings.permMode === "uniform"}
        <div class="grid grid-cols-3 gap-2">
          <label class="flex flex-col gap-1.5">
            <span class="text-xs font-medium">kX (mD)</span>
            <Input
              type="number"
              min="1"
              class="w-full"
              bind:value={bindings.uniformPermX}
            />
          </label>
          <label class="flex flex-col gap-1.5">
            <span class="text-xs font-medium">kY (mD)</span>
            <Input
              type="number"
              min="1"
              class="w-full"
              bind:value={bindings.uniformPermY}
            />
          </label>
          <label class="flex flex-col gap-1.5">
            <span class="text-xs font-medium">kZ (mD)</span>
            <Input
              type="number"
              min="1"
              class="w-full"
              bind:value={bindings.uniformPermZ}
            />
          </label>
        </div>
      {:else if bindings.permMode === "random"}
        <label class="flex items-center gap-2 cursor-pointer mb-2">
          <input
            type="checkbox"
            class="h-4 w-4 rounded border-input text-primary accent-primary"
            bind:checked={bindings.useRandomSeed}
          />
          <span class="text-sm font-medium leading-none"
            >Use Seeded Randomness</span
          >
        </label>

        {#if bindings.useRandomSeed}
          <label class="flex flex-col gap-1.5 mb-2">
            <span class="text-xs font-medium">Random Seed</span>
            <Input
              type="number"
              step="1"
              class="w-full max-w-40"
              bind:value={bindings.randomSeed}
            />
          </label>
        {/if}

        <div class="grid grid-cols-2 gap-2">
          <label class="flex flex-col gap-1.5">
            <span class="text-xs font-medium">Min Permeability (mD)</span>
            <ValidatedInput type="number" min="1" class="w-full" bind:value={bindings.minPerm} error={fieldErrors.permBounds} />
          </label>
          <label class="flex flex-col gap-1.5">
            <span class="text-xs font-medium">Max Permeability (mD)</span>
            <ValidatedInput type="number" min="1" class="w-full" bind:value={bindings.maxPerm} error={fieldErrors.permBounds} />
          </label>
        </div>
      {:else}
        <PanelTable columns={["Layer", "kX (mD)", "kY (mD)", "kZ (mD)"]}>
              {#each Array.from({ length: bindings.nz }) as _, i}
                <tr>
                  <td
                    class="font-semibold text-center align-middle p-2 border-r border-border bg-muted/20"
                    >{i + 1}</td
                  >
                  <td class="p-2"
                    ><Input
                      type="number"
                      min="1"
                      class="w-full h-7 px-2"
                      bind:value={bindings.layerPermsX[i]}
                    /></td
                  >
                  <td class="p-2"
                    ><Input
                      type="number"
                      min="1"
                      class="w-full h-7 px-2"
                      bind:value={bindings.layerPermsY[i]}
                    /></td
                  >
                  <td class="p-2"
                    ><Input
                      type="number"
                      min="1"
                      class="w-full h-7 px-2"
                      bind:value={bindings.layerPermsZ[i]}
                    /></td
                  >
                </tr>
              {/each}
        </PanelTable>
      {/if}
    </div>
  </div>
</Collapsible>
