<script lang="ts">
  import type {
    WarningPolicy,
    WarningPolicyGroup,
    WarningPolicyGroupKey,
    WarningPolicyGroupSources,
  } from "../../warningPolicy";
  import { getWarningPolicyGroups } from "../../warningPolicy";

  let {
    policy,
    groups = ["blockingValidation", "nonPhysical", "advisory"],
    groupSources = {},
  }: {
    policy: WarningPolicy;
    groups?: WarningPolicyGroupKey[];
    groupSources?: WarningPolicyGroupSources;
  } = $props();

  const visibleGroups = $derived(
    getWarningPolicyGroups(policy, groups, groupSources).filter(
      (group): group is WarningPolicyGroup => group.items.length > 0,
    ),
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
  <div class="space-y-1.5">
    {#each visibleGroups as group}
      <div class={`rounded border px-2.5 py-1.5 text-xs ${toneClass(group)}`}>
        {#if visibleGroups.length > 1}
          <div class="ui-chip-caps mb-0.5 font-semibold opacity-60">{group.title}</div>
        {/if}
        <ul class="space-y-0.5">
          {#each group.items as item}
            <li>{item.message}</li>
          {/each}
        </ul>
      </div>
    {/each}
  </div>
{/if}
