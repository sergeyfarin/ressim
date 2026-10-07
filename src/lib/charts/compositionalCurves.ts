/**
 * compositionalCurves.ts — which panels a compositional run fills, as data.
 *
 * The compositional counterpart of `simulationCurves.ts`. A compositional run has none of the
 * black-oil quantities (no oil rate, no GOR, no p/z), so it is not sourced from `DerivedRunSeries`
 * at all: its curves come from `compositional/runQuantities.ts` over the run's own series, and
 * land in panels of their own. No black-oil curve key is ever emitted for such a run, which is the
 * C13 rule — compositional output must not be fed into black-oil panels or overlays.
 *
 * **Per-component quantities get a panel per component.** The component set belongs to the fluid,
 * so it is not known to a layout. Putting every component in one panel would need a second visual
 * channel besides the case colour, and the only one left is the dash pattern, which the curve
 * style policy reserves for analytical and reference curves. Instead a property maps to a *panel
 * group*: the layout places and configures the group once, and the model expands it into one
 * panel per component, every curve solid and in its case's colour.
 */

import type { CompositionalRunRecord } from '../compositional/runRecord';
import { compositionalRunQuantities } from '../compositional/runQuantities';
import type { ChartXAxisMode } from './chartLayoutConfig';

/** One property's placement: a fixed panel, or a group expanded per component. */
export type CompositionalPanelPlacement =
    | { kind: 'panel'; panel: string }
    | { kind: 'group'; group: string };

/** Where each compositional property is plotted. Keyed by `CompositionalRunQuantity.property`. */
export const COMPOSITIONAL_PANEL_PLACEMENT: Readonly<Record<string, CompositionalPanelPlacement>> = {
    pressure: { kind: 'panel', panel: 'comp_pressure' },
    saturation: { kind: 'panel', panel: 'comp_vapour_saturation' },
    'phase-fraction': { kind: 'panel', panel: 'comp_two_phase' },
    'component-inventory': { kind: 'group', group: 'comp_inventory' },
    'component-well-moles': { kind: 'group', group: 'comp_net_injected' },
    diagnostic: { kind: 'panel', panel: 'comp_conservation' },
};

/** The x axes a compositional run can be drawn against: its report times. */
export const COMPOSITIONAL_X_AXIS_MODES: readonly ChartXAxisMode[] = ['time', 'logTime'];

/** A group member's panel id. Stable for a component, so expansion state survives re-renders. */
export function panelGroupMemberId(group: string, member: string): string {
    return `${group}_${member}`;
}

/** A member panel of an expanded group. */
export type PanelGroupMember = { id: string; title: string };

export type CompositionalCurve = {
    panel: string;
    /** Set when the panel is a member of a group; the group's layout applies to it. */
    group: (PanelGroupMember & { group: string }) | null;
    curveKey: string;
    label: string;
    property: string;
    xValues: Array<number | null>;
    values: Array<number | null>;
};

/**
 * The curves one compositional run contributes, in registry order.
 *
 * Empty on an axis the run has no values for: a compositional run has no injected pore volumes or
 * produced liquid, and a curve placed on an x it does not have is worse than none.
 */
export function compositionalCurvesForRun(
    record: CompositionalRunRecord,
    xAxisMode: ChartXAxisMode,
): CompositionalCurve[] {
    if (!COMPOSITIONAL_X_AXIS_MODES.includes(xAxisMode)) return [];
    const { series } = record;
    const xValues = series.timeDays.map((t) => (Number.isFinite(t) ? t : null));

    const curves: CompositionalCurve[] = [];
    for (const quantity of compositionalRunQuantities(series)) {
        const placement = COMPOSITIONAL_PANEL_PLACEMENT[quantity.property];
        if (!placement) continue;
        let panel: string;
        let group: CompositionalCurve['group'] = null;
        if (placement.kind === 'group') {
            if (quantity.component === undefined) continue;
            panel = panelGroupMemberId(placement.group, quantity.component);
            group = {
                group: placement.group,
                id: panel,
                title: quantity.unit ? `${quantity.label} (${quantity.unit})` : quantity.label,
            };
        } else {
            panel = placement.panel;
        }
        curves.push({
            panel,
            group,
            curveKey: `${quantity.id}-sim`,
            label: quantity.label,
            property: quantity.property,
            xValues,
            values: quantity.source(series),
        });
    }
    return curves;
}
