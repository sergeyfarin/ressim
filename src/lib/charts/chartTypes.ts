export type CurveConfig = {
    label: string;
    curveKey?: string;
    caseKey?: string;
    toggleLabel?: string;
    toggleGroupKey?: string;
    color: string;
    legendColor?: string;
    borderWidth?: number;
    borderDash?: number[];
    yAxisID: string;
    defaultVisible?: boolean;
    disabled?: boolean;
    legendSection?: string;
    legendSectionLabel?: string;
    /**
     * Identifies an additive reference curve that must remain visible even when
     * a scenario panel's curveKeys selects its live/analytical curves.
     */
    referenceSourceType?: 'simulation' | 'analytical' | 'published-reference' | 'opm-flow-precomputed';
    /** Override point radius for scatter-style markers (default 0 = no markers). */
    pointRadius?: number;
    /**
     * What this curve *is* — 'oil-rate', 'average-pressure', 'drive-index'.
     * Declared rather than parsed back out of `curveKey`; `appendSeries` stamps
     * it from the curve registry when a builder does not set it, so every built
     * curve carries one. A panel may only hold curves of a single property.
     */
    property?: string;
};
