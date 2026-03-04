<script lang="ts">
    import { tweened } from "svelte/motion";
    import { cubicOut } from "svelte/easing";

    let {
        label,
        options,
        selected = $bindable<string[]>(),
    } = $props<{
        label: string;
        options: readonly string[];
        selected: string[];
    }>();

    function toggle(opt: string) {
        if (selected.includes(opt)) {
            selected = selected.filter((s) => s !== opt);
        } else {
            selected = [...selected, opt];
        }
    }

    // Animation for count badge
    let matchCount = $derived(selected.length);

    $effect(() => {
        // optional animation logic
    });
</script>

<div class="filter-card">
    <div class="card-header">
        <span class="filter-label">{label}</span>
        {#if matchCount > 0}
            <span class="count-badge">{matchCount}</span>
        {/if}
    </div>
    <div class="pill-container">
        {#each options as opt}
            <button
                class="filter-pill"
                class:active={selected.includes(opt)}
                onclick={() => toggle(opt)}
            >
                {opt}
            </button>
        {/each}
    </div>
</div>

<style>
    .filter-card {
        display: inline-flex;
        flex-direction: column;
        gap: 4px;
        padding: 6px 10px;
        border-radius: var(--radius, 0.5rem);
        border: 1px solid hsl(var(--border));
        background-color: hsl(var(--card) / 0.5);
        min-width: 100px;
    }

    .card-header {
        display: flex;
        justify-content: space-between;
        align-items: center;
        gap: 6px;
    }

    .filter-label {
        font-size: 0.7rem;
        font-weight: 600;
        text-transform: uppercase;
        letter-spacing: 0.05em;
        color: hsl(var(--muted-foreground));
    }

    .count-badge {
        background-color: hsl(var(--primary) / 0.2);
        color: hsl(var(--primary));
        font-size: 0.6rem;
        font-weight: 700;
        padding: 0 4px;
        border-radius: 4px;
    }

    .pill-container {
        display: flex;
        flex-wrap: wrap;
        gap: 4px;
    }

    .filter-pill {
        font-size: 0.7rem;
        padding: 2px 6px;
        border-radius: 4px;
        background-color: transparent;
        border: 1px solid hsl(var(--border));
        color: hsl(var(--foreground) / 0.8);
        cursor: pointer;
        transition: all 0.15s ease-in-out;
    }

    .filter-pill:hover {
        background-color: hsl(var(--secondary));
        color: hsl(var(--secondary-foreground));
    }

    .filter-pill.active {
        background-color: hsl(var(--primary));
        color: hsl(var(--primary-foreground));
        border-color: hsl(var(--primary));
    }
</style>
