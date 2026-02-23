<script lang="ts">
  let {
    well_radius = $bindable(0.1),
    well_skin = $bindable(0),
    nx = $bindable(15),
    ny = $bindable(10),
    injectorEnabled = $bindable(true),
    injectorControlMode = $bindable<"rate" | "pressure">("pressure"),
    producerControlMode = $bindable<"rate" | "pressure">("pressure"),
    injectorBhp = $bindable(400),
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

<details
  class="rounded-lg border bg-base-100 shadow-sm"
  class:border-error={hasError}
  class:border-base-300={!hasError}
>
  <summary
    class="flex cursor-pointer list-none items-center justify-between px-4 py-3 md:px-5"
  >
    <div>
      <div class="font-semibold">Well Controls</div>
      <div class="text-xs opacity-70">{groupSummary}</div>
    </div>
    <div class="flex items-center gap-2 text-xs opacity-70">
      <span class="collapse-label-open hidden">Collapse</span>
      <span class="collapse-label-closed">Expand</span>
      <span class="collapse-chevron">▸</span>
    </div>
  </summary>

  <div class="space-y-3 border-t border-base-300 p-4 md:p-5">
    <p class="text-xs opacity-70">
      Well geometry and XY wellhead locations across all layers.
    </p>
    <div class="grid grid-cols-2 gap-2">
      <label class="form-control">
        <span class="label-text text-xs">Well Radius (m)</span>
        <input
          type="number"
          min="0.01"
          step="0.01"
          class="input input-bordered input-sm w-full"
          class:input-error={Boolean(fieldErrors.wellRadius)}
          bind:value={well_radius}
        />
      </label>
      <label class="form-control">
        <span class="label-text text-xs">Skin</span>
        <input
          type="number"
          step="0.1"
          class="input input-bordered input-sm w-full"
          bind:value={well_skin}
        />
      </label>
    </div>

    <label class="label cursor-pointer justify-start gap-2">
      <input
        type="checkbox"
        class="checkbox checkbox-sm"
        bind:checked={injectorEnabled}
      />
      <span class="label-text text-sm"
        >Enable Injector (disable for depletion case)</span
      >
    </label>

    <div class="overflow-x-auto rounded-md border border-base-300">
      <table class="table table-xs w-full">
        <thead>
          <tr class="bg-base-200/50">
            <th>Well</th>
            <th>Control Mode</th>
            <th>Target BHP (bar)</th>
            <th>Target Rate (m³/d)</th>
            <th>Location (i, j)</th>
          </tr>
        </thead>
        <tbody>
          <tr>
            <td
              class="font-semibold align-middle text-center"
              class:opacity-50={!injectorEnabled}>Injector</td
            >
            <td>
              <select
                class="select select-bordered select-xs w-full min-w-24"
                bind:value={injectorControlMode}
                disabled={!injectorEnabled}
              >
                <option value="pressure">Pressure</option>
                <option value="rate">Rate</option>
              </select>
            </td>
            <td
              ><input
                type="number"
                step="1"
                class="input input-bordered input-xs w-full min-w-16 max-w-24"
                class:input-error={Boolean(fieldErrors.wellPressureOrder)}
                bind:value={injectorBhp}
                disabled={injectorControlMode === "rate" || !injectorEnabled}
              /></td
            >
            <td
              ><input
                type="number"
                min="0"
                step="1"
                class="input input-bordered input-xs w-full min-w-16 max-w-24"
                class:input-error={Boolean(fieldErrors.injectorRate)}
                bind:value={targetInjectorRate}
                disabled={injectorControlMode === "pressure" ||
                  !injectorEnabled}
              /></td
            >
            <td>
              <div class="flex items-center gap-1">
                <input
                  type="number"
                  min="0"
                  max={Math.max(0, nx - 1)}
                  step="1"
                  class="input input-bordered input-xs w-12"
                  class:input-error={!injectorValid ||
                    Boolean(fieldErrors.wellOverlap)}
                  bind:value={injectorI}
                  disabled={!injectorEnabled}
                />
                <span class="text-xs">,</span>
                <input
                  type="number"
                  min="0"
                  max={Math.max(0, ny - 1)}
                  step="1"
                  class="input input-bordered input-xs w-12"
                  class:input-error={!injectorValid ||
                    Boolean(fieldErrors.wellOverlap)}
                  bind:value={injectorJ}
                  disabled={!injectorEnabled}
                />
              </div>
            </td>
          </tr>
          <tr>
            <td class="font-semibold align-middle text-center">Producer</td>
            <td>
              <select
                class="select select-bordered select-xs w-full min-w-24"
                bind:value={producerControlMode}
              >
                <option value="pressure">Pressure</option>
                <option value="rate">Rate</option>
              </select>
            </td>
            <td
              ><input
                type="number"
                step="1"
                class="input input-bordered input-xs w-full min-w-16 max-w-24"
                class:input-error={Boolean(fieldErrors.wellPressureOrder)}
                bind:value={producerBhp}
                disabled={producerControlMode === "rate"}
              /></td
            >
            <td
              ><input
                type="number"
                min="0"
                step="1"
                class="input input-bordered input-xs w-full min-w-16 max-w-24"
                class:input-error={Boolean(fieldErrors.producerRate)}
                bind:value={targetProducerRate}
                disabled={producerControlMode === "pressure"}
              /></td
            >
            <td>
              <div class="flex items-center gap-1">
                <input
                  type="number"
                  min="0"
                  max={Math.max(0, nx - 1)}
                  step="1"
                  class="input input-bordered input-xs w-12"
                  class:input-error={!producerValid ||
                    Boolean(fieldErrors.wellOverlap)}
                  bind:value={producerI}
                />
                <span class="text-xs">,</span>
                <input
                  type="number"
                  min="0"
                  max={Math.max(0, ny - 1)}
                  step="1"
                  class="input input-bordered input-xs w-12"
                  class:input-error={!producerValid ||
                    Boolean(fieldErrors.wellOverlap)}
                  bind:value={producerJ}
                />
              </div>
            </td>
          </tr>
        </tbody>
      </table>
    </div>

    <div class="text-xs mt-2 flex justify-between">
      <div>
        <span class:opacity-70={injectorValid} class:text-error={!injectorValid}
          >Injector: {injectorValid ? "valid" : "OOB"}</span
        ><span class="mx-2">·</span><span
          class:opacity-70={producerValid}
          class:text-error={!producerValid}
          >Producer: {producerValid ? "valid" : "OOB"}</span
        >
      </div>
      <div class="opacity-70">
        Grid i: 0–{Math.max(0, nx - 1)}, j: 0–{Math.max(0, ny - 1)}
      </div>
    </div>
  </div>
</details>

<style>
  details[open] .collapse-chevron {
    transform: rotate(90deg);
  }
  .collapse-chevron {
    transition: transform 0.15s ease;
    display: inline-block;
  }
  details[open] .collapse-label-open {
    display: inline;
  }
  details[open] .collapse-label-closed {
    display: none;
  }
</style>
