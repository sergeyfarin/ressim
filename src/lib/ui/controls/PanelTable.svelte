<script lang="ts">
  import {
    panelTableClass,
    panelTableHeadClass,
    panelTableShellClass,
  } from "../shared/panelStyles";
  import type { Snippet } from "svelte";

  let {
    columns,
    class: extraClass = "",
    children,
  }: {
    columns: string[];
    class?: string;
    children: Snippet;
  } = $props();
</script>

<div class="{panelTableShellClass} {extraClass}">
  <table class={panelTableClass}>
    <thead class={panelTableHeadClass}>
      <tr>
        {#each columns as col}
          <th class="font-medium p-2">{col}</th>
        {/each}
      </tr>
    </thead>
    <tbody class="divide-y divide-border">
      {@render children()}
    </tbody>
  </table>
</div>
