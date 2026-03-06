<script lang="ts">
  import Card from "../../components/ui/Card.svelte";
  import {
    panelBodyClass,
    panelTableClass,
    panelTableHeadClass,
    panelTableShellClass,
  } from "../shared/panelStyles";

  type BenchmarkMode = "baseline" | "refined";

  type BenchmarkRowWithStatus = {
    name: string;
    pvBtSim: number;
    pvBtRef: number;
    relError: number;
    tolerance: number;
    passes: boolean;
    improvementVsBaselinePp: number | null;
  };

  let {
    benchmarkSource = "fallback",
    benchmarkGeneratedAt = "",
    selectedBenchmarkMode = "baseline",
    benchmarkModeLabel = {
      baseline: "Baseline",
      refined: "Refined",
    },
    benchmarkRowsWithStatus = [],
    allBenchmarksPass = false,
    onSelectMode = () => {},
  }: {
    benchmarkSource?: string;
    benchmarkGeneratedAt?: string;
    selectedBenchmarkMode?: BenchmarkMode;
    benchmarkModeLabel?: Record<BenchmarkMode, string>;
    benchmarkRowsWithStatus?: BenchmarkRowWithStatus[];
    allBenchmarksPass?: boolean;
    onSelectMode?: (mode: BenchmarkMode) => void;
  } = $props();
</script>

<Card>
  <div class={panelBodyClass}>
    <div>
      <h3 class="text-base font-semibold leading-none tracking-tight">
        P4-1 Benchmark Results
      </h3>
      <p class="text-xs text-muted-foreground mt-1">
        Published reference Buckley-Leverett comparison (analytical vs
        simulation).
      </p>
      <div class="text-[11px] text-muted-foreground mt-0.5">
        Data Source: {benchmarkSource}{benchmarkGeneratedAt
          ? `, Generated: ${benchmarkGeneratedAt}`
          : ""}
      </div>
    </div>

    <div
      class="inline-flex rounded-md border border-border shadow-sm mb-2 mt-2"
      role="group"
    >
      <button
        type="button"
        class="px-3 py-1.5 text-xs font-medium rounded-l-md transition-colors
        {selectedBenchmarkMode === 'baseline'
          ? 'bg-primary text-primary-foreground'
          : 'bg-transparent text-muted-foreground hover:bg-muted/50 hover:text-foreground'}"
        onclick={() => onSelectMode("baseline")}
      >
        Baseline
      </button>
      <button
        type="button"
        class="px-3 py-1.5 text-xs font-medium rounded-r-md border-l border-border transition-colors
        {selectedBenchmarkMode === 'refined'
          ? 'bg-primary text-primary-foreground'
          : 'bg-transparent text-muted-foreground hover:bg-muted/50 hover:text-foreground'}"
        onclick={() => onSelectMode("refined")}
      >
        Refined
      </button>
    </div>

    <div class="text-[11px] text-muted-foreground">
      Showing: {benchmarkModeLabel[selectedBenchmarkMode]}
    </div>
    <div class={`${panelTableShellClass} mt-1`}>
      <table class={panelTableClass}>
        <thead class={panelTableHeadClass}>
          <tr>
            <th class="font-medium p-2">Case</th>
            <th class="font-medium p-2">PV_BT_sim</th>
            <th class="font-medium p-2">PV_BT_ref</th>
            <th class="font-medium p-2">Relative Error</th>
            <th class="font-medium p-2">Improvement vs Baseline</th>
            <th class="font-medium p-2">Tolerance</th>
            <th class="font-medium p-2">Status</th>
          </tr>
        </thead>
        <tbody class="divide-y divide-border">
          {#each benchmarkRowsWithStatus as row}
            <tr>
              <td class="p-2 font-medium">{row.name}</td>
              <td class="p-2">{row.pvBtSim.toFixed(4)}</td>
              <td class="p-2">{row.pvBtRef.toFixed(4)}</td>
              <td class="p-2">{(row.relError * 100).toFixed(1)}%</td>
              <td class="p-2"
                >{row.improvementVsBaselinePp === null
                  ? "-"
                  : `${row.improvementVsBaselinePp.toFixed(1)} pp`}</td
              >
              <td class="p-2">{(row.tolerance * 100).toFixed(1)}%</td>
              <td class="p-2">
                <span
                  class="inline-block px-1.5 py-0.5 rounded text-[10px] font-bold
                  {row.passes
                    ? 'bg-success/15 text-success'
                    : 'bg-destructive/15 text-destructive'}"
                >
                  {row.passes ? "PASS" : "FAIL"}
                </span>
              </td>
            </tr>
          {/each}
        </tbody>
      </table>
    </div>

    <div class="text-xs font-medium mt-3 pb-1 flex items-center gap-2">
      Overall Status:
      <span
        class="inline-block px-2 py-0.5 rounded text-xs font-bold
        {allBenchmarksPass
          ? 'bg-success/15 text-success'
          : 'bg-destructive/15 text-destructive'}"
      >
        {allBenchmarksPass ? "PASS" : "FAIL"}
      </span>
    </div>
  </div>
</Card>
