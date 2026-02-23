<script lang="ts">
  import Collapsible from "../components/ui/Collapsible.svelte";
  import Button from "../components/ui/Button.svelte";
  import Input from "../components/ui/Input.svelte";

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

  const groupSummary = $derived(
    `${workerRunning ? "Running..." : runCompleted ? "Run completed" : ""} ${longRunEstimate ? "· long run estimate" : ""}`,
  );
</script>

<Collapsible title="Simulation and Timestep" open>
  <div class="space-y-3 p-4 md:p-5">
    <p class="text-xs text-muted-foreground flex justify-between">
      <span>Simulation run actions and runtime diagnostics.</span>
      <span>{groupSummary}</span>
    </p>

    <label class="flex items-center gap-2">
      <span class="text-xs font-medium whitespace-nowrap">Steps</span>
      <Input type="number" min="1" class="w-full max-w-40" bind:value={steps} />
    </label>

    <div class="grid grid-cols-1 gap-2 sm:grid-cols-2">
      <Button
        size="sm"
        variant="default"
        class="w-full"
        onclick={onRunSteps}
        disabled={!wasmReady || workerRunning || hasValidationErrors}
        >Run {steps} Steps</Button
      >
      <Button
        size="sm"
        variant="secondary"
        class="w-full"
        onclick={onStepOnce}
        disabled={!wasmReady || workerRunning || hasValidationErrors}
        >Step Once</Button
      >
      <Button
        size="sm"
        variant="destructive"
        class="w-full"
        onclick={onStopRun}
        disabled={!canStop}>Stop</Button
      >
      <Button
        size="sm"
        variant="outline"
        class="w-full"
        onclick={onInitSimulator}
        disabled={!wasmReady || workerRunning || hasValidationErrors}
        >Reinitialize Simulator</Button
      >
    </div>

    <div class="text-xs text-muted-foreground mt-2 space-y-1">
      {#if modelReinitNotice}
        <div class="text-destructive font-semibold">⚠ {modelReinitNotice}</div>
      {/if}
      <div>Status: {wasmReady ? "WASM Ready" : "WASM Loading..."}</div>
      <div>
        Worker: {workerRunning ? "Running" : "Idle"} · Run Completed: {runCompleted
          ? "Yes"
          : "No"}
      </div>
      {#if solverWarning}
        <div class="text-destructive font-semibold">⚠ {solverWarning}</div>
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
</Collapsible>
