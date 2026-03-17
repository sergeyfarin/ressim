<script lang="ts">
  import Collapsible from "../controls/Collapsible.svelte";
  import Input from "../controls/Input.svelte";
  import Select from "../controls/Select.svelte";
  import PanelTable from "../controls/PanelTable.svelte";
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
  const groupSummary = $derived(
    `Inj(${injectorI},${injectorJ}) ${injectorEnabled ? "on" : "off"} · Prod(${producerI},${producerJ}) · r=${well_radius.toFixed(2)} m`,
  );
</script>

<Collapsible title="Well Controls" {hasError}>
  <div class="space-y-2 p-3">
    <p class="text-[11px] text-muted-foreground">{groupSummary}</p>

    <div class="grid grid-cols-2 gap-2">
      <label class="flex flex-col gap-1.5">
        <span class="text-xs font-medium">Well Radius (m)</span>
        <ValidatedInput type="number" min="0.01" step="0.01" class="w-full" bind:value={well_radius} error={fieldErrors.wellRadius} />
      </label>
      <label class="flex flex-col gap-1.5">
        <span class="text-xs font-medium">Skin</span>
        <Input type="number" step="0.1" class="w-full" bind:value={well_skin} />
      </label>
    </div>

    <label class="flex items-center gap-2 cursor-pointer">
      <input
        type="checkbox"
        class="h-4 w-4 rounded border-input text-primary accent-primary"
        bind:checked={injectorEnabled}
      />
      <span class="text-sm font-medium leading-none">
        Enable Injector <span class="text-muted-foreground font-normal">(disable for depletion)</span>
      </span>
    </label>

    <PanelTable columns={["Well", "Control Mode", "Target BHP (bar)", "Target Rate (m³/d)", "Loc (i, j)"]}>
          <tr>
            <td
              class={`font-semibold align-middle text-center p-2 border-r border-border bg-muted/20 ${!injectorEnabled ? "opacity-40" : ""}`}
              >Injector</td
            >
            <td class="p-2 align-top">
              <Select
                class="w-full h-7 px-2 text-xs"
                bind:value={injectorControlMode}
                disabled={!injectorEnabled}
              >
                <option value="pressure">Pressure</option>
                <option value="rate">Rate</option>
              </Select>
            </td>
            <td class="p-2 align-top">
              <ValidatedInput
                type="number"
                step="1"
                class="w-full h-7 px-2"
                bind:value={injectorBhp}
                disabled={injectorControlMode === "rate" || !injectorEnabled}
                error={injectorControlMode === "pressure" ? fieldErrors.wellPressureOrder : undefined}
              />
            </td>
            <td class="p-2 align-top">
              <ValidatedInput
                type="number"
                min="0"
                step="1"
                class="w-full h-7 px-2"
                bind:value={targetInjectorRate}
                disabled={injectorControlMode === "pressure" || !injectorEnabled}
                error={injectorControlMode === "rate" ? fieldErrors.injectorRate : undefined}
              />
            </td>
            <td class="p-2 align-top">
              <div class="flex items-center gap-1">
                <Input
                  type="number"
                  min="0"
                  max={Math.max(0, nx - 1)}
                  step="1"
                  class={`w-12 h-7 px-1 text-center ${!injectorValid || Boolean(fieldErrors.wellOverlap) ? "border-destructive" : ""}`}
                  bind:value={injectorI}
                  disabled={!injectorEnabled}
                />
                <span class="text-xs text-muted-foreground">,</span>
                <Input
                  type="number"
                  min="0"
                  max={Math.max(0, ny - 1)}
                  step="1"
                  class={`w-12 h-7 px-1 text-center ${!injectorValid || Boolean(fieldErrors.wellOverlap) ? "border-destructive" : ""}`}
                  bind:value={injectorJ}
                  disabled={!injectorEnabled}
                />
              </div>
              {#if (fieldErrors.wellOverlap || fieldErrors.wellIndexType || fieldErrors.wellIndexRange) && injectorEnabled}
                <div class="text-[10px] text-destructive mt-1 leading-tight">
                  {fieldErrors.wellOverlap ||
                    fieldErrors.wellIndexRange ||
                    fieldErrors.wellIndexType}
                </div>
              {/if}
            </td>
          </tr>
          <tr>
            <td
              class="font-semibold align-middle text-center p-2 border-r border-border bg-muted/20"
              >Producer</td
            >
            <td class="p-2 align-top">
              <Select
                class="w-full h-7 px-2 text-xs"
                bind:value={producerControlMode}
              >
                <option value="pressure">Pressure</option>
                <option value="rate">Rate</option>
              </Select>
            </td>
            <td class="p-2 align-top">
              <ValidatedInput
                type="number"
                step="1"
                class="w-full h-7 px-2"
                bind:value={producerBhp}
                disabled={producerControlMode === "rate"}
                error={producerControlMode === "pressure" ? fieldErrors.wellPressureOrder : undefined}
              />
            </td>
            <td class="p-2 align-top">
              <ValidatedInput
                type="number"
                min="0"
                step="1"
                class="w-full h-7 px-2"
                bind:value={targetProducerRate}
                disabled={producerControlMode === "pressure"}
                error={producerControlMode === "rate" ? fieldErrors.producerRate : undefined}
              />
            </td>
            <td class="p-2 align-top">
              <div class="flex items-center gap-1">
                <Input
                  type="number"
                  min="0"
                  max={Math.max(0, nx - 1)}
                  step="1"
                  class={`w-12 h-7 px-1 text-center ${!producerValid || Boolean(fieldErrors.wellOverlap) ? "border-destructive" : ""}`}
                  bind:value={producerI}
                />
                <span class="text-xs text-muted-foreground">,</span>
                <Input
                  type="number"
                  min="0"
                  max={Math.max(0, ny - 1)}
                  step="1"
                  class={`w-12 h-7 px-1 text-center ${!producerValid || Boolean(fieldErrors.wellOverlap) ? "border-destructive" : ""}`}
                  bind:value={producerJ}
                />
              </div>
              {#if fieldErrors.wellOverlap || fieldErrors.wellIndexType || fieldErrors.wellIndexRange}
                <div class="text-[10px] text-destructive mt-1 leading-tight">
                  {fieldErrors.wellOverlap ||
                    fieldErrors.wellIndexRange ||
                    fieldErrors.wellIndexType}
                </div>
              {/if}
            </td>
          </tr>
    </PanelTable>

    <div class="text-[11px] mt-2 flex justify-between">
      <div>
        <span
          class={!injectorValid
            ? "text-destructive font-medium"
            : "text-muted-foreground"}
          >Injector: {injectorValid ? "valid" : "OOB"}</span
        ><span class="mx-2 text-muted-foreground">·</span><span
          class={!producerValid
            ? "text-destructive font-medium"
            : "text-muted-foreground"}
          >Producer: {producerValid ? "valid" : "OOB"}</span
        >
      </div>
      <div class="text-muted-foreground">
        Grid i: 0–{Math.max(0, nx - 1)}, j: 0–{Math.max(0, ny - 1)}
      </div>
    </div>
  </div>
</Collapsible>
