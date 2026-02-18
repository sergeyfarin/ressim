<script lang="ts">
  import { caseCatalog, categoryKeys } from '../caseCatalog.js';

  export let activeCategory: string = '';
  export let activeCase: string = '';
  export let isCustomMode: boolean = false;
  export let onCategoryChange: (cat: string) => void = () => {};
  export let onCaseChange: (key: string) => void = () => {};
  export let onCustomMode: () => void = () => {};

  $: activeCases = activeCategory && caseCatalog[activeCategory]
    ? caseCatalog[activeCategory].cases
    : [];
</script>

<div class="space-y-3">
  <!-- Category pill buttons -->
  <div class="flex flex-wrap gap-2">
    {#each categoryKeys as catKey}
      <button
        class="btn btn-sm {activeCategory === catKey && !isCustomMode ? 'btn-primary' : 'btn-outline'}"
        on:click={() => onCategoryChange(catKey)}
      >
        {caseCatalog[catKey].label}
      </button>
    {/each}
    <button
      class="btn btn-sm {isCustomMode ? 'btn-accent' : 'btn-outline'}"
      on:click={onCustomMode}
    >
      ⚙ Custom
    </button>
  </div>

  <!-- Case selector (only shown when a category is selected and not custom mode) -->
  {#if activeCategory && !isCustomMode && activeCases.length > 0}
    <div class="flex flex-wrap gap-2">
      {#each activeCases as caseEntry}
        <button
          class="btn btn-sm {activeCase === caseEntry.key ? 'btn-secondary' : 'btn-ghost border border-base-300'}"
          on:click={() => onCaseChange(caseEntry.key)}
          title={caseEntry.description}
        >
          {caseEntry.label}
        </button>
      {/each}
    </div>
    {#if activeCategory && caseCatalog[activeCategory]}
      <p class="text-xs opacity-60">{caseCatalog[activeCategory].description}</p>
    {/if}
  {/if}

  {#if isCustomMode}
    <p class="text-xs opacity-60">Custom mode — edit all parameters in the Inputs tab, then run the simulation.</p>
  {/if}
</div>
