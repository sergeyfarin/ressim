<script lang="ts">
  import type { HTMLInputAttributes } from "svelte/elements";
  import Input from "./Input.svelte";

  let {
    value = $bindable(),
    error,
    class: className = "",
    ...rest
  }: HTMLInputAttributes & { value?: string | number | undefined; error?: string } = $props();
</script>

<Input
  bind:value
  class="{className} {error ? 'border-destructive' : ''}"
  {...rest}
/>
{#if error}
  <div class="text-[10px] text-destructive mt-1 leading-tight">{error}</div>
{/if}
