<script lang="ts">
    import type { HTMLAttributes } from "svelte/elements";
    let {
        children,
        class: className,
        ...rest
    }: HTMLAttributes<HTMLDivElement> = $props();
</script>

<div
    class="rounded-xl border bg-card text-card-foreground shadow {className}"
    {...rest}
>
    {@render children?.()}
</div>
