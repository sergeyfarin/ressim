<script lang="ts">
    let {
        label = "",
        options = [] as string[],
        selected = "",
        disabled = [] as string[],
        disabledReasons = {} as Record<string, string>,
        customLabels = {} as Record<string, string>,
        onchange = (_v: string) => {},
    } = $props<{
        label: string;
        options: string[];
        selected: string;
        disabled?: string[];
        disabledReasons?: Record<string, string>;
        customLabels?: Record<string, string>;
        onchange: (v: string) => void;
    }>();
</script>

<div class="filter-card">
    <span class="filter-label">{label}</span>
    <div class="flex flex-wrap gap-1">
        {#each options as opt}
            {@const isDisabled = disabled.includes(opt)}
            {@const isActive = selected === opt}
            <button
                class="filter-pill"
                class:active={isActive}
                class:disabled={isDisabled}
                disabled={isDisabled}
                title={isDisabled
                    ? (disabledReasons[opt] ?? "Not available")
                    : (customLabels[opt] ?? opt)}
                onclick={() => {
                    if (!isDisabled) onchange(opt);
                }}
            >
                {customLabels[opt] ?? opt}
            </button>
        {/each}
    </div>
</div>

<style>
    .filter-card {
        display: inline-flex;
        flex-direction: column;
        gap: 3px;
        border: 1px solid hsl(var(--border) / 0.6);
        border-radius: var(--radius);
        padding: 5px 8px 6px;
        background: hsl(var(--card) / 0.5);
        min-width: 0;
    }
    .filter-label {
        font-size: 10px;
        font-weight: 600;
        text-transform: uppercase;
        letter-spacing: 0.05em;
        color: hsl(var(--muted-foreground));
        line-height: 1;
    }
    .filter-pill {
        font-size: 11px;
        padding: 2px 7px;
        border-radius: 999px;
        border: 1px solid hsl(var(--border) / 0.5);
        background: transparent;
        color: hsl(var(--foreground) / 0.8);
        cursor: pointer;
        transition: all 0.15s ease;
        line-height: 1.3;
        white-space: nowrap;
    }
    .filter-pill:hover:not(.disabled) {
        background: hsl(var(--accent) / 0.15);
        border-color: hsl(var(--accent));
    }
    .filter-pill.active {
        background: hsl(var(--primary));
        color: hsl(var(--primary-foreground));
        border-color: hsl(var(--primary));
        font-weight: 600;
    }
    .filter-pill.disabled {
        opacity: 0.35;
        cursor: not-allowed;
        text-decoration: line-through;
    }
</style>
