<script lang="ts">
  import Collapsible from "../controls/Collapsible.svelte";
  import Input from "../controls/Input.svelte";
  import Select from "../controls/Select.svelte";
  import {
    panelTableClass,
    panelTableHeadClass,
    panelTableShellClass,
  } from "../shared/panelStyles";

  let {
    initialPressure = $bindable(300),
    initialSaturation = $bindable(0.2),
    reservoirPorosity = $bindable(0.2),
    mu_w = $bindable(0.5),
    mu_o = $bindable(1.0),
    c_o = $bindable(1e-5),
    c_w = $bindable(3e-6),
    rho_w = $bindable(1000),
    rho_o = $bindable(800),
    rock_compressibility = $bindable(1e-6),
    depth_reference = $bindable(0),
    volume_expansion_o = $bindable(1),
    volume_expansion_w = $bindable(1),
    gravityEnabled = $bindable(false),
    permMode = $bindable<"uniform" | "random" | "perLayer">("uniform"),
    uniformPermX = $bindable(100),
    uniformPermY = $bindable(100),
    uniformPermZ = $bindable(10),
    minPerm = $bindable(50),
    maxPerm = $bindable(200),
    useRandomSeed = $bindable(true),
    randomSeed = $bindable(12345),
    nz = $bindable(10),
    layerPermsX = $bindable<number[]>([]),
    layerPermsY = $bindable<number[]>([]),
    layerPermsZ = $bindable<number[]>([]),
    onNzOrPermModeChange = () => {},
    fieldErrors = {},
  }: {
    initialPressure?: number;
    initialSaturation?: number;
    reservoirPorosity?: number;
    mu_w?: number;
    mu_o?: number;
    c_o?: number;
    c_w?: number;
    rho_w?: number;
    rho_o?: number;
    rock_compressibility?: number;
    depth_reference?: number;
    volume_expansion_o?: number;
    volume_expansion_w?: number;
    gravityEnabled?: boolean;
    permMode?: "uniform" | "random" | "perLayer";
    uniformPermX?: number;
    uniformPermY?: number;
    uniformPermZ?: number;
    minPerm?: number;
    maxPerm?: number;
    useRandomSeed?: boolean;
    randomSeed?: number;
    nz?: number;
    layerPermsX?: number[];
    layerPermsY?: number[];
    layerPermsZ?: number[];
    onNzOrPermModeChange?: () => void;
    fieldErrors?: Record<string, string>;
  } = $props();

  const permSummary = $derived(
    permMode === "uniform"
      ? `Uniform ${uniformPermX}/${uniformPermY}/${uniformPermZ} mD`
      : permMode === "random"
        ? `Random ${minPerm}-${maxPerm} mD`
        : `Per Layer (${nz} layers)`,
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
    `P=${initialPressure.toFixed(0)} bar · Sw=${initialSaturation.toFixed(2)} · Φ=${reservoirPorosity.toFixed(2)} · μw/μo=${mu_w.toFixed(2)}/${mu_o.toFixed(2)} · ${permSummary}`,
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
          bind:value={initialPressure}
        />
      </label>
      <label class="flex flex-col gap-1.5">
        <span class="text-[11px] font-medium">Water Saturation</span>
        <Input
          type="number"
          min="0"
          max="1"
          step="0.05"
          class={`w-full h-8 ${Boolean(fieldErrors.initialSaturation) ? "border-destructive" : ""}`}
          bind:value={initialSaturation}
        />
        {#if fieldErrors.initialSaturation}
          <div class="text-[10px] text-destructive leading-tight mt-0.5">
            {fieldErrors.initialSaturation}
          </div>
        {/if}
      </label>
      <label class="flex flex-col gap-1.5">
        <span class="text-[11px] font-medium">Porosity</span>
        <Input
          type="number"
          min="0.01"
          max="1.0"
          step="0.01"
          class="w-full h-8"
          bind:value={reservoirPorosity}
        />
      </label>
      <label class="flex flex-col gap-1.5">
        <span class="text-[11px] font-medium">Depth Ref (m)</span>
        <Input
          type="number"
          step="1"
          class="w-full h-8"
          bind:value={depth_reference}
        />
      </label>
      <label class="flex flex-col gap-1.5">
        <span class="text-[11px] font-medium">Rock Compress. (1/bar)</span>
        <Input
          type="number"
          min="0"
          step="1e-6"
          class={`w-full h-8 ${Boolean(fieldErrors.rock_compressibility) ? "border-destructive" : ""}`}
          bind:value={rock_compressibility}
        />
        {#if fieldErrors.rock_compressibility}
          <div class="text-[10px] text-destructive leading-tight mt-0.5">
            {fieldErrors.rock_compressibility}
          </div>
        {/if}
      </label>
    </div>

    <div class={panelTableShellClass}>
      <table class={panelTableClass}>
        <thead class={panelTableHeadClass}>
          <tr>
            <th class="font-medium p-2">Phase</th>
            <th class="font-medium p-2">Viscosity (cP)</th>
            <th class="font-medium p-2">Density (kg/m³)</th>
            <th class="font-medium p-2">Compress. (1/bar)</th>
            <th class="font-medium p-2">Vol Exp Factor</th>
          </tr>
        </thead>
        <tbody class="divide-y divide-border">
          <tr>
            <td
              class="font-semibold text-center align-middle p-2 border-r border-border bg-muted/20"
              >Water</td
            >
            <td class="p-2 align-top text-center"
              ><Input
                type="number"
                min="0.1"
                step="0.1"
                class={`w-full h-7 px-2 ${Boolean(fieldErrors.mu_w) ? "border-destructive" : ""}`}
                bind:value={mu_w}
              />
              {#if fieldErrors.mu_w}
                <div
                  class="text-[10px] text-destructive mt-1 leading-tight text-left"
                >
                  {fieldErrors.mu_w}
                </div>
              {/if}
            </td>
            <td class="p-2"
              ><Input
                type="number"
                min="1"
                step="1"
                class="w-full h-7 px-2"
                bind:value={rho_w}
              /></td
            >
            <td class="p-2 align-top text-center"
              ><Input
                type="number"
                min="0"
                step="1e-6"
                class={`w-full h-7 px-2 ${Boolean(fieldErrors.c_w) ? "border-destructive" : ""}`}
                bind:value={c_w}
              />
              {#if fieldErrors.c_w}
                <div
                  class="text-[10px] text-destructive mt-1 leading-tight text-left"
                >
                  {fieldErrors.c_w}
                </div>
              {/if}
            </td>
            <td class="p-2 align-top text-center"
              ><Input
                type="number"
                min="0.1"
                step="0.1"
                class={`w-full h-7 px-2 ${Boolean(fieldErrors.volume_expansion_w) ? "border-destructive" : ""}`}
                bind:value={volume_expansion_w}
              />
              {#if fieldErrors.volume_expansion_w}
                <div
                  class="text-[10px] text-destructive mt-1 leading-tight text-left"
                >
                  {fieldErrors.volume_expansion_w}
                </div>
              {/if}
            </td>
          </tr>
          <tr>
            <td
              class="font-semibold text-center align-middle p-2 border-r border-border bg-muted/20"
              >Oil</td
            >
            <td class="p-2 align-top text-center"
              ><Input
                type="number"
                min="0.1"
                step="0.1"
                class={`w-full h-7 px-2 ${Boolean(fieldErrors.mu_o) ? "border-destructive" : ""}`}
                bind:value={mu_o}
              />
              {#if fieldErrors.mu_o}
                <div
                  class="text-[10px] text-destructive mt-1 leading-tight text-left"
                >
                  {fieldErrors.mu_o}
                </div>
              {/if}
            </td>
            <td class="p-2"
              ><Input
                type="number"
                min="1"
                step="1"
                class="w-full h-7 px-2"
                bind:value={rho_o}
              /></td
            >
            <td class="p-2 align-top text-center"
              ><Input
                type="number"
                min="0"
                step="1e-6"
                class={`w-full h-7 px-2 ${Boolean(fieldErrors.c_o) ? "border-destructive" : ""}`}
                bind:value={c_o}
              />
              {#if fieldErrors.c_o}
                <div
                  class="text-[10px] text-destructive mt-1 leading-tight text-left"
                >
                  {fieldErrors.c_o}
                </div>
              {/if}
            </td>
            <td class="p-2 align-top text-center"
              ><Input
                type="number"
                min="0.1"
                step="0.1"
                class={`w-full h-7 px-2 ${Boolean(fieldErrors.volume_expansion_o) ? "border-destructive" : ""}`}
                bind:value={volume_expansion_o}
              />
              {#if fieldErrors.volume_expansion_o}
                <div
                  class="text-[10px] text-destructive mt-1 leading-tight text-left"
                >
                  {fieldErrors.volume_expansion_o}
                </div>
              {/if}
            </td>
          </tr>
        </tbody>
      </table>
    </div>

    <label class="flex items-center gap-2 cursor-pointer">
      <input
        type="checkbox"
        class="h-4 w-4 rounded border-input text-primary accent-primary"
        bind:checked={gravityEnabled}
      />
      <span class="text-sm font-medium leading-none">Enable Gravity</span>
    </label>

    <label class="flex flex-col gap-1.5">
      <span class="text-xs font-medium">Permeability Mode</span>
      <Select
        class="w-full"
        bind:value={permMode}
        onchange={onNzOrPermModeChange}
      >
        <option value="uniform">Uniform</option>
        <option value="random">Random</option>
        <option value="perLayer">Per Layer</option>
      </Select>
    </label>

    <div>
      {#if permMode === "uniform"}
        <div class="grid grid-cols-3 gap-2">
          <label class="flex flex-col gap-1.5">
            <span class="text-xs font-medium">kX (mD)</span>
            <Input
              type="number"
              min="1"
              class="w-full"
              bind:value={uniformPermX}
            />
          </label>
          <label class="flex flex-col gap-1.5">
            <span class="text-xs font-medium">kY (mD)</span>
            <Input
              type="number"
              min="1"
              class="w-full"
              bind:value={uniformPermY}
            />
          </label>
          <label class="flex flex-col gap-1.5">
            <span class="text-xs font-medium">kZ (mD)</span>
            <Input
              type="number"
              min="1"
              class="w-full"
              bind:value={uniformPermZ}
            />
          </label>
        </div>
      {:else if permMode === "random"}
        <label class="flex items-center gap-2 cursor-pointer mb-2">
          <input
            type="checkbox"
            class="h-4 w-4 rounded border-input text-primary accent-primary"
            bind:checked={useRandomSeed}
          />
          <span class="text-sm font-medium leading-none"
            >Use Seeded Randomness</span
          >
        </label>

        {#if useRandomSeed}
          <label class="flex flex-col gap-1.5 mb-2">
            <span class="text-xs font-medium">Random Seed</span>
            <Input
              type="number"
              step="1"
              class="w-full max-w-40"
              bind:value={randomSeed}
            />
          </label>
        {/if}

        <div class="grid grid-cols-2 gap-2">
          <label class="flex flex-col gap-1.5">
            <span class="text-xs font-medium">Min Permeability (mD)</span>
            <Input
              type="number"
              min="1"
              class={`w-full ${Boolean(fieldErrors.permBounds) ? "border-destructive" : ""}`}
              bind:value={minPerm}
            />
            {#if fieldErrors.permBounds}
              <div class="text-[10px] text-destructive leading-tight mt-0.5">
                {fieldErrors.permBounds}
              </div>
            {/if}
          </label>
          <label class="flex flex-col gap-1.5">
            <span class="text-xs font-medium">Max Permeability (mD)</span>
            <Input
              type="number"
              min="1"
              class={`w-full ${Boolean(fieldErrors.permBounds) ? "border-destructive" : ""}`}
              bind:value={maxPerm}
            />
            {#if fieldErrors.permBounds}
              <div class="text-[10px] text-destructive leading-tight mt-0.5">
                {fieldErrors.permBounds}
              </div>
            {/if}
          </label>
        </div>
      {:else}
        <div class={panelTableShellClass}>
          <table class={panelTableClass}>
            <thead class={panelTableHeadClass}>
              <tr>
                <th class="font-medium p-2">Layer</th>
                <th class="font-medium p-2">kX (mD)</th>
                <th class="font-medium p-2">kY (mD)</th>
                <th class="font-medium p-2">kZ (mD)</th>
              </tr>
            </thead>
            <tbody class="divide-y divide-border">
              {#each Array.from({ length: nz }) as _, i}
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
                      bind:value={layerPermsX[i]}
                    /></td
                  >
                  <td class="p-2"
                    ><Input
                      type="number"
                      min="1"
                      class="w-full h-7 px-2"
                      bind:value={layerPermsY[i]}
                    /></td
                  >
                  <td class="p-2"
                    ><Input
                      type="number"
                      min="1"
                      class="w-full h-7 px-2"
                      bind:value={layerPermsZ[i]}
                    /></td
                  >
                </tr>
              {/each}
            </tbody>
          </table>
        </div>
      {/if}
    </div>
  </div>
</Collapsible>
