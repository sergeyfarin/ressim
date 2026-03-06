<script lang="ts">
  import type {
    WarningPolicy,
    WarningPolicyGroup,
    WarningPolicyGroupKey,
  } from "../warningPolicy";

  let {
    policy,
    groups = ["blockingValidation", "nonPhysical", "advisory"],
  }: {
    policy: WarningPolicy;
    groups?: WarningPolicyGroupKey[];
  } = $props();

  const visibleGroups = $derived(
    groups
      .map((key) => policy[key])
      .filter((group): group is WarningPolicyGroup => group.items.length > 0),
  );

  function toneClass(group: WarningPolicyGroup): string {
    if (group.tone === "destructive") {
      return "border-destructive/70 bg-destructive/8 text-destructive";
    }
    if (group.tone === "warning") {
      return "border-warning/70 bg-warning/10 text-warning";
    }
    return "border-info/70 bg-info/10 text-info";
  }
</script>

{#if visibleGroups.length > 0}
  <div class="warning-stack">
    {#each visibleGroups as group}
      <section class={`warning-group ${toneClass(group)}`}>
        <div class="flex flex-wrap items-start justify-between gap-2">
          <div>
            <div class="text-[11px] font-semibold uppercase tracking-wide">
              {group.title}
            </div>
            <p class="mt-1 text-xs opacity-90">{group.description}</p>
          </div>
          <span class="rounded border border-current/35 bg-card/75 px-2 py-1 text-[10px] font-semibold uppercase tracking-wide">
            {group.items.length} item{group.items.length === 1 ? "" : "s"}
          </span>
        </div>

        <ul class="mt-2 space-y-1.5 text-xs">
          {#each group.items as item}
            <li class="rounded border border-current/20 bg-card/70 px-2.5 py-2">
              {item.message}
            </li>
          {/each}
        </ul>
      </section>
    {/each}
  </div>
{/if}

<style>
  .warning-stack {
    display: grid;
    gap: 0.75rem;
    margin-top: 0.85rem;
  }

  .warning-group {
    border: 1px solid;
    border-radius: var(--radius);
    padding: 0.8rem 0.9rem;
  }
</style>
