<script lang="ts">
    import type { HTMLSelectAttributes } from "svelte/elements";
    let {
        value = $bindable(),
        children,
        class: className,
        ...rest
    }: HTMLSelectAttributes & {
        value?: string | number | undefined;
    } = $props();
</script>

<select
    bind:value
    class="flex h-9 w-full items-center justify-between rounded-md border border-input bg-background text-foreground px-3 py-2 text-sm shadow-sm ring-offset-background placeholder:text-muted-foreground focus:outline-none focus:ring-1 focus:ring-ring disabled:cursor-not-allowed disabled:opacity-50 {className}"
    {...rest}
>
    {@render children?.()}
</select>
