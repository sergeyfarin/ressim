<script lang="ts">
    import type { HTMLLabelAttributes } from "svelte/elements";
    let { children, class: className, ...rest }: HTMLLabelAttributes = $props();
</script>

<label
    class="text-sm font-medium leading-none peer-disabled:cursor-not-allowed peer-disabled:opacity-70 {className}"
    {...rest}
>
    {@render children?.()}
</label>
