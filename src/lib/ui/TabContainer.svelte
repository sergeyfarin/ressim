<script lang="ts">
  import type { Snippet } from 'svelte';

  let {
    activeTab = 'charts',
    onTabChange = () => {},
    charts,
    threeD,
    inputs,
  }: {
    activeTab?: 'charts' | '3d' | 'inputs';
    onTabChange?: (tab: 'charts' | '3d' | 'inputs') => void;
    charts?: Snippet;
    threeD?: Snippet;
    inputs?: Snippet;
  } = $props();

  const tabs = [
    { key: 'charts', label: '📊 Charts', title: 'Rate charts, Sw profile, and analytical comparisons' },
    { key: '3d', label: '🧊 3D Visualization', title: '3D grid visualization with property coloring' },
    { key: 'inputs', label: '⚙ Inputs', title: 'Edit simulation parameters' },
  ] as const;
</script>

<div>
  <!-- Tab header -->
  <div role="tablist" class="tabs tabs-bordered tabs-sm md:tabs-md">
    {#each tabs as tab}
      <button
        role="tab"
        class="tab {activeTab === tab.key ? 'tab-active font-semibold' : ''}"
        title={tab.title}
        onclick={() => onTabChange(tab.key)}
      >
        {tab.label}
      </button>
    {/each}
  </div>

  <!-- Tab content -->
  <div class="mt-3">
    {#if activeTab === 'charts'}
      {@render charts?.()}
    {:else if activeTab === '3d'}
      {@render threeD?.()}
    {:else if activeTab === 'inputs'}
      {@render inputs?.()}
    {/if}
  </div>
</div>
