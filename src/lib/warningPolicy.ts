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

export type AnalyticalStatusInput = {
  activeMode: CaseMode;
  analyticalMode: AnalyticalStatusMode;
  injectorEnabled: boolean;
  gravityEnabled: boolean;
  capillaryEnabled: boolean;
  permMode: 'uniform' | 'random' | 'perLayer';
  toggles: ToggleState;
};

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
        'Injector is disabled, so the waterflood reference solution assumptions do not hold.',
        'critical',
      );
    }
    if (toggles.geo !== '1d') {
      addReason(
        'wf-geometry-not-1d',
        'The waterflood reference solution expects 1D geometry.',
        'warning',
      );
    }
    if (toggles.well !== 'e2e') {
      addReason(
        'wf-well-not-e2e',
        'The waterflood reference solution expects end-to-end wells.',
        'warning',
      );
    }
  } else {
    if (injectorEnabled) {
      addReason(
        'dep-injector-enabled',
        'Injector is enabled, so the depletion reference solution assumptions do not hold.',
        'critical',
      );
    }
    if (!(toggles.geo === '1d' || toggles.well === 'center')) {
      addReason(
        'dep-geometry-well-mismatch',
        'The depletion reference solution expects 1D or center-producer assumptions.',
        'warning',
      );
    }
  }

  if (permMode !== 'uniform') {
    addReason(
      'perm-nonuniform',
      'Permeability is non-uniform, so the reference solution becomes approximate.',
      'warning',
    );
  }
  if (gravityEnabled) {
    addReason(
      'gravity-enabled',
      'Gravity is enabled, which deviates from the reference solution assumptions.',
      'warning',
    );
  }
  if (capillaryEnabled) {
    addReason(
      'capillary-enabled',
      'Capillary pressure is enabled, which deviates from the reference solution assumptions.',
      'warning',
    );
  }

  if (activeMode === 'sim') {
    addReason(
      'sim-mode-exploratory',
      'Scenario Builder is exploratory; the reference solution is treated as approximate guidance.',
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
