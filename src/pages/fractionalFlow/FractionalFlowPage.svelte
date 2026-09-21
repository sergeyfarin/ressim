<!--
    A second page, built entirely from workspace packages.

    Its purpose is to make the claim "different frontends are possible" checkable rather than
    asserted (S3). It composes three packages — `@ressim/analytical` for the physics,
    `@ressim/charts` for rendering, `@ressim/primitives` for the control — and imports **nothing**
    from the application: no store, no scenario catalog, no worker, no WASM, no `App.svelte`.
    That absence is the point, so keep it. If this page ever needs something from `src/lib`
    outside a package, that is a finding about the package boundaries, not a reason to import it.
-->
<script lang="ts">
    import {
        fractionalFlow,
        computeWelgeMetrics,
        type RockProps,
        type FluidProps,
    } from '@ressim/analytical/fractionalFlow';
    import ChartSubPanel from '@ressim/charts/ChartSubPanel.svelte';
    import ToggleGroup from '@ressim/primitives/ToggleGroup.svelte';

    const ROCK: RockProps = {
        s_wc: 0.2,
        s_or: 0.2,
        n_w: 2,
        n_o: 2,
        k_rw_max: 0.3,
        k_ro_max: 0.9,
    };

    // The one thing the page lets you change. Mobility ratio is what makes a displacement
    // favourable or not, so it is the variable worth putting a control on.
    const RATIOS = [
        { value: 0.2, label: 'μo/μw = 0.2' },
        { value: 1, label: '1' },
        { value: 5, label: '5' },
        { value: 20, label: '20' },
    ];
    let viscosityRatio = $state(5);

    const fluid = $derived<FluidProps>({ mu_w: 1, mu_o: viscosityRatio });

    const SAMPLES = 201;
    const curve = $derived.by(() => {
        const points: { x: number; y: number | null }[] = [];
        for (let i = 0; i < SAMPLES; i += 1) {
            const s_w = i / (SAMPLES - 1);
            points.push({ x: s_w, y: fractionalFlow(s_w, ROCK, fluid) });
        }
        return points;
    });

    const welge = $derived(computeWelgeMetrics(ROCK, fluid, ROCK.s_wc));

    // The Welge tangent, drawn from the initial saturation to the shock front. Two points is a
    // line; the chart package does not care that one curve is 201 points and the other is 2.
    const tangent = $derived.by(() => [
        { x: welge.initialSw, y: 0 },
        { x: welge.shockSw, y: fractionalFlow(welge.shockSw, ROCK, fluid) },
    ]);

    const curves = $derived([
        { label: 'Fractional flow fw(Sw)', color: '#38bdf8', yAxisID: 'y', curveKey: 'fw' },
        {
            label: 'Welge tangent',
            color: '#f59e0b',
            yAxisID: 'y',
            curveKey: 'tangent',
            borderDash: [6, 4],
        },
    ]);

    const scaleConfigs = {
        x: { type: 'linear', min: 0, max: 1, title: { display: true, text: 'Water saturation Sw' } },
        y: { type: 'linear', min: 0, max: 1, title: { display: true, text: 'Fractional flow fw' } },
    };
</script>

<main class="mx-auto max-w-3xl p-6 space-y-5" data-testid="fractional-flow-page">
    <header class="space-y-1">
        <h1 class="text-xl font-semibold text-foreground">Fractional flow</h1>
        <p class="text-sm text-muted-foreground">
            Buckley–Leverett fractional flow and the Welge tangent construction, computed and
            drawn entirely from the <code>@ressim/analytical</code>, <code>@ressim/charts</code> and
            <code>@ressim/primitives</code> packages. No simulator runs on this page.
        </p>
    </header>

    <div class="flex items-center gap-3">
        <span class="text-xs text-muted-foreground">Viscosity ratio</span>
        <ToggleGroup
            options={RATIOS}
            bind:value={viscosityRatio}
            onChange={(v) => (viscosityRatio = Number(v))}
        />
    </div>

    <ChartSubPanel
        panelId="fractional-flow"
        title="fw(Sw) with Welge tangent"
        {curves}
        seriesData={[curve, tangent]}
        {scaleConfigs}
        theme="dark"
    />

    <dl class="grid grid-cols-3 gap-4 text-sm">
        <div>
            <dt class="text-muted-foreground text-xs">Shock front Sw</dt>
            <dd class="font-mono" data-testid="shock-sw">{welge.shockSw.toFixed(4)}</dd>
        </div>
        <div>
            <dt class="text-muted-foreground text-xs">Breakthrough PVI</dt>
            <dd class="font-mono" data-testid="breakthrough-pvi">{welge.breakthroughPvi.toFixed(4)}</dd>
        </div>
        <div>
            <dt class="text-muted-foreground text-xs">Water cut at breakthrough</dt>
            <dd class="font-mono">{welge.waterCutAtBreakthrough.toFixed(4)}</dd>
        </div>
    </dl>
</main>
