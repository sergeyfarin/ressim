<script lang="ts">
  import WarningPolicyPanel from "./WarningPolicyPanel.svelte";
  import type { WarningPolicy } from "../../warningPolicy";

  let {
    policy,
    className = "",
  }: {
    policy: WarningPolicy;
    className?: string;
  } = $props();

  const hasReferenceCaveats = $derived(
    policy.referenceCaveat.items.length > 0,
  );
</script>

{#if hasReferenceCaveats}
  <div class={className}>
    <WarningPolicyPanel
      {policy}
      groups={["referenceCaveat"]}
      groupSources={{ referenceCaveat: ["analytical"] }}
    />
  </div>
{/if}
