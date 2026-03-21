<script lang="ts">
  import Collapsible from "../controls/Collapsible.svelte";
  import Input from "../controls/Input.svelte";
  import ChartSubPanel from "../../charts/ChartSubPanel.svelte";
  import type { CurveConfig } from "../../charts/chartTypes";
  import type { ModePanelParameterBindings } from "../modePanelTypes";

  let {
    bindings,
  }: {
    bindings: ModePanelParameterBindings;
  } = $props();

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

<Collapsible title="Black-Oil Correlations">
  <div class="space-y-2 px-2.5 py-2">
    <!-- Correlation inputs -->
    <div class="overflow-x-auto rounded-md border border-border">
      <table class="compact-table w-full text-left">
        <thead class="border-b border-border bg-muted/50 text-[10px] uppercase tracking-wide text-muted-foreground">
          <tr>
            <th class="px-1 py-0.5 font-medium">API Gravity</th>
            <th class="px-1 py-0.5 font-medium">Gas SG</th>
            <th class="px-1 py-0.5 font-medium">Temp (°C)</th>
            <th class="px-1 py-0.5 font-medium">P_b (bar)</th>
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
      <!-- Generated PVT table (collapsible) -->
      <details class="text-xs group">
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

      <!-- PVT curves chart -->
      <div class="border border-border rounded-md overflow-hidden shadow-sm">
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
  </div>
</Collapsible>
