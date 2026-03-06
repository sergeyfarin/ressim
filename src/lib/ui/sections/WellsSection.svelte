<script lang="ts">
  import Collapsible from "../../components/ui/Collapsible.svelte";
  import Input from "../../components/ui/Input.svelte";
  import Select from "../../components/ui/Select.svelte";
  import {
    panelBodyClass,
    panelTableClass,
    panelTableHeadClass,
    panelTableShellClass,
  } from "../shared/panelStyles";

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
  <div class={panelBodyClass}>
    <div class="flex justify-between items-center mb-2">
      <p class="text-xs font-medium text-muted-foreground">
        Well geometry and XY wellhead locations across all layers.
      </p>
      <p class="text-xs text-muted-foreground font-medium">{groupSummary}</p>
    </div>

    <div class="grid grid-cols-2 gap-2 mt-2">
      <label class="flex flex-col gap-1.5">
        <span class="text-xs font-medium">Well Radius (m)</span>
        <Input
          type="number"
          min="0.01"
          step="0.01"
          class={`w-full ${Boolean(fieldErrors.wellRadius) ? "border-destructive" : ""}`}
          bind:value={well_radius}
        />
        {#if fieldErrors.wellRadius}
          <div class="text-[10px] text-destructive leading-tight mt-0.5">
            {fieldErrors.wellRadius}
          </div>
        {/if}
      </label>
      <label class="flex flex-col gap-1.5">
        <span class="text-xs font-medium">Skin</span>
        <Input type="number" step="0.1" class="w-full" bind:value={well_skin} />
      </label>
    </div>

    <label class="flex items-center gap-2 cursor-pointer mt-3 mb-2">
      <input
        type="checkbox"
        class="h-4 w-4 rounded border-input text-primary accent-primary"
        bind:checked={injectorEnabled}
      />
      <span class="text-sm font-medium leading-none"
        >Enable Injector <span class="text-muted-foreground font-normal"
          >(disable for depletion case)</span
        ></span
      >
    </label>

    <div class={panelTableShellClass}>
      <table class={panelTableClass}>
        <thead class={panelTableHeadClass}>
          <tr>
            <th class="font-medium p-2 w-20">Well</th>
            <th class="font-medium p-2">Control Mode</th>
            <th class="font-medium p-2">Target BHP (bar)</th>
            <th class="font-medium p-2">Target Rate (m³/d)</th>
            <th class="font-medium p-2">Loc (i, j)</th>
          </tr>
        </thead>
        <tbody class="divide-y divide-border">
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
            <td class="p-2 align-top"
              ><Input
                type="number"
                step="1"
                class={`w-full h-7 px-2 ${Boolean(fieldErrors.wellPressureOrder) ? "border-destructive" : ""}`}
                bind:value={injectorBhp}
                disabled={injectorControlMode === "rate" || !injectorEnabled}
              />
              {#if fieldErrors.wellPressureOrder && injectorControlMode === "pressure"}
                <div class="text-[10px] text-destructive mt-1 leading-tight">
                  {fieldErrors.wellPressureOrder}
                </div>
              {/if}
            </td>
            <td class="p-2 align-top"
              ><Input
                type="number"
                min="0"
                step="1"
                class={`w-full h-7 px-2 ${Boolean(fieldErrors.injectorRate) ? "border-destructive" : ""}`}
                bind:value={targetInjectorRate}
                disabled={injectorControlMode === "pressure" ||
                  !injectorEnabled}
              />
              {#if fieldErrors.injectorRate && injectorControlMode === "rate"}
                <div class="text-[10px] text-destructive mt-1 leading-tight">
                  {fieldErrors.injectorRate}
                </div>
              {/if}
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
            <td class="p-2 align-top"
              ><Input
                type="number"
                step="1"
                class={`w-full h-7 px-2 ${Boolean(fieldErrors.wellPressureOrder) ? "border-destructive" : ""}`}
                bind:value={producerBhp}
                disabled={producerControlMode === "rate"}
              />
              {#if fieldErrors.wellPressureOrder && producerControlMode === "pressure"}
                <div class="text-[10px] text-destructive mt-1 leading-tight">
                  {fieldErrors.wellPressureOrder}
                </div>
              {/if}
            </td>
            <td class="p-2 align-top"
              ><Input
                type="number"
                min="0"
                step="1"
                class={`w-full h-7 px-2 ${Boolean(fieldErrors.producerRate) ? "border-destructive" : ""}`}
                bind:value={targetProducerRate}
                disabled={producerControlMode === "pressure"}
              />
              {#if fieldErrors.producerRate && producerControlMode === "rate"}
                <div class="text-[10px] text-destructive mt-1 leading-tight">
                  {fieldErrors.producerRate}
                </div>
              {/if}
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
        </tbody>
      </table>
    </div>

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
