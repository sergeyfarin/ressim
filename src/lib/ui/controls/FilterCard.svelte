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

    import ToggleGroup from "./ToggleGroup.svelte";

    const formattedOptions = $derived(
        options.map((opt: string) => ({
            value: opt,
            label: customLabels[opt] ?? opt,
            disabled: disabled?.includes(opt),
            title: disabled?.includes(opt)
                ? (disabledReasons?.[opt] ?? "Not available")
                : undefined,
        })),
    );
</script>

<div class="filter-card">
    <span class="filter-label">{label}</span>
    <ToggleGroup
        options={formattedOptions}
        value={selected}
        onChange={(val: string | number) => onchange(val as string)}
        wrap={options.length >= 4}
    />
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
</style>
