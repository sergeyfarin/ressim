<script lang="ts">
  type ProfileStats = {
    batchMs: number;
    avgStepMs: number;
    extractMs: number;
    renderApplyMs: number;
    snapshotsSent: number;
  };

  export let wasmReady = false;
  export let workerRunning = false;
  export let runCompleted = false;
  export let simTime = 0;
  export let historyLength = 0;
  export let profileStats: ProfileStats = {
    batchMs: 0,
    avgStepMs: 0,
    extractMs: 0,
    renderApplyMs: 0,
    snapshotsSent: 0,
  };
  export let onRunSteps: () => void;
  export let onStepOnce: () => void;
  export let onInitSimulator: () => void;

  export let steps = 20;
  // steps=${steps} ·
  $: groupSummary = `${workerRunning ? 'Running...' : runCompleted ? 'Run completed' : ''}`;
</script>

<details class="rounded-lg border border-base-300 bg-base-100 shadow-sm" open>
  <summary class="flex cursor-pointer list-none items-center justify-between px-4 py-3 md:px-5">
    <div>
      <div class="font-semibold">Simulation and Timestep</div>
      <div class="text-xs opacity-70">{groupSummary}</div>
    </div>
    <div class="flex items-center gap-2 text-xs opacity-70">
      <span class="collapse-label-open hidden">Collapse</span>
      <span class="collapse-label-closed">Expand</span>
      <span class="collapse-chevron">▸</span>
    </div>
  </summary>
  <div class="space-y-3 border-t border-base-300 p-4 md:p-5">
    <p class="text-xs opacity-70">Simulation run actions and runtime diagnostics.</p>

    <label class="form-control">
      <span class="label-text text-xs">Steps</span>
      <input type="number" min="1" class="input input-bordered input-sm w-full max-w-40" bind:value={steps} />
    </label>

    <div class="grid grid-cols-1 gap-2 sm:grid-cols-2">
      <button class="btn btn-sm btn-primary w-full" on:click={onRunSteps} disabled={!wasmReady || workerRunning}>Run {steps} Steps</button>
      <button class="btn btn-sm w-full" on:click={onStepOnce} disabled={!wasmReady || workerRunning}>Step Once</button>
      <button class="btn btn-sm btn-outline sm:col-span-2" on:click={onInitSimulator} disabled={!wasmReady || workerRunning}>Reinitialize Simulator</button>
    </div>

    <div class="text-xs opacity-80">
      <div>Status: {wasmReady ? 'WASM Ready' : 'WASM Loading...'}</div>
      <div>Worker: {workerRunning ? 'Running' : 'Idle'} · Run Completed: {runCompleted ? 'Yes' : 'No'}</div>
      <div>Time: {simTime.toFixed(2)} days · Recorded Steps: {historyLength}</div>
      <div>Avg Step: {profileStats.avgStepMs.toFixed(3)} ms · Batch: {profileStats.batchMs.toFixed(1)} ms</div>
      <div>Extract: {profileStats.extractMs.toFixed(3)} ms · Apply: {profileStats.renderApplyMs.toFixed(3)} ms · Snapshots: {profileStats.snapshotsSent}</div>
    </div>
  </div>
</details>

<style>
  details[open] .collapse-chevron { transform: rotate(90deg); }
  .collapse-chevron { transition: transform 0.15s ease; display: inline-block; }
  details[open] .collapse-label-open { display: inline; }
  details[open] .collapse-label-closed { display: none; }
</style>
