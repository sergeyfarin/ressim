<script lang="ts">
    type OptionValue = string | number;

    let {
        options = [],
        value = $bindable(),
        onChange = () => {},
        wrap = false,
    }: {
        options?: Array<{
            value: OptionValue;
            label: string;
            title?: string;
            disabled?: boolean;
        }>;
        value: OptionValue;
        onChange?: (val: OptionValue) => void;
        wrap?: boolean;
    } = $props();
</script>

<div
    class="shadow-sm overflow-hidden shrink-0 {wrap
        ? options.length === 4
            ? 'grid grid-cols-2 rounded-md border border-border bg-border gap-px'
            : 'grid grid-cols-3 rounded-md border border-border bg-border gap-px'
        : 'inline-flex rounded-md border border-border'}"
>
    {#each options as option, index}
        <button
            type="button"
            disabled={option.disabled}
            class="px-2 py-1 text-[11px] font-medium transition-colors {wrap
                ? value === option.value
                    ? 'bg-primary text-primary-foreground'
                    : 'bg-card text-muted-foreground hover:bg-muted/80'
                : (index > 0 ? 'border-l border-border ' : '') +
                  (value === option.value
                      ? 'bg-primary text-primary-foreground'
                      : 'bg-transparent text-muted-foreground hover:bg-muted/50 hover:text-foreground')}
                disabled:opacity-30 disabled:cursor-not-allowed"
            onclick={() => {
                if (!option.disabled) {
                    value = option.value;
                    onChange(option.value);
                }
            }}
            title={option.title}
        >
            {option.label}
        </button>
    {/each}
</div>
