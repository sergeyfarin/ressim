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
  export let estimatedRunSeconds = 0;
  export let longRunEstimate = false;
  export let canStop = false;
  export let hasValidationErrors = false;
  export let solverWarning = '';
  export let modelReinitNotice = '';
  export let steps = 20;
  export let profileStats: ProfileStats = {
    batchMs: 0, avgStepMs: 0, extractMs: 0, renderApplyMs: 0, snapshotsSent: 0,
  };

  export let onRunSteps: () => void;
  export let onStepOnce: () => void;
  export let onInitSimulator: () => void;
  export let onStopRun: () => void;
</script>

<div class="card border border-base-300 bg-base-100 shadow-sm">
  <div class="card-body p-3 md:p-4">
    <div class="flex flex-wrap items-center gap-3">
      <!-- Steps input -->
      <label class="flex items-center gap-2">
        <span class="text-xs font-medium whitespace-nowrap">Steps:</span>
        <input
          type="number"
          min="1"
          class="input input-bordered input-sm w-20"
          bind:value={steps}
        />
      </label>

      <!-- Action buttons -->
      <div class="flex flex-wrap gap-2">
        <button
          class="btn btn-sm btn-primary"
          on:click={onRunSteps}
          disabled={!wasmReady || workerRunning || hasValidationErrors}
        >▶ Run {steps} Steps</button>

        <button
          class="btn btn-sm btn-outline"
          on:click={onStepOnce}
          disabled={!wasmReady || workerRunning || hasValidationErrors}
        >Step Once</button>

        <button
          class="btn btn-sm btn-warning"
          on:click={onStopRun}
          disabled={!canStop}
        >⏹ Stop</button>

        <button
          class="btn btn-sm btn-ghost"
          on:click={onInitSimulator}
          disabled={!wasmReady || workerRunning || hasValidationErrors}
        >↻ Reinit</button>
      </div>

      <!-- Status -->
      <div class="flex flex-wrap items-center gap-3 text-xs opacity-70 ml-auto">
        <span class="badge badge-sm {wasmReady ? 'badge-success' : 'badge-warning'}">
          {wasmReady ? 'WASM Ready' : 'Loading…'}
        </span>
        <span>{workerRunning ? '⏳ Running' : runCompleted ? '✓ Done' : '○ Idle'}</span>
        <span>{simTime.toFixed(1)} days</span>
        <span>{historyLength} steps</span>
      </div>
    </div>

    <!-- Warnings / notices -->
    {#if modelReinitNotice}
      <div class="text-xs text-warning mt-1">⚠ {modelReinitNotice}</div>
    {/if}
    {#if solverWarning}
      <div class="text-xs text-warning mt-1">⚠ {solverWarning}</div>
    {/if}
    {#if longRunEstimate}
      <div class="text-xs opacity-60 mt-1">Estimated: {estimatedRunSeconds.toFixed(1)}s — you can stop at any time</div>
    {/if}
  </div>
</div>
