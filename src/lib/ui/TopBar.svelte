<script lang="ts">
  import { caseCatalog, categoryKeys } from '../caseCatalog';

  let {
    activeCategory = '',
    activeCase = '',
    isCustomMode = false,
    customSubCase = null,
    onCategoryChange = () => {},
    onCaseChange = () => {},
    onCustomMode = () => {},
  }: {
    activeCategory?: string;
    activeCase?: string;
    isCustomMode?: boolean;
    customSubCase?: { key: string; label: string } | null;
    onCategoryChange?: (cat: string) => void;
    onCaseChange?: (key: string) => void;
    onCustomMode?: () => void;
  } = $props();

  const activeCases = $derived(activeCategory && caseCatalog[activeCategory]
    ? caseCatalog[activeCategory].cases
    : []);

  const isCustomSubCaseActive = $derived(
    !isCustomMode &&
      Boolean(customSubCase?.key) &&
      activeCase === customSubCase?.key
  );
</script>

<div class="space-y-3">
  <!-- Category pill buttons -->
  <div class="flex flex-wrap gap-2">
    {#each categoryKeys as catKey}
      <button
        class="btn btn-sm {activeCategory === catKey && !isCustomMode ? 'btn-primary' : 'btn-outline'}"
        onclick={() => onCategoryChange(catKey)}
      >
        {caseCatalog[catKey].label}
      </button>
    {/each}
    <button
      class="btn btn-sm {isCustomMode ? 'btn-accent' : 'btn-outline'}"
      onclick={onCustomMode}
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
          onclick={() => onCaseChange(caseEntry.key)}
          title={caseEntry.description}
        >
          {caseEntry.label}
        </button>
      {/each}
      {#if isCustomSubCaseActive}
        <button class="btn btn-sm btn-secondary" disabled>
          {customSubCase?.label}
        </button>
      {/if}
    </div>
    {#if activeCategory && caseCatalog[activeCategory]}
      <p class="text-xs opacity-60">{caseCatalog[activeCategory].description}</p>
    {/if}
  {/if}

  {#if isCustomMode}
    <p class="text-xs opacity-60">Custom mode — edit all parameters in the Inputs tab, then run the simulation.</p>
  {/if}
</div>
