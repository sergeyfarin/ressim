<script lang="ts">
  import Collapsible from "../controls/Collapsible.svelte";
  import Input from "../controls/Input.svelte";
  import Select from "../controls/Select.svelte";
  import ValidatedInput from "../controls/ValidatedInput.svelte";

  let {
    well_radius = $bindable(0.1),
    well_skin = $bindable(0),
    nx = $bindable(15),
    ny = $bindable(10),
    injectorEnabled = $bindable(true),
    injectorControlMode = $bindable<"rate" | "pressure">("pressure"),
    producerControlMode = $bindable<"rate" | "pressure">("pressure"),
    injectorBhp = $bindable(500),
    producerBhp = $bindable(100),
    targetInjectorRate = $bindable(350),
    targetProducerRate = $bindable(350),
    fieldErrors = {},
    injectorI = $bindable(0),
    injectorJ = $bindable(0),
    producerI = $bindable(14),
    producerJ = $bindable(0),
  }: {
    well_radius?: number;
    well_skin?: number;
    nx?: number;
    ny?: number;
    injectorEnabled?: boolean;
    injectorControlMode?: "rate" | "pressure";
    producerControlMode?: "rate" | "pressure";
    injectorBhp?: number;
    producerBhp?: number;
    targetInjectorRate?: number;
    targetProducerRate?: number;
    fieldErrors?: Record<string, string>;
    injectorI?: number;
    injectorJ?: number;
    producerI?: number;
    producerJ?: number;
  } = $props();

  function inBounds(i: number, j: number) {
    return i >= 0 && i < nx && j >= 0 && j < ny;
  }

  const injectorValid = $derived(inBounds(injectorI, injectorJ));
  const producerValid = $derived(inBounds(producerI, producerJ));
  const hasError = $derived(
    injectorValid === false ||
      producerValid === false ||
      Object.keys(fieldErrors).some(
        (key) =>
          key.includes("well") ||
          key.includes("injector") ||
          key.includes("producer"),
      ),
  );
</script>

<Collapsible title="Well Controls" {hasError}>
  <div class="space-y-2 px-2.5 py-2">
    <!-- Wellbore params + injector toggle -->
    <div class="flex items-center gap-3 flex-wrap">
      <label class="flex items-center gap-1 text-xs">
        <span class="text-[10px] font-medium text-muted-foreground">r_w (m)</span>
        <ValidatedInput type="number" min="0.01" step="0.01" class="w-16 h-6 px-1 text-xs" bind:value={well_radius} error={fieldErrors.wellRadius} />
      </label>
      <label class="flex items-center gap-1 text-xs">
        <span class="text-[10px] font-medium text-muted-foreground">Skin</span>
        <Input type="number" step="0.1" class="w-16 h-6 px-1 text-xs" bind:value={well_skin} />
      </label>
      <label class="flex items-center gap-1.5 cursor-pointer ml-auto">
        <input type="checkbox" class="h-3.5 w-3.5 rounded border-input accent-primary" bind:checked={injectorEnabled} />
        <span class="text-xs font-medium">Injector</span>
      </label>
    </div>

    <!-- Well table -->
    <div class="overflow-x-auto rounded-md border border-border">
      <table class="compact-table w-full text-left">
        <thead class="border-b border-border bg-muted/50 text-[10px] uppercase tracking-wide text-muted-foreground">
          <tr>
            <th class="px-1 py-0.5 font-medium">Well</th>
            <th class="px-1 py-0.5 font-medium">Mode</th>
            <th class="px-1 py-0.5 font-medium">BHP (bar)</th>
            <th class="px-1 py-0.5 font-medium">Rate (m³/d)</th>
            <th class="px-1 py-0.5 font-medium">i, j</th>
          </tr>
        </thead>
        <tbody class="divide-y divide-border text-xs">
          <tr class={!injectorEnabled ? "opacity-40" : ""}>
            <td class="px-1 py-0.5 font-medium text-muted-foreground">Inj</td>
            <td class="px-0.5 py-0.5">
              <Select class="w-full h-6 px-1 text-xs" bind:value={injectorControlMode} disabled={!injectorEnabled}>
                <option value="pressure">P</option>
                <option value="rate">Q</option>
              </Select>
            </td>
            <td class="px-0.5 py-0.5">
              <ValidatedInput type="number" step="1" class="w-full h-6 px-1 text-xs"
                bind:value={injectorBhp}
                disabled={injectorControlMode === "rate" || !injectorEnabled}
                error={injectorControlMode === "pressure" ? fieldErrors.wellPressureOrder : undefined} />
            </td>
            <td class="px-0.5 py-0.5">
              <ValidatedInput type="number" min="0" step="1" class="w-full h-6 px-1 text-xs"
                bind:value={targetInjectorRate}
                disabled={injectorControlMode === "pressure" || !injectorEnabled}
                error={injectorControlMode === "rate" ? fieldErrors.injectorRate : undefined} />
            </td>
            <td class="px-0.5 py-0.5">
              <div class="flex items-center gap-0.5">
                <Input type="number" min="0" max={Math.max(0, nx - 1)} step="1"
                  class={`w-10 h-6 px-1 text-center text-xs ${!injectorValid || Boolean(fieldErrors.wellOverlap) ? "border-destructive" : ""}`}
                  bind:value={injectorI} disabled={!injectorEnabled} />
                <span class="text-muted-foreground">,</span>
                <Input type="number" min="0" max={Math.max(0, ny - 1)} step="1"
                  class={`w-10 h-6 px-1 text-center text-xs ${!injectorValid || Boolean(fieldErrors.wellOverlap) ? "border-destructive" : ""}`}
                  bind:value={injectorJ} disabled={!injectorEnabled} />
              </div>
            </td>
          </tr>
          <tr>
            <td class="px-1 py-0.5 font-medium text-muted-foreground">Prod</td>
            <td class="px-0.5 py-0.5">
              <Select class="w-full h-6 px-1 text-xs" bind:value={producerControlMode}>
                <option value="pressure">P</option>
                <option value="rate">Q</option>
              </Select>
            </td>
            <td class="px-0.5 py-0.5">
              <ValidatedInput type="number" step="1" class="w-full h-6 px-1 text-xs"
                bind:value={producerBhp}
                disabled={producerControlMode === "rate"}
                error={producerControlMode === "pressure" ? fieldErrors.wellPressureOrder : undefined} />
            </td>
            <td class="px-0.5 py-0.5">
              <ValidatedInput type="number" min="0" step="1" class="w-full h-6 px-1 text-xs"
                bind:value={targetProducerRate}
                disabled={producerControlMode === "pressure"}
                error={producerControlMode === "rate" ? fieldErrors.producerRate : undefined} />
            </td>
            <td class="px-0.5 py-0.5">
              <div class="flex items-center gap-0.5">
                <Input type="number" min="0" max={Math.max(0, nx - 1)} step="1"
                  class={`w-10 h-6 px-1 text-center text-xs ${!producerValid || Boolean(fieldErrors.wellOverlap) ? "border-destructive" : ""}`}
                  bind:value={producerI} />
                <span class="text-muted-foreground">,</span>
                <Input type="number" min="0" max={Math.max(0, ny - 1)} step="1"
                  class={`w-10 h-6 px-1 text-center text-xs ${!producerValid || Boolean(fieldErrors.wellOverlap) ? "border-destructive" : ""}`}
                  bind:value={producerJ} />
              </div>
            </td>
          </tr>
        </tbody>
      </table>
    </div>

    {#if fieldErrors.wellOverlap || fieldErrors.wellIndexType || fieldErrors.wellIndexRange}
      <div class="text-[10px] text-destructive leading-tight">
        {fieldErrors.wellOverlap || fieldErrors.wellIndexRange || fieldErrors.wellIndexType}
      </div>
    {/if}

    <div class="text-[10px] text-muted-foreground text-right">
      Grid: i 0–{Math.max(0, nx - 1)}, j 0–{Math.max(0, ny - 1)}
    </div>
  </div>
</Collapsible>
