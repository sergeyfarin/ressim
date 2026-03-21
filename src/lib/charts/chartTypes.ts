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
};