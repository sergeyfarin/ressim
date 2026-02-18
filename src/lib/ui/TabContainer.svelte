<script lang="ts">
  export let activeTab: 'charts' | '3d' | 'inputs' = 'charts';
  export let onTabChange: (tab: 'charts' | '3d' | 'inputs') => void = () => {};

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
        on:click={() => onTabChange(tab.key)}
      >
        {tab.label}
      </button>
    {/each}
  </div>

  <!-- Tab content -->
  <div class="mt-3">
    {#if activeTab === 'charts'}
      <slot name="charts" />
    {:else if activeTab === '3d'}
      <slot name="3d" />
    {:else if activeTab === 'inputs'}
      <slot name="inputs" />
    {/if}
  </div>
</div>
