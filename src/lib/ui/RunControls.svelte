<script lang="ts">
  let {
    wasmReady = false,
    workerRunning = false,
    runCompleted = false,
    simTime = 0,
    historyLength = 0,
    estimatedRunSeconds = 0,
    longRunEstimate = false,
    canStop = false,
    hasValidationErrors = false,
    solverWarning = "",
    modelReinitNotice = "",
    continuationStatus = "",
    runProgress = "",
    inputsAnchorHref = "",
    steps = $bindable(20),
    onRunSteps = () => {},
    onStepOnce = () => {},
    onInitSimulator = () => {},
    onStopRun = () => {},
  }: {
    wasmReady?: boolean;
    workerRunning?: boolean;
    runCompleted?: boolean;
    simTime?: number;
    historyLength?: number;
    estimatedRunSeconds?: number;
    longRunEstimate?: boolean;
    canStop?: boolean;
    hasValidationErrors?: boolean;
    solverWarning?: string;
    modelReinitNotice?: string;
    continuationStatus?: string;
    runProgress?: string;
    inputsAnchorHref?: string;
    steps?: number;
    onRunSteps?: () => void;
    onStepOnce?: () => void;
    onInitSimulator?: () => void;
    onStopRun?: () => void;
  } = $props();
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
          onclick={onRunSteps}
          disabled={!wasmReady || workerRunning || hasValidationErrors}
          >▶ Run {steps} Steps</button
        >

        <button
          class="btn btn-sm btn-outline"
          onclick={onStepOnce}
          disabled={!wasmReady || workerRunning || hasValidationErrors}
          >Step Once</button
        >

        <button
          class="btn btn-sm btn-warning"
          onclick={onStopRun}
          disabled={!canStop}>⏹ Stop</button
        >

        <button
          class="btn btn-sm btn-ghost"
          onclick={onInitSimulator}
          disabled={!wasmReady || workerRunning || hasValidationErrors}
          >↻ Reinit</button
        >

        {#if inputsAnchorHref}
          <a
            class="link link-primary text-xs self-center"
            href={inputsAnchorHref}>Jump to Inputs</a
          >
        {/if}
      </div>

      <!-- Status -->
      <div class="flex flex-wrap items-center gap-3 text-xs opacity-70 ml-auto">
        <span
          class="badge badge-sm {wasmReady ? 'badge-success' : 'badge-warning'}"
        >
          {wasmReady ? "WASM Ready" : "Loading…"}
        </span>
        <span>{continuationStatus ||
            (workerRunning
              ? "⏳ Running"
              : runCompleted
                ? "✓ Done"
                : "○ Idle")}</span>
        <span>{simTime.toFixed(1)} days</span>
        <span>{historyLength} snapshots</span>
        {#if runProgress}
          <span class="text-primary">{runProgress}</span>
        {/if}
      </div>
    </div>

    {#if continuationStatus}
      <div class="text-xs text-info mt-1">{continuationStatus}</div>
    {/if}

    <!-- Warnings / notices -->
    {#if modelReinitNotice}
      <div class="text-xs text-warning mt-1">⚠ {modelReinitNotice}</div>
    {/if}
    {#if solverWarning}
      <div class="text-xs text-warning mt-1">⚠ {solverWarning}</div>
    {/if}
    {#if longRunEstimate}
      <div class="text-xs opacity-60 mt-1">
        Estimated: {estimatedRunSeconds.toFixed(1)}s — you can stop at any time
      </div>
    {/if}
  </div>
</div>
