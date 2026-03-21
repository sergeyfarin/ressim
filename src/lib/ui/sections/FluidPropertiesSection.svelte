<script lang="ts">
  import Collapsible from "../controls/Collapsible.svelte";
  import Input from "../controls/Input.svelte";
  import Select from "../controls/Select.svelte";
  import ValidatedInput from "../controls/ValidatedInput.svelte";
  import ChartSubPanel from "../../charts/ChartSubPanel.svelte";
  import type { CurveConfig } from "../../charts/ChartSubPanel.svelte";
  import type { ModePanelParameterBindings } from "../modePanelTypes";

  let {
    bindings,
    fieldErrors = {},
  }: {
    bindings: ModePanelParameterBindings;
    fieldErrors?: Record<string, string>;
  } = $props();

  const hasError = $derived(
    !!fieldErrors.mu_w ||
    !!fieldErrors.mu_o ||
    !!fieldErrors.c_o ||
    !!fieldErrors.c_w ||
    !!fieldErrors.volume_expansion_w ||
    !!fieldErrors.volume_expansion_o
  );

  // Compressibility scale factor: display as coefficient × 10⁻⁶
  const C_SCALE = 1e6;
  let c_w_scaled = $derived(Math.round(bindings.c_w * C_SCALE * 1e4) / 1e4);
  let c_o_scaled = $derived(Math.round(bindings.c_o * C_SCALE * 1e4) / 1e4);

  function setCwScaled(e: Event) {
    const v = Number((e.currentTarget as HTMLInputElement).value);
    if (Number.isFinite(v)) bindings.c_w = v / C_SCALE;
  }
  function setCoScaled(e: Event) {
    const v = Number((e.currentTarget as HTMLInputElement).value);
    if (Number.isFinite(v)) bindings.c_o = v / C_SCALE;
  }

  let pvtChartExpanded = $state(true);

  let pvtData = $derived.by(() => {
    if (!bindings.pvtTable) return [[], [], [], [], []];
    return [
      bindings.pvtTable.map(r => ({ x: r.p_bar, y: r.rs_m3m3 })),
      bindings.pvtTable.map(r => ({ x: r.p_bar, y: r.bo_m3m3 })),
      bindings.pvtTable.map(r => ({ x: r.p_bar, y: r.mu_o_cp })),
      bindings.pvtTable.map(r => ({ x: r.p_bar, y: r.bg_m3m3 })),
      bindings.pvtTable.map(r => ({ x: r.p_bar, y: r.mu_g_cp }))
    ];
  });

  const pvtCurves: CurveConfig[] = [
    { label: "Rs", curveKey: "rs", color: "#22c55e", yAxisID: "y", borderWidth: 2 },
    { label: "Bo", curveKey: "bo", color: "#3b82f6", yAxisID: "y1", borderWidth: 2 },
    { label: "μ_o", curveKey: "mu_o", color: "#eab308", yAxisID: "y2", borderWidth: 2 },
    { label: "Bg", curveKey: "bg", color: "#ef4444", yAxisID: "y3", defaultVisible: false, borderWidth: 2 },
    { label: "μ_g", curveKey: "mu_g", color: "#a855f7", yAxisID: "y4", defaultVisible: false, borderWidth: 2 }
  ];

  const pvtScales = {
    x: { type: "linear", display: true, title: { display: true, text: "Pressure (bar)" }, min: 0 },
    y:  { type: "linear", display: true, position: "left", title: { display: true, text: "Rs (m³/m³)" }, _auto: true },
    y1: { type: "linear", display: true, position: "right", title: { display: true, text: "Bo" }, _auto: true, grid: { drawOnChartArea: false } },
    y2: { type: "linear", display: true, position: "right", title: { display: true, text: "μ_o (cP)" }, _auto: true, grid: { drawOnChartArea: false } },
    y3: { type: "linear", display: true, position: "right", title: { display: true, text: "Bg" }, _auto: true, grid: { drawOnChartArea: false } },
    y4: { type: "linear", display: true, position: "right", title: { display: true, text: "μ_g (cP)" }, _auto: true, grid: { drawOnChartArea: false } }
  };
</script>

<Collapsible title="Fluid Properties" {hasError}>
  <div class="space-y-3 px-2.5 py-2">
    <!-- Fluid PVT Header -->
    <div class="flex items-center justify-between mb-1 px-1">
      <span class="text-xs font-semibold text-foreground">PVT Model</span>
      <Select class="h-6 text-xs px-1.5 w-40" bind:value={bindings.pvtMode}>
        <option value="constant">Constant Properties</option>
        <option value="black-oil">Black-Oil Correlations</option>
      </Select>
    </div>

    <!-- Black-Oil inputs (if active) -->
    {#if bindings.pvtMode === 'black-oil'}
      <div class="overflow-x-auto rounded-md border border-border mb-2">
        <table class="compact-table w-full text-left">
          <thead class="border-b border-border bg-muted/50 text-[10px] uppercase tracking-wide text-muted-foreground">
            <tr>
              <th class="px-1 py-0.5 font-medium">API Gravity</th>
              <th class="px-1 py-0.5 font-medium">Gas SG (Air=1)</th>
              <th class="px-1 py-0.5 font-medium">Temp (°C)</th>
              <th class="px-1 py-0.5 font-medium">Bubble Pt (bar)</th>
            </tr>
          </thead>
          <tbody>
            <tr>
              <td class="px-0.5 py-0.5"><Input type="number" step="1" class="w-full h-6 px-1 text-xs" bind:value={bindings.apiGravity} /></td>
              <td class="px-0.5 py-0.5"><Input type="number" step="0.05" class="w-full h-6 px-1 text-xs" bind:value={bindings.gasSpecificGravity} /></td>
              <td class="px-0.5 py-0.5"><Input type="number" step="5" class="w-full h-6 px-1 text-xs" bind:value={bindings.reservoirTemperature} /></td>
              <td class="px-0.5 py-0.5"><Input type="number" step="10" class="w-full h-6 px-1 text-xs" bind:value={bindings.bubblePoint} /></td>
            </tr>
          </tbody>
        </table>
      </div>

      {#if bindings.pvtTable && bindings.pvtTable.length > 0}
        <details class="text-xs mb-2 group">
          <summary class="cursor-pointer font-medium text-muted-foreground hover:text-foreground list-none flex items-center gap-1">
            <svg class="h-3 w-3 shrink-0 transition-transform duration-200 group-open:rotate-90" xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><polyline points="9 18 15 12 9 6"></polyline></svg>
            View Generated PVT Table
          </summary>
          <div class="overflow-y-auto max-h-40 rounded-md border border-border mt-1 relative">
            <table class="compact-table w-full text-left text-[10px]">
              <thead class="border-b border-border bg-muted/80 text-muted-foreground sticky top-0 backdrop-blur-sm">
                <tr>
                  <th class="px-1 py-0.5 font-medium">P (bar)</th>
                  <th class="px-1 py-0.5 font-medium">R_s (m³/m³)</th>
                  <th class="px-1 py-0.5 font-medium">B_o</th>
                  <th class="px-1 py-0.5 font-medium">μ_o (cP)</th>
                  <th class="px-1 py-0.5 font-medium">B_g</th>
                  <th class="px-1 py-0.5 font-medium">μ_g (cP)</th>
                </tr>
              </thead>
              <tbody class="divide-y divide-border">
                {#each bindings.pvtTable as row, i}
                  {#if i % Math.max(1, Math.floor(bindings.pvtTable.length / 10)) === 0 || i === bindings.pvtTable.length - 1}
                    <tr>
                      <td class="px-2 py-0.5">{row.p_bar.toFixed(1)}</td>
                      <td class="px-2 py-0.5">{row.rs_m3m3.toFixed(2)}</td>
                      <td class="px-2 py-0.5">{row.bo_m3m3.toFixed(3)}</td>
                      <td class="px-2 py-0.5">{row.mu_o_cp.toFixed(3)}</td>
                      <td class="px-2 py-0.5">{row.bg_m3m3.toExponential(2)}</td>
                      <td class="px-2 py-0.5">{row.mu_g_cp.toFixed(4)}</td>
                    </tr>
                  {/if}
                {/each}
              </tbody>
            </table>
          </div>
        </details>

        <div class="mb-3 border border-border rounded-md overflow-hidden shadow-sm">
          <ChartSubPanel
            panelId="pvt"
            title="PVT Property Curves"
            bind:expanded={pvtChartExpanded}
            curves={pvtCurves}
            seriesData={pvtData}
            scaleConfigs={pvtScales}
            chartHeight="260px"
          />
        </div>
      {/if}
    {/if}

    <!-- Fluid PVT — dense table -->
    <div class="overflow-x-auto rounded-md border border-border">
      <table class="compact-table w-full text-left">
        <thead class="border-b border-border bg-muted/50 text-[10px] uppercase tracking-wide text-muted-foreground">
          <tr>
            <th class="px-1 py-0.5 font-medium">Phase</th>
            <th class="px-1 py-0.5 font-medium">μ (cP)</th>
            <th class="px-1 py-0.5 font-medium">ρ (kg/m³)</th>
            <th class="px-1 py-0.5 font-medium" title="Compressibility (×10⁻⁶ per bar)">c (×10⁻⁶)</th>
            <th class="px-1 py-0.5 font-medium">B_f</th>
          </tr>
        </thead>
        <tbody class="divide-y divide-border text-xs">
          <tr>
            <td class="px-1 py-0.5 font-medium text-muted-foreground">Water</td>
            <td class="px-0.5 py-0.5">
              <ValidatedInput type="number" min="0.1" step="0.1" class="w-full h-6 px-1 text-xs" bind:value={bindings.mu_w} error={fieldErrors.mu_w} />
            </td>
            <td class="px-0.5 py-0.5">
              <Input type="number" min="1" step="1" class="w-full h-6 px-1 text-xs" bind:value={bindings.rho_w} />
            </td>
            <td class="px-0.5 py-0.5">
               <ValidatedInput type="number" min="0" step="1" class="w-full h-6 px-1 text-xs" value={c_w_scaled} oninput={setCwScaled} error={fieldErrors.c_w} />
            </td>
            <td class="px-0.5 py-0.5">
               <ValidatedInput type="number" min="0.1" step="0.1" class="w-full h-6 px-1 text-xs" bind:value={bindings.volume_expansion_w} error={fieldErrors.volume_expansion_w} />
            </td>
          </tr>
          <tr>
            <td class="px-1 py-0.5 font-medium text-muted-foreground">Oil</td>
            <td class="px-0.5 py-0.5">
              <ValidatedInput type="number" min="0.1" step="0.1" class="w-full h-6 px-1 text-xs" disabled={bindings.pvtMode === 'black-oil'} bind:value={bindings.mu_o} error={fieldErrors.mu_o} />
            </td>
            <td class="px-0.5 py-0.5">
              <Input type="number" min="1" step="1" class="w-full h-6 px-1 text-xs" disabled={bindings.pvtMode === 'black-oil'} bind:value={bindings.rho_o} />
            </td>
            <td class="px-0.5 py-0.5">
              <ValidatedInput type="number" min="0" step="1" class="w-full h-6 px-1 text-xs" disabled={bindings.pvtMode === 'black-oil'} value={c_o_scaled} oninput={setCoScaled} error={fieldErrors.c_o} />
            </td>
            <td class="px-0.5 py-0.5">
              <ValidatedInput type="number" min="0.1" step="0.1" class="w-full h-6 px-1 text-xs" disabled={bindings.pvtMode === 'black-oil'} bind:value={bindings.volume_expansion_o} error={fieldErrors.volume_expansion_o} />
            </td>
          </tr>
        </tbody>
      </table>
    </div>

  </div>
</Collapsible>
