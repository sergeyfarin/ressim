<script lang="ts">
  type BenchmarkMode = 'baseline' | 'refined';

  type BenchmarkRowWithStatus = {
    name: string;
    pvBtSim: number;
    pvBtRef: number;
    relError: number;
    tolerance: number;
    passes: boolean;
    improvementVsBaselinePp: number | null;
  };

  export let benchmarkSource = 'fallback';
  export let benchmarkGeneratedAt = '';
  export let selectedBenchmarkMode: BenchmarkMode = 'baseline';
  export let benchmarkModeLabel: Record<BenchmarkMode, string> = {
    baseline: 'Baseline',
    refined: 'Refined',
  };
  export let benchmarkRowsWithStatus: BenchmarkRowWithStatus[] = [];
  export let allBenchmarksPass = false;
  export let onSelectMode: (mode: BenchmarkMode) => void;
</script>

<div class="card border border-base-300 bg-base-100 shadow-sm">
  <div class="card-body space-y-2 p-4 md:p-5">
    <h3 class="text-lg font-semibold">P4-1 Benchmark Results</h3>
    <p class="text-sm opacity-80">Published reference Buckley-Leverett comparison (analytical vs simulation).</p>
    <div class="text-xs opacity-70">Data Source: {benchmarkSource}{benchmarkGeneratedAt ? `, Generated: ${benchmarkGeneratedAt}` : ''}</div>

    <div class="join my-2">
      <button class="btn btn-sm join-item" class:btn-active={selectedBenchmarkMode === 'baseline'} on:click={() => onSelectMode('baseline')}>
        Baseline
      </button>
      <button class="btn btn-sm join-item" class:btn-active={selectedBenchmarkMode === 'refined'} on:click={() => onSelectMode('refined')}>
        Refined
      </button>
    </div>

    <div class="text-xs opacity-70">Showing: {benchmarkModeLabel[selectedBenchmarkMode]}</div>
    <div class="overflow-x-auto">
      <table class="table table-sm w-full">
        <thead>
          <tr>
            <th>Case</th>
            <th>PV_BT_sim</th>
            <th>PV_BT_ref</th>
            <th>Relative Error</th>
            <th>Improvement vs Baseline</th>
            <th>Tolerance</th>
            <th>Status</th>
          </tr>
        </thead>
        <tbody>
          {#each benchmarkRowsWithStatus as row}
            <tr>
              <td>{row.name}</td>
              <td>{row.pvBtSim.toFixed(4)}</td>
              <td>{row.pvBtRef.toFixed(4)}</td>
              <td>{(row.relError * 100).toFixed(1)}%</td>
              <td>{row.improvementVsBaselinePp === null ? '-' : `${row.improvementVsBaselinePp.toFixed(1)} pp`}</td>
              <td>{(row.tolerance * 100).toFixed(1)}%</td>
              <td>{row.passes ? 'PASS' : 'FAIL'}</td>
            </tr>
          {/each}
        </tbody>
      </table>
    </div>
    <div class="text-sm">Overall Status: {allBenchmarksPass ? 'PASS' : 'FAIL'}</div>
  </div>
</div>
