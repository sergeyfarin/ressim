<script lang="ts">
  type ProfileStats = {
    batchMs: number;
    avgStepMs: number;
    extractMs: number;
    renderApplyMs: number;
    snapshotsSent: number;
  };

  let {
    wasmReady = false,
    workerRunning = false,
    runCompleted = false,
    modelReinitNotice = "",
    simTime = 0,
    historyLength = 0,
    profileStats = {
      batchMs: 0,
      avgStepMs: 0,
      extractMs: 0,
      renderApplyMs: 0,
      snapshotsSent: 0,
    },
    onRunSteps = () => {},
    onStepOnce = () => {},
    onInitSimulator = () => {},
    onStopRun = () => {},
    estimatedRunSeconds = 0,
    longRunEstimate = false,
    canStop = false,
    hasValidationErrors = false,
    solverWarning = "",
    steps = $bindable(20),
  }: {
    wasmReady?: boolean;
    workerRunning?: boolean;
    runCompleted?: boolean;
    modelReinitNotice?: string;
    simTime?: number;
    historyLength?: number;
    profileStats?: ProfileStats;
    onRunSteps?: () => void;
    onStepOnce?: () => void;
    onInitSimulator?: () => void;
    onStopRun?: () => void;
    estimatedRunSeconds?: number;
    longRunEstimate?: boolean;
    canStop?: boolean;
    hasValidationErrors?: boolean;
    solverWarning?: string;
    steps?: number;
  } = $props();

  const groupSummary = $derived(`${workerRunning ? "Running..." : runCompleted ? "Run completed" : ""} ${longRunEstimate ? "· long run estimate" : ""}`);
</script>

<details class="rounded-lg border border-base-300 bg-base-100 shadow-sm" open>
  <summary
    class="flex cursor-pointer list-none items-center justify-between px-4 py-3 md:px-5"
  >
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
    <p class="text-xs opacity-70">
      Simulation run actions and runtime diagnostics.
    </p>

    <label class="form-control">
      <span class="label-text text-xs">Steps</span>
      <input
        type="number"
        min="1"
        class="input input-bordered input-sm w-full max-w-40"
        bind:value={steps}
      />
    </label>

    <div class="grid grid-cols-1 gap-2 sm:grid-cols-2">
      <button
        class="btn btn-sm btn-primary w-full"
        onclick={onRunSteps}
        disabled={!wasmReady || workerRunning || hasValidationErrors}
        >Run {steps} Steps</button
      >
      <button
        class="btn btn-sm w-full"
        onclick={onStepOnce}
        disabled={!wasmReady || workerRunning || hasValidationErrors}
        >Step Once</button
      >
      <button
        class="btn btn-sm btn-warning w-full"
        onclick={onStopRun}
        disabled={!canStop}>Stop</button
      >
      <button
        class="btn btn-sm btn-outline w-full"
        onclick={onInitSimulator}
        disabled={!wasmReady || workerRunning || hasValidationErrors}
        >Reinitialize Simulator</button
      >
    </div>

    <div class="text-xs opacity-80">
      {#if modelReinitNotice}
        <div class="text-warning font-semibold">⚠ {modelReinitNotice}</div>
      {/if}
      <div>Status: {wasmReady ? "WASM Ready" : "WASM Loading..."}</div>
      <div>
        Worker: {workerRunning ? "Running" : "Idle"} · Run Completed: {runCompleted
          ? "Yes"
          : "No"}
      </div>
      {#if solverWarning}
        <div class="text-warning font-semibold">⚠ {solverWarning}</div>
      {/if}
      <div>
        Time: {simTime.toFixed(2)} days · Recorded Steps: {historyLength}
      </div>
      <div>
        Estimated run time: {estimatedRunSeconds.toFixed(1)} s {#if longRunEstimate}·
          consider Stop for long runs{/if}
      </div>
      <div>
        Avg Step: {profileStats.avgStepMs.toFixed(3)} ms · Batch: {profileStats.batchMs.toFixed(
          1,
        )} ms
      </div>
      <div>
        Extract: {profileStats.extractMs.toFixed(3)} ms · Apply: {profileStats.renderApplyMs.toFixed(
          3,
        )} ms · Snapshots: {profileStats.snapshotsSent}
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
