import type { CaseMode, ToggleState } from "./catalog/caseCatalog";
import type { ValidationWarning } from "./validateInputs";

export type WarningPolicyGroupKey =
  | "blockingValidation"
  | "nonPhysical"
  | "referenceCaveat"
  | "advisory";

export type WarningPolicyTone = "destructive" | "warning" | "info";

export type WarningPolicySource = "validation" | "runtime" | "analytical";

export type WarningPolicyGroupSources = Partial<
  Record<WarningPolicyGroupKey, WarningPolicySource[]>
>;

export type WarningPolicyItem = {
  id: string;
  code: string;
  message: string;
  source: WarningPolicySource;
  fieldKey?: string;
};

export type WarningPolicyGroup = {
  key: WarningPolicyGroupKey;
  title: string;
  description: string;
  tone: WarningPolicyTone;
  items: WarningPolicyItem[];
};

export type WarningPolicy = {
  blockingValidation: WarningPolicyGroup;
  nonPhysical: WarningPolicyGroup;
  referenceCaveat: WarningPolicyGroup;
  advisory: WarningPolicyGroup;
  hasVisibleItems: boolean;
  totalCount: number;
};

export type WarningPolicyInput = {
  validationErrors: Record<string, string>;
  validationWarnings: ValidationWarning[];
  analyticalStatus: AnalyticalStatus;
  runtimeWarning?: string;
  solverWarning?: string;
  modelReinitNotice?: string;
  longRunEstimate?: boolean;
  estimatedRunSeconds?: number;
};

const GROUP_META: Record<WarningPolicyGroupKey, Omit<WarningPolicyGroup, "items">> = {
  blockingValidation: {
    key: "blockingValidation",
    title: "Action Required",
    description: "Resolve these inputs before initializing or running.",
    tone: "destructive",
  },
  nonPhysical: {
    key: "nonPhysical",
    title: "Reliability Cautions",
    description: "These settings can undermine physical realism or solver stability.",
    tone: "warning",
  },
  referenceCaveat: {
    key: "referenceCaveat",
    title: "Reference Limits",
    description: "Reference guidance is still shown, but this case is no longer a strict match.",
    tone: "info",
  },
  advisory: {
    key: "advisory",
    title: "Run Notes",
    description: "Operational notices about resets, runtime changes, and long runs.",
    tone: "info",
  },
};

function createEmptyGroup(key: WarningPolicyGroupKey): WarningPolicyGroup {
  return {
    ...GROUP_META[key],
    items: [],
  };
}

function pushUniqueItem(group: WarningPolicyGroup, item: WarningPolicyItem) {
  const exists = group.items.some((entry) => entry.id === item.id);
  if (!exists) {
    group.items = [...group.items, item];
  }
}

function analyticalReasonToPolicyItem(
  reason: AnalyticalStatusReason,
): WarningPolicyItem {
  return {
    id: `analytical:${reason.code}`,
    code: reason.code,
    message: reason.message,
    source: "analytical",
  };
}

function cloneGroupWithItems(
  group: WarningPolicyGroup,
  items: WarningPolicyItem[],
): WarningPolicyGroup {
  return {
    ...group,
    items,
  };
}

export function getWarningPolicyGroup(
  policy: WarningPolicy,
  key: WarningPolicyGroupKey,
  sources?: WarningPolicySource[],
): WarningPolicyGroup {
  const group = policy[key];
  if (!sources?.length) {
    return cloneGroupWithItems(group, [...group.items]);
  }

  const allowedSources = new Set(sources);
  return cloneGroupWithItems(
    group,
    group.items.filter((item) => allowedSources.has(item.source)),
  );
}

export function getWarningPolicyGroups(
  policy: WarningPolicy,
  keys: WarningPolicyGroupKey[],
  groupSources: WarningPolicyGroupSources = {},
): WarningPolicyGroup[] {
  return keys
    .map((key) => getWarningPolicyGroup(policy, key, groupSources[key]))
    .filter((group) => group.items.length > 0);
}

// ---------- Analytical Status ----------

export type AnalyticalStatusLevel = 'reference' | 'approximate' | 'off';

export type AnalyticalStatusMode = 'waterflood' | 'depletion' | 'none';

export type AnalyticalReasonSeverity = 'notice' | 'warning' | 'critical';

export type AnalyticalStatusWarningSeverity = 'none' | AnalyticalReasonSeverity;

export type AnalyticalStatusReason = {
  code: string;
  message: string;
  severity: AnalyticalReasonSeverity;
};

export type AnalyticalStatus = {
  level: AnalyticalStatusLevel;
  mode: AnalyticalStatusMode;
  warningSeverity: AnalyticalStatusWarningSeverity;
  reasonDetails: AnalyticalStatusReason[];
  reasons: string[];
};

/**
 * The grid and well placement an analytical caveat is judged against.
 *
 * Supplied by the catalog path, where the real numbers are known. The builder
 * path has only its toggle state and passes none, which is why every field is
 * read through `resolveGeometryFacts` rather than directly.
 */
export type AnalyticalGeometry = {
  nx: number;
  ny: number;
  nz: number;
  injectorI: number;
  injectorJ: number;
  producerI: number;
  producerJ: number;
  /** Perforated layers; empty means every layer. */
  injectorKLayers?: number[];
  producerKLayers?: number[];
};

export type AnalyticalStatusInput = {
  activeMode: CaseMode;
  analyticalMode: AnalyticalStatusMode;
  injectorEnabled: boolean;
  gravityEnabled: boolean;
  capillaryEnabled: boolean;
  permMode: 'uniform' | 'random' | 'perLayer' | 'field';
  toggles: ToggleState;
  /** Actual geometry, when the caller knows it. Falls back to `toggles`. */
  geometry?: AnalyticalGeometry;
};

/**
 * Whether the flow path is one-dimensional, and whether the two wells sit at
 * its opposite ends — the two facts the Buckley-Leverett caveats turn on.
 *
 * These used to be read from `toggles.geo` / `toggles.well`, which the Scenario
 * Builder sets. The builder's dimension catalog is empty in the shipped app, so
 * `toggles.geo` was `undefined` for every predefined scenario and both caveats
 * fired unconditionally — `wf_bl1d`, a 96 x 1 x 1 slab with wells at cell 0 and
 * cell 95, was told its geometry was not 1D. Measured from the grid instead, the
 * warnings say something true: they stay silent on the 1D cases and fire on the
 * 2D section, which is exactly where displacement efficiency stops being the
 * whole answer.
 */
export function resolveGeometryFacts(
  toggles: ToggleState,
  geometry: AnalyticalGeometry | undefined,
): { isOneDimensional: boolean; isEndToEnd: boolean; isWellCentered: boolean } {
  if (!geometry) {
    return {
      isOneDimensional: toggles.geo === '1d',
      isEndToEnd: toggles.well === 'e2e',
      isWellCentered: toggles.well === 'center',
    };
  }

  const extents = [geometry.nx, geometry.ny, geometry.nz].map((extent) => Math.max(1, Math.round(extent)));
  const varying = extents.filter((extent) => extent > 1);
  const isOneDimensional = varying.length <= 1;

  const layer = (layers: number[] | undefined, fallback: number) => (
    Array.isArray(layers) && layers.length > 0 ? layers[0] : fallback
  );
  const flowAxis = extents.findIndex((extent) => extent > 1);
  const injector = [geometry.injectorI, geometry.injectorJ, layer(geometry.injectorKLayers, 0)];
  const producer = [geometry.producerI, geometry.producerJ, layer(geometry.producerKLayers, extents[2] - 1)];
  // A single-cell model has no ends to be at; treat it as end to end.
  const isEndToEnd = flowAxis < 0
    ? true
    : Math.min(injector[flowAxis], producer[flowAxis]) === 0
      && Math.max(injector[flowAxis], producer[flowAxis]) === extents[flowAxis] - 1;

  // Centred to within half a cell, so an even-sided grid has two valid centres.
  const centred = (index: number, extent: number) => extent <= 1
    || index === Math.floor((extent - 1) / 2)
    || index === Math.ceil((extent - 1) / 2);
  const isWellCentered = centred(geometry.producerI, extents[0]) && centred(geometry.producerJ, extents[1]);

  return { isOneDimensional, isEndToEnd, isWellCentered };
}

const ANALYTICAL_SEVERITY_RANK: Record<AnalyticalStatusWarningSeverity, number> = {
  none: 0,
  notice: 1,
  warning: 2,
  critical: 3,
};

function maxAnalyticalSeverity(
  reasons: readonly AnalyticalStatusReason[],
): AnalyticalStatusWarningSeverity {
  if (!reasons.length) return 'none';
  let max: AnalyticalStatusWarningSeverity = 'none';
  for (const reason of reasons) {
    const severity = reason.severity;
    if (ANALYTICAL_SEVERITY_RANK[severity] > ANALYTICAL_SEVERITY_RANK[max]) {
      max = severity;
    }
  }
  return max;
}

export function evaluateAnalyticalStatus(input: AnalyticalStatusInput): AnalyticalStatus {
  const {
    activeMode,
    analyticalMode,
    injectorEnabled,
    gravityEnabled,
    capillaryEnabled,
    permMode,
    toggles,
  } = input;
  const { isOneDimensional, isEndToEnd, isWellCentered } = resolveGeometryFacts(toggles, input.geometry);

  if (analyticalMode !== 'waterflood' && analyticalMode !== 'depletion') {
    const reasonDetails: AnalyticalStatusReason[] = [
      {
        code: 'analytical-disabled',
        message: 'Reference solution guidance is disabled for this scenario.',
        severity: 'notice',
      },
    ];
    return {
      level: 'off',
      mode: 'none',
      warningSeverity: 'none',
      reasonDetails,
      reasons: reasonDetails.map((r) => r.message),
    };
  }

  const reasonDetails: AnalyticalStatusReason[] = [];

  const addReason = (
    code: string,
    message: string,
    severity: AnalyticalReasonSeverity,
  ) => {
    reasonDetails.push({ code, message, severity });
  };

  if (analyticalMode === 'waterflood') {
    if (!injectorEnabled) {
      addReason(
        'wf-injector-disabled',
        'Injector is disabled — the waterflood analytical solution does not apply.',
        'critical',
      );
    }
    if (!isOneDimensional) {
      addReason(
        'wf-geometry-not-1d',
        'Geometry is not 1D — the waterflood analytical solution assumes linear flow.',
        'warning',
      );
    }
    if (!isEndToEnd) {
      addReason(
        'wf-well-not-e2e',
        'Wells are not end-to-end — the waterflood analytical solution assumes linear end-to-end flow.',
        'warning',
      );
    }
  } else {
    if (injectorEnabled) {
      addReason(
        'dep-injector-enabled',
        'Injector is enabled — the depletion analytical solution does not apply.',
        'critical',
      );
    }
    if (!(isOneDimensional || isWellCentered)) {
      addReason(
        'dep-geometry-well-mismatch',
        'Well position deviates — the depletion analytical solution assumes 1D or centered well.',
        'warning',
      );
    }
  }

  if (permMode === 'random' || permMode === 'field') {
    addReason(
      'perm-nonuniform',
      'Per-cell permeability heterogeneity — the analytical solution is approximate.',
      'warning',
    );
  }
  const hasMultipleLayers = input.geometry
    ? Math.max(1, Math.round(input.geometry.nz)) > 1
    : (toggles.geo === '2dxz' || toggles.geo === '3d');
  if (permMode === 'perLayer' && hasMultipleLayers && analyticalMode === 'depletion') {
    addReason(
      'perm-layered-depletion',
      'Layered permeability with multiple layers — the depletion analytical solution assumes a single homogeneous layer.',
      'warning',
    );
  }
  if (gravityEnabled) {
    addReason(
      'gravity-enabled',
      'Gravity is enabled — the analytical solution assumes gravity-free flow.',
      'warning',
    );
  }
  if (capillaryEnabled) {
    addReason(
      'capillary-enabled',
      'Capillary pressure is enabled — the analytical solution assumes capillary-free flow.',
      'warning',
    );
  }

  if (activeMode === 'sim') {
    addReason(
      'sim-mode-exploratory',
      'Scenario Builder is exploratory — the analytical solution is shown as guidance only.',
      'notice',
    );
  }

  const warningSeverity = maxAnalyticalSeverity(reasonDetails);

  return {
    level: reasonDetails.length === 0 ? 'reference' : 'approximate',
    mode: analyticalMode,
    warningSeverity,
    reasonDetails,
    reasons: reasonDetails.map((r) => r.message),
  };
}

// ---------- Warning Policy ----------

export function buildWarningPolicy(input: WarningPolicyInput): WarningPolicy {
  const blockingValidation = createEmptyGroup("blockingValidation");
  const nonPhysical = createEmptyGroup("nonPhysical");
  const referenceCaveat = createEmptyGroup("referenceCaveat");
  const advisory = createEmptyGroup("advisory");

  for (const [fieldKey, message] of Object.entries(input.validationErrors)) {
    pushUniqueItem(blockingValidation, {
      id: `validation-error:${fieldKey}`,
      code: fieldKey,
      message,
      source: "validation",
      fieldKey,
    });
  }

  for (const warning of input.validationWarnings) {
    const target = warning.surface === "non-physical" ? nonPhysical : advisory;
    pushUniqueItem(target, {
      id: `validation-warning:${warning.code}`,
      code: warning.code,
      message: warning.message,
      source: "validation",
      fieldKey: warning.fieldKey,
    });
  }

  if (input.analyticalStatus.level === "approximate") {
    for (const reason of input.analyticalStatus.reasonDetails) {
      pushUniqueItem(referenceCaveat, analyticalReasonToPolicyItem(reason));
    }
  }

  if (input.solverWarning) {
    pushUniqueItem(nonPhysical, {
      id: "runtime:solver-warning",
      code: "solver-warning",
      message: input.solverWarning,
      source: "runtime",
    });
  }

  if (input.runtimeWarning) {
    pushUniqueItem(advisory, {
      id: "runtime:runtime-warning",
      code: "runtime-warning",
      message: input.runtimeWarning,
      source: "runtime",
    });
  }

  if (input.modelReinitNotice) {
    pushUniqueItem(advisory, {
      id: "runtime:model-reinit",
      code: "model-reinit",
      message: input.modelReinitNotice,
      source: "runtime",
    });
  }

  if (input.longRunEstimate) {
    const seconds = Number(input.estimatedRunSeconds ?? 0);
    pushUniqueItem(advisory, {
      id: "runtime:long-run-estimate",
      code: "long-run-estimate",
      message:
        seconds > 0
          ? `Estimated run: ${seconds.toFixed(1)}s. You can stop at any time.`
          : "Estimated run is long enough that you may want to stop early if results are already clear.",
      source: "runtime",
    });
  }

  const groups = [
    blockingValidation,
    nonPhysical,
    referenceCaveat,
    advisory,
  ];

  const totalCount = groups.reduce((sum, group) => sum + group.items.length, 0);

  return {
    blockingValidation,
    nonPhysical,
    referenceCaveat,
    advisory,
    hasVisibleItems: totalCount > 0,
    totalCount,
  };
}
