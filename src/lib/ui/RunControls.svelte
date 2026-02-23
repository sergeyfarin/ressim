<script lang="ts">
  import Card from "../components/ui/Card.svelte";
  import Input from "../components/ui/Input.svelte";
  import Button from "../components/ui/Button.svelte";

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
    historyInterval = $bindable(1),
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
    historyInterval?: number;
    onRunSteps?: () => void;
    onStepOnce?: () => void;
    onInitSimulator?: () => void;
    onStopRun?: () => void;
  } = $props();
</script>

<Card>
  <div class="p-3 md:p-4">
    <div class="flex flex-wrap items-center gap-3">
      <!-- Steps input -->
      <label class="flex items-center gap-2">
        <span class="text-xs font-medium whitespace-nowrap">Steps:</span>
        <Input type="number" min="1" class="w-20" bind:value={steps} />
      </label>

      <label class="flex items-center gap-2">
        <span class="text-xs font-medium whitespace-nowrap">Render Every:</span>
        <Input
          type="number"
          min="1"
          class="w-16"
          bind:value={historyInterval}
        />
      </label>

      <!-- Action buttons -->
      <div class="flex flex-wrap gap-2">
        <Button
          size="sm"
          variant="default"
          onclick={onRunSteps}
          disabled={!wasmReady || workerRunning || hasValidationErrors}
          >▶ Run {steps} Steps</Button
        >

        <Button
          size="sm"
          variant="outline"
          onclick={onStepOnce}
          disabled={!wasmReady || workerRunning || hasValidationErrors}
          >Step Once</Button
        >

        <Button
          size="sm"
          variant="destructive"
          onclick={onStopRun}
          disabled={!canStop}>⏹ Stop</Button
        >

        <Button
          size="sm"
          variant="ghost"
          onclick={onInitSimulator}
          disabled={!wasmReady || workerRunning || hasValidationErrors}
          >↻ Reinit</Button
        >

        {#if inputsAnchorHref}
          <a
            class="text-primary text-xs self-center underline-offset-4 hover:underline"
            href={inputsAnchorHref}>Jump to Inputs</a
          >
        {/if}
      </div>

      <!-- Status -->
      <div
        class="flex flex-wrap items-center gap-3 text-xs text-muted-foreground ml-auto"
      >
        <span
          class="inline-flex items-center rounded-full border px-2.5 py-0.5 text-xs font-semibold font-mono transition-colors focus:outline-none focus:ring-2 focus:ring-ring focus:ring-offset-2 {wasmReady
            ? 'bg-primary text-primary-foreground text-[10px]'
            : 'bg-destructive text-destructive-foreground text-[10px]'}"
        >
          {wasmReady ? "WASM Ready" : "Loading…"}
        </span>
        <span
          >{continuationStatus ||
            (workerRunning
              ? "⏳ Running"
              : runCompleted
                ? "✓ Done"
                : "○ Idle")}</span
        >
        <span>{simTime.toFixed(1)} days</span>
        <span>{historyLength} snapshots</span>
        {#if runProgress}
          <span class="text-primary font-medium">{runProgress}</span>
        {/if}
      </div>
    </div>

    {#if continuationStatus}
      <div class="text-xs text-primary mt-1">{continuationStatus}</div>
    {/if}

    <!-- Warnings / notices -->
    {#if modelReinitNotice}
      <div class="text-xs text-destructive mt-1">⚠ {modelReinitNotice}</div>
    {/if}
    {#if solverWarning}
      <div class="text-xs text-destructive mt-1">⚠ {solverWarning}</div>
    {/if}
    {#if longRunEstimate}
      <div class="text-xs text-muted-foreground mt-1">
        Estimated: {estimatedRunSeconds.toFixed(1)}s — you can stop at any time
      </div>
    {/if}
  </div>
</Card>
