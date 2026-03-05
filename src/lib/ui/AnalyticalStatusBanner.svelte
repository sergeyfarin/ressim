<script lang="ts">
  type AnalyticalReasonSeverity = "notice" | "warning" | "critical";

  type AnalyticalStatusReason = {
    code: string;
    message: string;
    severity: AnalyticalReasonSeverity;
  };

  type AnalyticalStatus = {
    level: "reference" | "approximate" | "off";
    mode: "waterflood" | "depletion" | "none";
    warningSeverity: "none" | AnalyticalReasonSeverity;
    reasonDetails: AnalyticalStatusReason[];
    reasons: string[];
  };

  let {
    status,
    className = "",
  }: {
    status: AnalyticalStatus;
    className?: string;
  } = $props();

  let showDetails = $state(false);

  const normalizedReasons = $derived(status.reasonDetails);

  const bannerTone = $derived.by(() => {
    if (status.level === "reference") {
      return "border-success/60 bg-success/10 text-success";
    }

    if (status.level === "off") {
      return "border-border bg-muted/30 text-muted-foreground";
    }

    if (status.warningSeverity === "critical") {
      return "border-destructive/70 bg-destructive/10 text-destructive";
    }

    if (status.warningSeverity === "notice") {
      return "border-info/60 bg-info/10 text-info";
    }

    return "border-warning/80 bg-warning/15 text-warning";
  });

  const summary = $derived.by(() => {
    if (status.level === "reference") {
      return "Analytical overlay matches reference assumptions for this scenario.";
    }
    if (status.level === "off") {
      return "Analytical overlay is disabled.";
    }

    if (status.warningSeverity === "critical") {
      return "Analytical overlay is approximate with critical caveats.";
    }

    return "Analytical overlay is approximate; review caveats before comparing absolute values.";
  });

  const modeLabel = $derived.by(() => {
    if (status.mode === "waterflood") return "Waterflood";
    if (status.mode === "depletion") return "Depletion";
    return "None";
  });

  function severityTone(severity: AnalyticalReasonSeverity): string {
    if (severity === "critical") {
      return "border-destructive/70 bg-destructive/10 text-destructive";
    }
    if (severity === "warning") {
      return "border-warning/70 bg-warning/10 text-warning";
    }
    return "border-info/70 bg-info/10 text-info";
  }

  function severityLabel(severity: AnalyticalReasonSeverity): string {
    return severity.toUpperCase();
  }
</script>

<section
  class={`rounded-lg border p-4 shadow-sm ${bannerTone} ${className}`}
  role="status"
  aria-live="polite"
>
  <div class="flex flex-wrap items-start justify-between gap-2">
    <div>
      <div class="text-[11px] font-semibold uppercase tracking-wide opacity-90">
        Analytical Overlay Status
      </div>
      <p class="mt-1 text-sm font-semibold leading-snug">
        {summary}
      </p>
    </div>
    <div class="flex flex-wrap gap-1.5 text-[10px] font-semibold uppercase tracking-wide">
      <span class="rounded border border-current/40 bg-card/70 px-2 py-1">
        {status.level}
      </span>
      <span class="rounded border border-current/40 bg-card/70 px-2 py-1">
        {modeLabel}
      </span>
      {#if status.warningSeverity !== "none"}
        <span class="rounded border border-current/40 bg-card/70 px-2 py-1">
          {status.warningSeverity}
        </span>
      {/if}
    </div>
  </div>

  {#if normalizedReasons.length > 0}
    <div class="mt-3 border-t border-current/20 pt-3">
      <button
        class="rounded border border-current/40 bg-card/70 px-2 py-1 text-[11px] font-semibold transition-opacity hover:opacity-90"
        type="button"
        aria-expanded={showDetails}
        onclick={() => (showDetails = !showDetails)}
      >
        {showDetails ? "Hide caveat details" : "Show caveat details"}
      </button>

      {#if showDetails}
        <ul class="mt-2 space-y-2 text-xs">
          {#each normalizedReasons as reason}
            <li class="flex items-start gap-2 rounded border border-current/25 bg-card/70 p-2">
              <span
                class={`rounded border px-1.5 py-0.5 text-[10px] font-bold uppercase tracking-wide ${severityTone(reason.severity)}`}
                title={`Reason code: ${reason.code}`}
              >
                {severityLabel(reason.severity)}
              </span>
              <span class="text-foreground/90">{reason.message}</span>
            </li>
          {/each}
        </ul>
      {/if}
    </div>
  {/if}
</section>
