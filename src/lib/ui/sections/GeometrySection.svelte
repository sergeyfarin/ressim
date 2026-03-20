<script lang="ts">
  import Input from "../controls/Input.svelte";
  import type { ModePanelParameterBindings } from "../modePanelTypes";

  let {
    bindings,
    fieldErrors = {},
    onParamEdit = () => {},
    showHeader = true,
    hideQuickPickOptions = false,
  }: {
    bindings: ModePanelParameterBindings;
    fieldErrors?: Record<string, string>;
    onParamEdit?: () => void;
    showHeader?: boolean;
    hideQuickPickOptions?: boolean;
  } = $props();

  const totalX = $derived(bindings.nx * bindings.cellDx);
  const totalY = $derived(bindings.ny * bindings.cellDy);
  const totalZ = $derived(bindings.nz * bindings.cellDz);
  const totalCells = $derived(bindings.nx * bindings.ny * bindings.nz);

  function fmtLen(v: number): string {
    return v >= 1000 ? `${(v / 1000).toFixed(2)} km` : `${v.toFixed(1)} m`;
  }

  function setInt(param: "nx" | "ny" | "nz", raw: string) {
    const v = parseInt(raw, 10);
    if (!Number.isFinite(v) || v < 1) return;
    bindings[param] = v;
    if (param === "nz") bindings.handleNzOrPermModeChange();
  }

  function setFloat(param: "cellDx" | "cellDy" | "cellDz", raw: string) {
    const v = Number(raw);
    if (!Number.isFinite(v) || v <= 0) return;
    bindings[param] = v;
  }
</script>

<div class="dense-section rounded-lg border border-border bg-card shadow-sm">
  <div class="overflow-x-auto rounded-md border border-border">
    <table class="dense-table w-full text-left">
      <thead class="border-b border-border bg-muted/50 text-[10px] uppercase tracking-wide text-muted-foreground">
        <tr>
          <th class="px-2 py-1.5 font-medium">Axis</th>
          <th class="px-2 py-1.5 font-medium">Cells</th>
          <th class="px-2 py-1.5 font-medium">Size (m)</th>
          <th class="px-2 py-1.5 font-medium text-right">Total</th>
        </tr>
      </thead>
      <tbody class="divide-y divide-border text-xs">
        <tr>
          <td class="px-2 py-1 font-medium text-muted-foreground">X</td>
          <td class="px-1 py-1">
            <Input type="number" min={1} step={1} class={`w-full h-7 px-2 text-xs ${fieldErrors.nx ? 'border-destructive' : ''}`}
              value={bindings.nx} oninput={(e) => setInt('nx', (e.currentTarget as HTMLInputElement).value)} />
          </td>
          <td class="px-1 py-1">
            <Input type="number" min={0.1} step={1} class={`w-full h-7 px-2 text-xs ${fieldErrors.cellDx ? 'border-destructive' : ''}`}
              value={bindings.cellDx} oninput={(e) => setFloat('cellDx', (e.currentTarget as HTMLInputElement).value)} />
          </td>
          <td class="px-2 py-1 text-right text-muted-foreground tabular-nums">{fmtLen(totalX)}</td>
        </tr>
        <tr>
          <td class="px-2 py-1 font-medium text-muted-foreground">Y</td>
          <td class="px-1 py-1">
            <Input type="number" min={1} step={1} class={`w-full h-7 px-2 text-xs ${fieldErrors.ny ? 'border-destructive' : ''}`}
              value={bindings.ny} oninput={(e) => setInt('ny', (e.currentTarget as HTMLInputElement).value)} />
          </td>
          <td class="px-1 py-1">
            <Input type="number" min={0.1} step={1} class={`w-full h-7 px-2 text-xs ${fieldErrors.cellDy ? 'border-destructive' : ''}`}
              value={bindings.cellDy} oninput={(e) => setFloat('cellDy', (e.currentTarget as HTMLInputElement).value)} />
          </td>
          <td class="px-2 py-1 text-right text-muted-foreground tabular-nums">{fmtLen(totalY)}</td>
        </tr>
        <tr>
          <td class="px-2 py-1 font-medium text-muted-foreground">Z</td>
          <td class="px-1 py-1">
            <Input type="number" min={1} step={1} class={`w-full h-7 px-2 text-xs ${fieldErrors.nz ? 'border-destructive' : ''}`}
              value={bindings.nz} oninput={(e) => setInt('nz', (e.currentTarget as HTMLInputElement).value)} />
          </td>
          <td class="px-1 py-1">
            <Input type="number" min={0.1} step={0.5} class={`w-full h-7 px-2 text-xs ${fieldErrors.cellDz ? 'border-destructive' : ''}`}
              value={bindings.cellDz} oninput={(e) => setFloat('cellDz', (e.currentTarget as HTMLInputElement).value)} />
          </td>
          <td class="px-2 py-1 text-right text-muted-foreground tabular-nums">{fmtLen(totalZ)}</td>
        </tr>
      </tbody>
    </table>
  </div>
  <div class="mt-1 flex justify-between text-[10px] text-muted-foreground tabular-nums">
    <span>{totalCells.toLocaleString()} cells</span>
    <span>{fmtLen(totalX)} × {fmtLen(totalY)} × {fmtLen(totalZ)}</span>
  </div>
</div>

<style>
  .dense-section {
    padding: 0.5rem;
  }
</style>
