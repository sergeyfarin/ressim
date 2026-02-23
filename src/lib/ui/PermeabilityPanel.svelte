<script lang="ts">
  import Card from "../components/ui/Card.svelte";
  import Select from "../components/ui/Select.svelte";
  import Input from "../components/ui/Input.svelte";

  let {
    permMode = $bindable("default"),
    useRandomSeed = $bindable(true),
    randomSeed = $bindable(12345),
    minPerm = $bindable(50),
    maxPerm = $bindable(200),
    layerPermsXStr = $bindable(""),
    layerPermsYStr = $bindable(""),
    layerPermsZStr = $bindable(""),
  }: {
    permMode?: string;
    useRandomSeed?: boolean;
    randomSeed?: number;
    minPerm?: number;
    maxPerm?: number;
    layerPermsXStr?: string;
    layerPermsYStr?: string;
    layerPermsZStr?: string;
  } = $props();
</script>

<Card>
  <div class="space-y-3 p-4 md:p-5">
    <h2 class="text-base font-semibold">Permeability</h2>
    <p class="-mt-1 text-xs font-medium text-muted-foreground">
      Distribution controls for permeability fields.
    </p>
    <label class="flex flex-col gap-1.5 mt-2">
      <span class="text-xs font-medium">Permeability Mode</span>
      <Select class="w-full" bind:value={permMode}>
        <option value="default">Default</option>
        <option value="random">Random</option>
        <option value="perLayer">Per Layer</option>
      </Select>
    </label>

    {#if permMode === "random"}
      <label class="flex items-center gap-2 cursor-pointer mt-2">
        <input
          type="checkbox"
          class="h-4 w-4 rounded border-input text-primary accent-primary"
          bind:checked={useRandomSeed}
        />
        <span class="text-sm font-medium leading-none"
          >Use Seeded Randomness</span
        >
      </label>
      <div class="grid grid-cols-2 gap-2 mt-2">
        {#if useRandomSeed}
          <label class="col-span-2 flex flex-col gap-1.5">
            <span class="text-xs font-medium">Random Seed</span>
            <Input
              type="number"
              step="1"
              class="w-full max-w-40"
              bind:value={randomSeed}
            />
          </label>
        {/if}
        <label class="flex flex-col gap-1.5">
          <span class="text-xs font-medium">Min Perm (mD)</span>
          <Input type="number" class="w-full" bind:value={minPerm} />
        </label>
        <label class="flex flex-col gap-1.5">
          <span class="text-xs font-medium">Max Perm (mD)</span>
          <Input type="number" class="w-full" bind:value={maxPerm} />
        </label>
      </div>
    {:else if permMode === "perLayer"}
      <div class="space-y-2 mt-2">
        <label class="flex flex-col gap-1.5">
          <span class="text-xs font-medium">Permeability X by Layer (CSV)</span>
          <Input type="text" class="w-full" bind:value={layerPermsXStr} />
        </label>
        <label class="flex flex-col gap-1.5">
          <span class="text-xs font-medium">Permeability Y by Layer (CSV)</span>
          <Input type="text" class="w-full" bind:value={layerPermsYStr} />
        </label>
        <label class="flex flex-col gap-1.5">
          <span class="text-xs font-medium">Permeability Z by Layer (CSV)</span>
          <Input type="text" class="w-full" bind:value={layerPermsZStr} />
        </label>
      </div>
    {/if}
  </div>
</Card>
