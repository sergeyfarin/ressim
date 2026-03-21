<script lang="ts">
  import Collapsible from "../controls/Collapsible.svelte";
  import Input from "../controls/Input.svelte";
  import ValidatedInput from "../controls/ValidatedInput.svelte";

  let {
    s_wc = $bindable(0.2),
    s_or = $bindable(0.1),
    n_w = $bindable(2),
    n_o = $bindable(2),
    k_rw_max = $bindable(1.0),
    k_ro_max = $bindable(1.0),
    capillaryEnabled = $bindable(true),
    capillaryPEntry = $bindable(5),
    capillaryLambda = $bindable(2),
    fieldErrors = {},
  }: {
    s_wc?: number;
    s_or?: number;
    n_w?: number;
    n_o?: number;
    k_rw_max?: number;
    k_ro_max?: number;
    capillaryEnabled?: boolean;
    capillaryPEntry?: number;
    capillaryLambda?: number;
    fieldErrors?: Record<string, string>;
  } = $props();

  const width = 160;
  const height = 64;
  const pad = 6;

  function clamp(v: number, lo: number, hi: number) {
    return Math.max(lo, Math.min(hi, v));
  }

  function toPath(fn: (sw: number) => number, yMax = 1) {
    const pts: string[] = [];
    for (let i = 0; i <= 40; i++) {
      const sw = i / 40;
      const x = pad + sw * (width - pad * 2);
      const y = height - pad - (fn(sw) / yMax) * (height - pad * 2);
      pts.push(`${i === 0 ? "M" : "L"} ${x.toFixed(2)} ${y.toFixed(2)}`);
    }
    return pts.join(" ");
  }

  const numericSwc = $derived(Number(s_wc));
  const numericSor = $derived(Number(s_or));
  const numericNw = $derived(Number(n_w));
  const numericNo = $derived(Number(n_o));
  const numericKrwMax = $derived(Number(k_rw_max));
  const numericKroMax = $derived(Number(k_ro_max));
  const numericPEntry = $derived(Number(capillaryPEntry));
  const numericLambda = $derived(Number(capillaryLambda));

  const safeSwc = $derived(Number.isFinite(numericSwc) ? clamp(numericSwc, 0, 0.95) : 0.1);
  const safeSor = $derived(Number.isFinite(numericSor) ? clamp(numericSor, 0, 0.95) : 0.1);
  const safeNw = $derived(Number.isFinite(numericNw) ? Math.max(0.1, numericNw) : 2);
  const safeNo = $derived(Number.isFinite(numericNo) ? Math.max(0.1, numericNo) : 2);
  const safeKrwMax = $derived(Number.isFinite(numericKrwMax) ? clamp(numericKrwMax, 0.01, 1.0) : 1.0);
  const safeKroMax = $derived(Number.isFinite(numericKroMax) ? clamp(numericKroMax, 0.01, 1.0) : 1.0);
  const safePEntry = $derived(Number.isFinite(numericPEntry) ? Math.max(0, numericPEntry) : 0);
  const safeLambda = $derived(Number.isFinite(numericLambda) ? Math.max(0.1, numericLambda) : 2);

  function swEffWith(sw: number, swc: number, sor: number) {
    const denom = Math.max(1e-6, 1 - swc - sor);
    return clamp((sw - swc) / denom, 0, 1);
  }

  function krwWith(sw: number, swc: number, sor: number, nw: number, krw_max: number) {
    return krw_max * Math.pow(swEffWith(sw, swc, sor), nw);
  }

  function kroWith(sw: number, swc: number, sor: number, no: number, kro_max: number) {
    return kro_max * Math.pow(1 - swEffWith(sw, swc, sor), no);
  }

  function pcWith(sw: number, swc: number, sor: number, pEntry: number, lambda: number, enabled: boolean) {
    if (!enabled || pEntry <= 0) return 0;
    const se = swEffWith(sw, swc, sor);
    if (se >= 1) return 0;
    if (se <= 0) return 500;
    return Math.min(500, pEntry * Math.pow(se, -1 / lambda));
  }

  const maxPc = $derived(
    Math.max(1, ...Array.from({ length: 41 }, (_, i) =>
      pcWith(i / 40, safeSwc, safeSor, safePEntry, safeLambda, capillaryEnabled))),
  );

  const relPermPathW = $derived(toPath((sw) => krwWith(sw, safeSwc, safeSor, safeNw, safeKrwMax), 1));
  const relPermPathO = $derived(toPath((sw) => kroWith(sw, safeSwc, safeSor, safeNo, safeKroMax), 1));
  const capillaryPath = $derived(
    toPath((sw) => pcWith(sw, safeSwc, safeSor, safePEntry, safeLambda, capillaryEnabled), maxPc),
  );
  const hasError = $derived(fieldErrors.saturationEndpoints !== undefined);
</script>

<Collapsible title="Rel Perm & Capillary" {hasError}>
  <div class="space-y-2 px-2.5 py-2">
    <!-- Rel perm endpoints table -->
    <div class="overflow-x-auto rounded-md border border-border">
      <table class="compact-table w-full text-left">
        <thead class="border-b border-border bg-muted/50 text-[10px] uppercase tracking-wide text-muted-foreground">
          <tr>
            <th class="px-1 py-0.5 font-medium">Phase</th>
            <th class="px-1 py-0.5 font-medium">S_end</th>
            <th class="px-1 py-0.5 font-medium">n (Corey)</th>
            <th class="px-1 py-0.5 font-medium">kr_max</th>
          </tr>
        </thead>
        <tbody class="divide-y divide-border text-xs">
          <tr>
            <td class="px-1 py-0.5 font-medium text-muted-foreground">Water</td>
            <td class="px-0.5 py-0.5">
              <Input type="number" min="0" max="0.9" step="0.01"
                class={`w-full h-6 px-1 text-xs ${Boolean(fieldErrors.saturationEndpoints) ? "border-destructive" : ""}`}
                bind:value={s_wc} />
            </td>
            <td class="px-0.5 py-0.5"><Input type="number" min="0.1" step="0.1" class="w-full h-6 px-1 text-xs" bind:value={n_w} /></td>
            <td class="px-0.5 py-0.5"><Input type="number" min="0.01" max="1.0" step="0.01" class="w-full h-6 px-1 text-xs" bind:value={k_rw_max} /></td>
          </tr>
          <tr>
            <td class="px-1 py-0.5 font-medium text-muted-foreground">Oil</td>
            <td class="px-0.5 py-0.5">
              <Input type="number" min="0" max="0.9" step="0.01"
                class={`w-full h-6 px-1 text-xs ${Boolean(fieldErrors.saturationEndpoints) ? "border-destructive" : ""}`}
                bind:value={s_or} />
            </td>
            <td class="px-0.5 py-0.5"><Input type="number" min="0.1" step="0.1" class="w-full h-6 px-1 text-xs" bind:value={n_o} /></td>
            <td class="px-0.5 py-0.5"><Input type="number" min="0.01" max="1.0" step="0.01" class="w-full h-6 px-1 text-xs" bind:value={k_ro_max} /></td>
          </tr>
        </tbody>
      </table>
    </div>

    {#if fieldErrors.saturationEndpoints}
      <div class="text-[10px] text-destructive leading-tight">{fieldErrors.saturationEndpoints}</div>
    {/if}

    <!-- Capillary: toggle + inline params -->
    <div class="flex items-center gap-3">
      <label class="flex items-center gap-1.5 cursor-pointer">
        <input type="checkbox" class="h-3.5 w-3.5 rounded border-input accent-primary" bind:checked={capillaryEnabled} />
        <span class="text-xs font-medium">Capillary Pressure</span>
      </label>
      {#if capillaryEnabled}
        <label class="flex items-center gap-1 text-xs">
          <span class="text-[10px] text-muted-foreground">P_e (bar)</span>
          <Input type="number" min="0" step="0.1" class="w-16 h-6 px-1 text-xs" bind:value={capillaryPEntry} />
        </label>
        <label class="flex items-center gap-1 text-xs">
          <span class="text-[10px] text-muted-foreground">λ</span>
          <Input type="number" min="0.1" step="0.1" class="w-14 h-6 px-1 text-xs" bind:value={capillaryLambda} />
        </label>
      {/if}
    </div>

    <!-- Curves side-by-side -->
    <div class="grid grid-cols-2 gap-1.5">
      <div class="rounded border border-border/70 bg-muted/10 p-1.5">
        <div class="text-[9px] text-muted-foreground font-medium mb-0.5">kr(Sw)</div>
        <svg viewBox={`0 0 ${width} ${height}`} class="h-14 w-full">
          <path d={relPermPathW} stroke="#3b82f6" stroke-width="1.5" fill="none" />
          <path d={relPermPathO} stroke="#f97316" stroke-width="1.5" fill="none" />
        </svg>
      </div>
      <div class="rounded border border-border/70 bg-muted/10 p-1.5">
        <div class="text-[9px] text-muted-foreground font-medium mb-0.5">Pc(Sw)</div>
        <svg viewBox={`0 0 ${width} ${height}`} class="h-14 w-full">
          <path d={capillaryPath} stroke="#22c55e" stroke-width="1.5" fill="none" />
        </svg>
      </div>
    </div>
  </div>
</Collapsible>
