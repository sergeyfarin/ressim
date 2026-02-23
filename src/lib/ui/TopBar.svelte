<script lang="ts">
  import { caseCatalog, categoryKeys } from "../caseCatalog";
  import Button from "../components/ui/Button.svelte";

  let {
    activeCategory = "",
    activeCase = "",
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

  const activeCases = $derived(
    activeCategory && caseCatalog[activeCategory]
      ? caseCatalog[activeCategory].cases
      : [],
  );

  const isCustomSubCaseActive = $derived(
    !isCustomMode &&
      Boolean(customSubCase?.key) &&
      activeCase === customSubCase?.key,
  );
</script>

<div class="space-y-3">
  <!-- Category pill buttons -->
  <div class="flex flex-wrap gap-2">
    {#each categoryKeys as catKey}
      <Button
        size="sm"
        variant={activeCategory === catKey && !isCustomMode
          ? "default"
          : "outline"}
        onclick={() => onCategoryChange(catKey)}
      >
        {caseCatalog[catKey].label}
      </Button>
    {/each}
    <Button
      size="sm"
      variant={isCustomMode ? "default" : "outline"}
      onclick={onCustomMode}
    >
      ⚙ Custom
    </Button>
  </div>

  <!-- Case selector (only shown when a category is selected and not custom mode) -->
  {#if activeCategory && !isCustomMode && activeCases.length > 0}
    <div class="flex flex-wrap gap-2">
      {#each activeCases as caseEntry}
        <Button
          size="sm"
          variant={activeCase === caseEntry.key ? "secondary" : "outline"}
          onclick={() => onCaseChange(caseEntry.key)}
          title={caseEntry.description}
        >
          {caseEntry.label}
        </Button>
      {/each}
      {#if isCustomSubCaseActive}
        <Button size="sm" variant="secondary" disabled>
          {customSubCase?.label}
        </Button>
      {/if}
    </div>
    {#if activeCategory && caseCatalog[activeCategory]}
      <p class="text-xs text-muted-foreground">
        {caseCatalog[activeCategory].description}
      </p>
    {/if}
  {/if}

  {#if isCustomMode}
    <p class="text-xs text-muted-foreground">
      Custom mode — edit all parameters in the Inputs tab, then run the
      simulation.
    </p>
  {/if}
</div>
