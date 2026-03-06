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
    estimatedRunSeconds = 0,
    longRunEstimate = false,
    canStop = false,
    hasValidationErrors = false,
    warningPolicy = undefined,
    continuationStatus = "",
    runProgress = "",
    inputsAnchorHref = "",
    steps = $bindable(20),
    historyInterval = $bindable(1),
    onRunSteps = () => {},
    onStepOnce = () => {},
    onInitSimulator = () => {},
    onStopRun = () => {},
    fieldErrors = {},
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
    warningPolicy?: WarningPolicy;
    continuationStatus?: string;
    runProgress?: string;
    inputsAnchorHref?: string;
    steps?: number;
    historyInterval?: number;
    onRunSteps?: () => void;
    onStepOnce?: () => void;
    onInitSimulator?: () => void;
    onStopRun?: () => void;
    fieldErrors?: Record<string, string>;
  } = $props();
</script>

<Card>
  <div class="p-3 md:p-4">
    <div class="flex flex-wrap items-center gap-3">
      <!-- Steps input -->
      <label class="flex flex-col items-start gap-1">
        <div class="flex items-center gap-2">
          <span class="text-xs font-medium whitespace-nowrap">Steps:</span>
          <Input
            type="number"
            min="1"
            class={`w-20 ${Boolean(fieldErrors.steps) ? "border-destructive" : ""}`}
            bind:value={steps} />
        </div>
        {#if fieldErrors.steps}
          <div class="text-[10px] text-destructive leading-tight">
            {fieldErrors.steps}
          </div>
        {/if}
      </label>

      <label class="flex items-center gap-2">
        <span class="text-xs font-medium whitespace-nowrap">Render Every:</span>
        <Input
          type="number"
          min="1"
          class="w-16"
          bind:value={historyInterval} />
      </label>

      <!-- Action buttons -->
      <div class="flex flex-wrap gap-2">
        <Button
          size="sm"
          variant="default"
          onclick={onRunSteps}
          disabled={!wasmReady || workerRunning || hasValidationErrors}
          >▶ Run {steps} Steps</Button>

        <Button
          size="sm"
          variant="outline"
          onclick={onStepOnce}
          disabled={!wasmReady || workerRunning || hasValidationErrors}
          >Step Once</Button>

        <Button
          size="sm"
          variant="destructive"
          onclick={onStopRun}
          disabled={!canStop}>⏹ Stop</Button>

        <Button
          size="sm"
          variant="ghost"
          onclick={onInitSimulator}
          disabled={!wasmReady || workerRunning || hasValidationErrors}
          >↻ Reinit</Button>

        {#if inputsAnchorHref}
          <a
            class="text-primary text-xs self-center underline-offset-4 hover:underline"
            href={inputsAnchorHref}>Jump to Inputs</a>
        {/if}
      </div>

      <!-- Status -->
      <div
        class="flex flex-wrap items-center gap-3 text-xs text-muted-foreground ml-auto">
        <span
          >{continuationStatus ||
            (workerRunning
              ? "⏳ Running"
              : runCompleted
                ? "✅ Done"
                : "🟢 Idle")}</span>
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

    {#if warningPolicy}
      <WarningPolicyPanel
        policy={warningPolicy}
        groups={["blockingValidation", "nonPhysical", "advisory"]}
        groupSources={{
          blockingValidation: ["validation"],
          nonPhysical: ["runtime"],
          advisory: ["runtime"],
        }} />
    {/if}
  </div>
</Card>
