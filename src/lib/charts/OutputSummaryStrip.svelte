<script lang="ts">
    import type { OutputSummaryItem } from './outputSummary';

    let {
        items = [],
    }: {
        items?: OutputSummaryItem[];
    } = $props();

    function toneClasses(tone: OutputSummaryItem['tone']) {
        if (tone === 'positive') {
            return 'border-emerald-500/25 bg-emerald-500/8';
        }

        if (tone === 'warning') {
            return 'border-amber-500/25 bg-amber-500/8';
        }

        return 'border-border/60 bg-muted/10';
    }
</script>

{#if items.length > 0}
    <div class="grid gap-2 sm:grid-cols-2 xl:grid-cols-4">
        {#each items as item}
            <article class={`rounded-md border px-3 py-2.5 ${toneClasses(item.tone)}`}>
                <div class="ui-panel-kicker">{item.label}</div>
                <div class="mt-2 text-base font-semibold text-foreground">
                    {item.value}
                </div>
                <div class="ui-microcopy mt-1">
                    {item.detail}
                </div>
            </article>
        {/each}
    </div>
{/if}