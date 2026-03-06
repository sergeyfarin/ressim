import type {
  AnalyticalStatus,
  AnalyticalStatusReason,
} from "./stores/phase2PresetContract";
import type { ValidationWarning } from "./validateInputs";

export type WarningPolicyGroupKey =
  | "blockingValidation"
  | "nonPhysical"
  | "referenceCaveat"
  | "advisory";

export type WarningPolicyTone = "destructive" | "warning" | "info";

export type WarningPolicySource = "validation" | "runtime" | "analytical";

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
};

const GROUP_META: Record<WarningPolicyGroupKey, Omit<WarningPolicyGroup, "items">> = {
  blockingValidation: {
    key: "blockingValidation",
    title: "Blocking Validation",
    description: "These inputs must be fixed before init or run can proceed.",
    tone: "destructive",
  },
  nonPhysical: {
    key: "nonPhysical",
    title: "Non-Physical / Contradictory",
    description: "Editable states that can undermine physical or numerical reliability.",
    tone: "warning",
  },
  referenceCaveat: {
    key: "referenceCaveat",
    title: "Reference-Model Caveat",
    description: "Analytical overlay remains permissive but is no longer a strict reference match.",
    tone: "info",
  },
  advisory: {
    key: "advisory",
    title: "Advisory",
    description: "Operational notices and softer guidance that should stay visible.",
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
