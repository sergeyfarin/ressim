<script lang="ts">
    import type { SvelteHTMLElements } from "svelte/elements";

    type Props = SvelteHTMLElements['details'] & {
    hasError?: boolean;
};

    let {
        title,
        open = false,
        hasError = false,
        children,
        class: className,
        ...rest
    }: Props & {
        title: string;
        open?: boolean;
        hasError?: boolean;
    } = $props();
</script>

<details
    class={`group overflow-hidden rounded-xl border bg-card text-card-foreground shadow-sm transition-all ${className || ""} ${hasError ? "border-destructive" : "border-border"}`}
    {open}
    {...rest}
>
    <summary
        class="flex cursor-pointer list-none items-center justify-between bg-muted/40 px-4 py-3 text-sm font-semibold transition-colors hover:bg-muted/60 [&::-webkit-details-marker]:hidden"
    >
        {title}
        <svg
            xmlns="http://www.w3.org/2000/svg"
            width="16"
            height="16"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            stroke-width="2"
            stroke-linecap="round"
            stroke-linejoin="round"
            class="h-4 w-4 opacity-70 transition-transform duration-200 group-open:rotate-180"
        >
            <polyline points="6 9 12 15 18 9"></polyline>
        </svg>
    </summary>
    <div class="border-t border-border bg-card">
        {@render children?.()}
    </div>
</details>
