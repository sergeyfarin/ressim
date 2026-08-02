/**
 * sweepMethods.ts — the catalog of selectable sweep correlations.
 *
 * `sweepEfficiency.ts` owns the math; this owns what each method is *called*,
 * what it claims, and what it cites. Kept separate so adding a correlation does
 * not mean editing a 1,000-line numerics module, and so scenario files can name
 * a method without restating its prose.
 *
 * Before this, the Stiles / Dykstra-Parsons choice existed as a hand-written
 * `analyticalOptions` array inside `sweep_combined.ts` — three lines of label,
 * summary and citation per method, in one scenario, unavailable to the others.
 */

import type { SweepAnalyticalMethod, SweepGeometry } from './sweepEfficiency';

export type SweepMethodDescriptor = {
    method: SweepAnalyticalMethod;
    /** Stable selection key used in UI state and tests. */
    key: string;
    /** Short toggle-button label. */
    label: string;
    /**
     * What this method does to the *total* recovery / combined E_vol, at a given
     * geometry. Geometry-dependent because the inactive component is held at 1:
     * telling a vertical-only scenario its answer comes from "Craig areal × …"
     * would be wrong, since there is no areal sweep to apply.
     */
    summary: (geometry: SweepGeometry) => string;
    reference: string;
};

/**
 * Correlation used when a scenario declares none. Was a bare 'dykstra-parsons'
 * literal repeated as a default argument in `sweepEfficiency.ts` and as a `??`
 * fallback in the navigation store and the comparison builder.
 */
export const DEFAULT_SWEEP_METHOD: SweepAnalyticalMethod = 'dykstra-parsons';

export const SWEEP_METHODS: Record<SweepAnalyticalMethod, SweepMethodDescriptor> = {
    'stiles': {
        method: 'stiles',
        key: 'stiles',
        label: 'Stiles',
        summary: (geometry) => (geometry === 'areal'
            // With one layer there is nothing for Stiles to order, so it reduces
            // to the same Craig-plus-BL calculation as Dykstra-Parsons.
            ? 'Stiles layer-by-layer sweep, reduced to the Craig five-spot areal correlation × '
                + 'Buckley-Leverett displacement: with no layers to order it is identical to '
                + 'Dykstra-Parsons at this geometry.'
            : 'Stiles layer-by-layer sweep: total recovery and combined E_vol use layer-by-layer '
                + 'Buckley-Leverett displacement inside the contacted region, ordering layers by kh.'),
        reference: 'Stiles (1949); Craig (1971); Buckley and Leverett (1942); Welge (1952).',
    },
    'dykstra-parsons': {
        method: 'dykstra-parsons',
        key: 'dykstra',
        label: 'Dykstra-Parsons',
        summary: (geometry) => {
            const factors = geometry === 'areal'
                ? 'the Craig five-spot areal correlation (E_V = 1 at this geometry)'
                : geometry === 'vertical'
                    ? 'the Dykstra-Parsons non-communicating-layer model (E_A = 1 at this geometry)'
                    : 'Craig areal × Dykstra-Parsons vertical';
            return `Factorized sweep model: total recovery comes from ${factors} × Buckley-Leverett `
                + 'displacement through the local-PVI approximation.';
        },
        reference: 'Craig (1971); Dykstra and Parsons (1950); Buckley and Leverett (1942); Welge (1952).',
    },
};

/**
 * The decomposition caveat, per geometry.
 *
 * Deliberately stated on every sweep method (ROADMAP 2.2): improving the *total*
 * recovery comparison does not promote the per-component panels to predictions.
 * E_A and E_V stay teaching diagnostics — they decompose the model's own answer,
 * they are not independently validated against the simulation.
 */
function decompositionCaveat(geometry: SweepGeometry): string {
    if (geometry === 'areal') return 'E_A remains an analytical diagnostic decomposition view.';
    if (geometry === 'vertical') return 'E_V remains an analytical diagnostic decomposition view.';
    return 'E_A and E_V remain analytical diagnostic decomposition views.';
}

/**
 * Compose the user-facing description of a sweep method as applied at a given
 * geometry — the method's own claim plus the decomposition caveat for the
 * components that scenario actually shows.
 */
export function describeSweepMethod(
    method: SweepAnalyticalMethod,
    geometry: SweepGeometry,
): { key: string; label: string; summary: string; reference: string } {
    const descriptor = SWEEP_METHODS[method];
    return {
        key: descriptor.key,
        label: descriptor.label,
        summary: `${descriptor.summary(geometry)} ${decompositionCaveat(geometry)}`,
        reference: descriptor.reference,
    };
}
