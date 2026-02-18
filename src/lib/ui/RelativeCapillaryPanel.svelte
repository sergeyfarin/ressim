<script lang="ts">
  let {
    s_wc = $bindable(0.2),
    s_or = $bindable(0.1),
    n_w = $bindable(2),
    n_o = $bindable(2),
    capillaryEnabled = $bindable(true),
    capillaryPEntry = $bindable(5),
    capillaryLambda = $bindable(2),
  }: {
    s_wc?: number;
    s_or?: number;
    n_w?: number;
    n_o?: number;
    capillaryEnabled?: boolean;
    capillaryPEntry?: number;
    capillaryLambda?: number;
  } = $props();

  const width = 180;
  const height = 80;
  const pad = 8;

  function clamp(v: number, lo: number, hi: number) {
    return Math.max(lo, Math.min(hi, v));
  }

  function toPath(fn: (sw: number) => number, yMax = 1) {
    const pts: string[] = [];
    for (let i = 0; i <= 40; i++) {
      const sw = i / 40;
      const x = pad + sw * (width - pad * 2);
      const y = height - pad - (fn(sw) / yMax) * (height - pad * 2);
      pts.push(`${i === 0 ? 'M' : 'L'} ${x.toFixed(2)} ${y.toFixed(2)}`);
    }
    return pts.join(' ');
  }

  const numericSwc = $derived(Number(s_wc));
  const numericSor = $derived(Number(s_or));
  const numericNw = $derived(Number(n_w));
  const numericNo = $derived(Number(n_o));
  const numericPEntry = $derived(Number(capillaryPEntry));
  const numericLambda = $derived(Number(capillaryLambda));

  const safeSwc = $derived(Number.isFinite(numericSwc) ? clamp(numericSwc, 0, 0.95) : 0.1);
  const safeSor = $derived(Number.isFinite(numericSor) ? clamp(numericSor, 0, 0.95) : 0.1);
  const safeNw = $derived(Number.isFinite(numericNw) ? Math.max(0.1, numericNw) : 2);
  const safeNo = $derived(Number.isFinite(numericNo) ? Math.max(0.1, numericNo) : 2);
  const safePEntry = $derived(Number.isFinite(numericPEntry) ? Math.max(0, numericPEntry) : 0);
  const safeLambda = $derived(Number.isFinite(numericLambda) ? Math.max(0.1, numericLambda) : 2);

  function swEffWith(sw: number, swc: number, sor: number) {
    const denom = Math.max(1e-6, 1 - swc - sor);
    return clamp((sw - swc) / denom, 0, 1);
  }

  function krwWith(sw: number, swc: number, sor: number, nw: number) {
    return Math.pow(swEffWith(sw, swc, sor), nw);
  }

  function kroWith(sw: number, swc: number, sor: number, no: number) {
    return Math.pow(1 - swEffWith(sw, swc, sor), no);
  }

  function pcWith(sw: number, swc: number, sor: number, pEntry: number, lambda: number, enabled: boolean) {
    if (!enabled || pEntry <= 0) return 0;
    const se = swEffWith(sw, swc, sor);
    if (se >= 1) return 0;
    if (se <= 0) return 500;
    return Math.min(500, pEntry * Math.pow(se, -1 / lambda));
  }

  const maxPc = $derived(Math.max(
    1,
    ...Array.from({ length: 41 }, (_, i) =>
      pcWith(i / 40, safeSwc, safeSor, safePEntry, safeLambda, capillaryEnabled)
    )
  ));

  const relPermPathW = $derived(toPath((sw) => krwWith(sw, safeSwc, safeSor, safeNw), 1));
  const relPermPathO = $derived(toPath((sw) => kroWith(sw, safeSwc, safeSor, safeNo), 1));
  const capillaryPath = $derived(toPath(
    (sw) => pcWith(sw, safeSwc, safeSor, safePEntry, safeLambda, capillaryEnabled),
    maxPc
  ));
  const relPermSummary = $derived(`S_wc=${s_wc.toFixed(2)}, S_or=${s_or.toFixed(2)}, n_w=${n_w.toFixed(1)}, n_o=${n_o.toFixed(1)}`);
  const capSummary = $derived(capillaryEnabled
    ? `Pc on (P_entry=${capillaryPEntry.toFixed(1)} bar, λ=${capillaryLambda.toFixed(1)})`
    : 'Pc off');
  const groupSummary = $derived(`${relPermSummary} · ${capSummary}`);
</script>

<details class="rounded-lg border border-base-300 bg-base-100 shadow-sm">
  <summary class="flex cursor-pointer list-none items-center justify-between px-4 py-3 md:px-5">
    <div>
      <div class="font-semibold">Relative Permeability + Capillary</div>
      <div class="text-xs opacity-70">{groupSummary}</div>
    </div>
    <div class="flex items-center gap-2 text-xs opacity-70">
      <span class="collapse-label-open hidden">Collapse</span>
      <span class="collapse-label-closed">Expand</span>
      <span class="collapse-chevron">▸</span>
    </div>
  </summary>

  <div class="space-y-3 border-t border-base-300 p-4 md:p-5">
    <div class="grid grid-cols-2 gap-2">
      <label class="form-control">
        <span class="label-text text-xs">S_wc</span>
        <input type="number" min="0" max="0.9" step="0.01" class="input input-bordered input-sm w-full" bind:value={s_wc} />
      </label>
      <label class="form-control">
        <span class="label-text text-xs">S_or</span>
        <input type="number" min="0" max="0.9" step="0.01" class="input input-bordered input-sm w-full" bind:value={s_or} />
      </label>
      <label class="form-control">
        <span class="label-text text-xs">n_w</span>
        <input type="number" min="0.1" step="0.1" class="input input-bordered input-sm w-full" bind:value={n_w} />
      </label>
      <label class="form-control">
        <span class="label-text text-xs">n_o</span>
        <input type="number" min="0.1" step="0.1" class="input input-bordered input-sm w-full" bind:value={n_o} />
      </label>
    </div>

    <label class="label cursor-pointer justify-start gap-2">
      <input type="checkbox" class="checkbox checkbox-sm" bind:checked={capillaryEnabled} />
      <span class="label-text text-sm">Enable Capillary Pressure</span>
    </label>

    <div class="grid grid-cols-2 gap-2">
      <label class="form-control">
        <span class="label-text text-xs">P_entry (bar)</span>
        <input type="number" min="0" step="0.1" class="input input-bordered input-sm w-full" bind:value={capillaryPEntry} disabled={!capillaryEnabled} />
      </label>
      <label class="form-control">
        <span class="label-text text-xs">Lambda</span>
        <input type="number" min="0.1" step="0.1" class="input input-bordered input-sm w-full" bind:value={capillaryLambda} disabled={!capillaryEnabled} />
      </label>
    </div>

    <div class="grid grid-cols-1 gap-2">
      <div class="rounded-md border border-base-300 p-2">
        <div class="mb-1 text-[11px] opacity-70">Relative Permeability Curves</div>
        <svg viewBox={`0 0 ${width} ${height}`} class="h-20 w-full">
          <path d={relPermPathW} stroke="#3b82f6" stroke-width="2" fill="none" />
          <path d={relPermPathO} stroke="#f97316" stroke-width="2" fill="none" />
        </svg>
      </div>

      <div class="rounded-md border border-base-300 p-2">
        <div class="mb-1 text-[11px] opacity-70">Capillary Pressure Curve</div>
        <svg viewBox={`0 0 ${width} ${height}`} class="h-20 w-full">
          <path d={capillaryPath} stroke="#22c55e" stroke-width="2" fill="none" />
        </svg>
      </div>
    </div>
  </div>
</details>

<style>
  details[open] .collapse-chevron { transform: rotate(90deg); }
  .collapse-chevron { transition: transform 0.15s ease; display: inline-block; }
  details[open] .collapse-label-open { display: inline; }
  details[open] .collapse-label-closed { display: none; }
</style>
