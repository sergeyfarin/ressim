<script lang="ts">
  import type { Snippet } from "svelte";

  let {
    activeTab = "charts",
    onTabChange = () => {},
    charts,
    threeD,
    inputs,
  }: {
    activeTab?: "charts" | "3d" | "inputs";
    onTabChange?: (tab: "charts" | "3d" | "inputs") => void;
    charts?: Snippet;
    threeD?: Snippet;
    inputs?: Snippet;
  } = $props();

  const tabs = [
    {
      key: "charts",
      label: "📊 Charts",
      title: "Rate charts, Sw profile, and analytical comparisons",
    },
    {
      key: "3d",
      label: "🧊 3D Visualization",
      title: "3D grid visualization with property coloring",
    },
    { key: "inputs", label: "⚙ Inputs", title: "Edit simulation parameters" },
  ] as const;
</script>

<div class="w-full">
  <!-- Tab header -->
  <div
    class="inline-flex h-9 items-center justify-center rounded-lg bg-muted p-1 text-muted-foreground w-full max-w-fit mb-4"
  >
    {#each tabs as tab}
      <button
        role="tab"
        class="inline-flex items-center justify-center whitespace-nowrap rounded-md px-3 py-1 text-sm font-medium ring-offset-background transition-all focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 disabled:pointer-events-none disabled:opacity-50
        {activeTab === tab.key
          ? 'bg-background text-foreground shadow'
          : 'hover:bg-background/50 hover:text-foreground'}"
        title={tab.title}
        onclick={() => onTabChange(tab.key)}
      >
        {tab.label}
      </button>
    {/each}
  </div>

  <!-- Tab content -->
  <div
    class="mt-2 ring-offset-background focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2"
  >
    {#if activeTab === "charts"}
      {@render charts?.()}
    {:else if activeTab === "3d"}
      {@render threeD?.()}
    {:else if activeTab === "inputs"}
      {@render inputs?.()}
    {/if}
  </div>
</div>
