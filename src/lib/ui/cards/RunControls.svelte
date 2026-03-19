<script lang="ts">
  import Button from "../controls/Button.svelte";
  import Card from "../controls/Card.svelte";
  import Input from "../controls/Input.svelte";
  import WarningPolicyPanel from "../feedback/WarningPolicyPanel.svelte";
  import type { WarningPolicy } from "../../warningPolicy";

  let {
    wasmReady = false,
    workerRunning = false,
    runCompleted = false,
    simTime = 0,
    historyLength = 0,
    hasValidationErrors = false,
    warningPolicy = undefined,
    runProgress = "",
    steps = $bindable(20),
    numSensitivities = 0,
    stopPending = false,
    onRunSteps = () => {},
    onInitSimulator = () => {},
    onStopRun = () => {},
    fieldErrors = {},
  }: {
    wasmReady?: boolean;
    workerRunning?: boolean;
    runCompleted?: boolean;
    simTime?: number;
    historyLength?: number;
    hasValidationErrors?: boolean;
    warningPolicy?: WarningPolicy;
    runProgress?: string;
    steps?: number;
    numSensitivities?: number;
    stopPending?: boolean;
    onRunSteps?: () => void;
    onInitSimulator?: () => void;
    onStopRun?: () => void;
    fieldErrors?: Record<string, string>;
  } = $props();

  const runLabel = $derived(
    numSensitivities > 0
      ? `Run ${numSensitivities} Sensitivit${numSensitivities === 1 ? "y" : "ies"}`
      : `Run ${steps} Step${steps !== 1 ? "s" : ""}`
  );

  const statusText = $derived(
    workerRunning
      ? runProgress || "Running"
      : runCompleted
        ? "Complete"
        : "Ready"
  );
</script>

<Card>
  <div class="p-3 md:p-4 space-y-2">
    <div class="flex flex-wrap items-center gap-2">
      <!-- Reset leftmost -->
      <Button
        size="sm"
        variant="ghost"
        onclick={onInitSimulator}
        disabled={!wasmReady || workerRunning || hasValidationErrors}
        >Reset Model</Button>

      <div class="h-4 w-px bg-border/60 mx-1 hidden sm:block"></div>

      <!-- Primary run button -->
      <Button
        size="sm"
        variant="success"
        onclick={onRunSteps}
        disabled={!wasmReady || workerRunning || hasValidationErrors}
        >{runLabel}</Button>

      <Button
        size="sm"
        variant="destructive"
        onclick={onStopRun}
        disabled={!workerRunning || stopPending}
        >{stopPending ? "Stopping…" : "Stop Run"}</Button>

      <div class="h-4 w-px bg-border/60 mx-1 hidden sm:block"></div>

      <!-- Steps input -->
      <label class="flex flex-col items-start gap-1">
        <div class="flex items-center gap-2">
          <span class="text-xs font-medium text-muted-foreground whitespace-nowrap">Steps:</span>
          <Input
            type="number"
            min="1"
            class={`w-20 ${Boolean(fieldErrors.steps) ? "border-destructive" : ""}`}
            bind:value={steps} />
        </div>
        {#if fieldErrors.steps}
          <div class="text-[10px] text-destructive leading-tight">{fieldErrors.steps}</div>
        {/if}
      </label>

      <!-- Inline status -->
      <div class="flex items-center gap-3 text-xs text-muted-foreground ml-auto">
        <span class={`rounded-full px-2 py-0.5 text-xs font-medium ${
          workerRunning
            ? "bg-primary/15 text-primary"
            : statusText === "Ready"
              ? "bg-success/15 text-success"
              : statusText === "Complete"
                ? "bg-success/15 text-success"
                : "bg-muted text-muted-foreground"
        }`}>{statusText}</span>
        <span class="opacity-60">·</span>
        <span>{simTime.toFixed(1)} d</span>
      </div>
    </div>

    {#if warningPolicy}
      <WarningPolicyPanel
        policy={warningPolicy}
        groups={["nonPhysical", "advisory"]}
        groupSources={{
          nonPhysical: ["runtime"],
          advisory: ["runtime"],
        }} />
    {/if}
  </div>
</Card>
