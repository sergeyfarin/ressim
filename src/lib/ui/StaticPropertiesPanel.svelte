<script lang="ts">
  import Collapsible from "../components/ui/Collapsible.svelte";
  import Input from "../components/ui/Input.svelte";

  let {
    nx = $bindable(15),
    ny = $bindable(10),
    nz = $bindable(10),
    cellDx = $bindable(10),
    cellDy = $bindable(10),
    cellDz = $bindable(1),
    onNzOrPermModeChange = () => {},
    fieldErrors = {},
  }: {
    nx?: number;
    ny?: number;
    nz?: number;
    cellDx?: number;
    cellDy?: number;
    cellDz?: number;
    onNzOrPermModeChange?: () => void;
    fieldErrors?: Record<string, string>;
  } = $props();

  const modelSizeX = $derived(nx * cellDx);
  const modelSizeY = $derived(ny * cellDy);
  const modelSizeZ = $derived(nz * cellDz);
  const groupSummary = $derived(
    `${nx}×${ny}×${nz} · ${modelSizeX}×${modelSizeY}×${modelSizeZ} m`,
  );

  function handleTotalXChange(e: Event) {
    const val = parseFloat((e.target as HTMLInputElement).value);
    if (!isNaN(val) && nx > 0) cellDx = parseFloat((val / nx).toFixed(3));
  }

  function handleTotalYChange(e: Event) {
    const val = parseFloat((e.target as HTMLInputElement).value);
    if (!isNaN(val) && ny > 0) cellDy = parseFloat((val / ny).toFixed(3));
  }

  function handleTotalZChange(e: Event) {
    const val = parseFloat((e.target as HTMLInputElement).value);
    if (!isNaN(val) && nz > 0) cellDz = parseFloat((val / nz).toFixed(3));
  }
  const hasError = $derived(
    Object.keys(fieldErrors).some(
      (key) =>
        key.includes("nx") ||
        key.includes("ny") ||
        key.includes("nz") ||
        key.includes("cellD"),
    ),
  );
</script>

<Collapsible title="Grid Parameters" {hasError}>
  <div class="space-y-3 p-4 md:p-5">
    <div class="flex justify-between items-center mb-2">
      <p class="text-xs font-medium text-muted-foreground">
        Grid dimensions and physical model extent.
      </p>
      <p class="text-xs text-muted-foreground font-medium">{groupSummary}</p>
    </div>

    <div class="overflow-x-auto rounded-md border border-border">
      <table class="compact-table w-full text-left">
        <thead
          class="bg-muted/50 border-b border-border text-muted-foreground px-2"
        >
          <tr>
            <th class="font-medium p-2">Dim</th>
            <th class="font-medium p-2">Cells (n)</th>
            <th class="font-medium p-2">Size (Δ, m)</th>
            <th class="font-medium p-2">Total (m)</th>
          </tr>
        </thead>
        <tbody class="divide-y divide-border">
          <tr>
            <td
              class="font-semibold text-center align-middle p-2 border-r border-border bg-muted/20"
              >X</td
            >
            <td class="p-2 align-top"
              ><Input
                type="number"
                min="1"
                class={`w-full h-7 px-2 ${Boolean(fieldErrors.nx) ? "border-destructive" : ""}`}
                bind:value={nx}
              />
              {#if fieldErrors.nx}
                <div class="text-[10px] text-destructive mt-1 leading-tight">
                  {fieldErrors.nx}
                </div>
              {/if}
            </td>
            <td class="p-2 align-top"
              ><Input
                type="number"
                min="0.1"
                step="0.1"
                class={`w-full h-7 px-2 ${Boolean(fieldErrors.cellDx) ? "border-destructive" : ""}`}
                bind:value={cellDx}
              />
              {#if fieldErrors.cellDx}
                <div class="text-[10px] text-destructive mt-1 leading-tight">
                  {fieldErrors.cellDx}
                </div>
              {/if}
            </td>
            <td class="p-2 align-top"
              ><Input
                type="number"
                min="0.1"
                step="0.1"
                class="w-full h-7 px-2"
                value={modelSizeX}
                oninput={handleTotalXChange}
              /></td
            >
          </tr>
          <tr>
            <td
              class="font-semibold text-center align-middle p-2 border-r border-border bg-muted/20"
              >Y</td
            >
            <td class="p-2 align-top"
              ><Input
                type="number"
                min="1"
                class={`w-full h-7 px-2 ${Boolean(fieldErrors.ny) ? "border-destructive" : ""}`}
                bind:value={ny}
              />
              {#if fieldErrors.ny}
                <div class="text-[10px] text-destructive mt-1 leading-tight">
                  {fieldErrors.ny}
                </div>
              {/if}
            </td>
            <td class="p-2 align-top"
              ><Input
                type="number"
                min="0.1"
                step="0.1"
                class={`w-full h-7 px-2 ${Boolean(fieldErrors.cellDy) ? "border-destructive" : ""}`}
                bind:value={cellDy}
              />
              {#if fieldErrors.cellDy}
                <div class="text-[10px] text-destructive mt-1 leading-tight">
                  {fieldErrors.cellDy}
                </div>
              {/if}
            </td>
            <td class="p-2 align-top"
              ><Input
                type="number"
                min="0.1"
                step="0.1"
                class="w-full h-7 px-2"
                value={modelSizeY}
                oninput={handleTotalYChange}
              /></td
            >
          </tr>
          <tr>
            <td
              class="font-semibold text-center align-middle p-2 border-r border-border bg-muted/20"
              >Z</td
            >
            <td class="p-2 align-top"
              ><Input
                type="number"
                min="1"
                class={`w-full h-7 px-2 ${Boolean(fieldErrors.nz) ? "border-destructive" : ""}`}
                bind:value={nz}
                oninput={onNzOrPermModeChange}
              />
              {#if fieldErrors.nz}
                <div class="text-[10px] text-destructive mt-1 leading-tight">
                  {fieldErrors.nz}
                </div>
              {/if}
            </td>
            <td class="p-2 align-top"
              ><Input
                type="number"
                min="0.1"
                step="0.1"
                class={`w-full h-7 px-2 ${Boolean(fieldErrors.cellDz) ? "border-destructive" : ""}`}
                bind:value={cellDz}
              />
              {#if fieldErrors.cellDz}
                <div class="text-[10px] text-destructive mt-1 leading-tight">
                  {fieldErrors.cellDz}
                </div>
              {/if}
            </td>
            <td class="p-2 align-top"
              ><Input
                type="number"
                min="0.1"
                step="0.1"
                class="w-full h-7 px-2"
                value={modelSizeZ}
                oninput={handleTotalZChange}
              /></td
            >
          </tr>
        </tbody>
      </table>
    </div>
  </div>
</Collapsible>
