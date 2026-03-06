import { describe, expect, it } from "vitest";
import {
  buildWarningPolicy,
  getWarningPolicyGroup,
  getWarningPolicyGroups,
} from "./warningPolicy";
import type { AnalyticalStatus } from "./stores/phase2PresetContract";

const referenceStatus: AnalyticalStatus = {
  level: "reference",
  mode: "waterflood",
  warningSeverity: "none",
  reasonDetails: [],
  reasons: [],
};

describe("warningPolicy filters", () => {
  it("filters each group by requested sources", () => {
    const policy = buildWarningPolicy({
      validationErrors: {
        steps: "Steps must be an integer >= 1.",
      },
      validationWarnings: [
        {
          code: "pressure-step-large",
          message: "Large max dP per step may reduce numerical robustness.",
          surface: "non-physical",
          fieldKey: "max_pressure_change_per_step",
        },
      ],
      analyticalStatus: referenceStatus,
      solverWarning: "Pressure solve stalled; check timestep limits.",
      runtimeWarning: "Config changed during run. Reservoir reinitialized at step 0.",
      modelReinitNotice: "Model reinit required due to input changes",
      longRunEstimate: true,
      estimatedRunSeconds: 18.2,
    });

    const validationOnly = getWarningPolicyGroups(policy, [
      "blockingValidation",
      "nonPhysical",
      "advisory",
    ], {
      blockingValidation: ["validation"],
      nonPhysical: ["validation"],
      advisory: ["validation"],
    });

    expect(validationOnly.map((group) => [group.key, group.items.map((item) => item.code)])).toEqual([
      ["blockingValidation", ["steps"]],
      ["nonPhysical", ["pressure-step-large"]],
    ]);

    const runtimeNonPhysical = getWarningPolicyGroup(policy, "nonPhysical", ["runtime"]);
    expect(runtimeNonPhysical.items.map((item) => item.code)).toEqual(["solver-warning"]);

    const runtimeAdvisory = getWarningPolicyGroup(policy, "advisory", ["runtime"]);
    expect(runtimeAdvisory.items.map((item) => item.code)).toEqual([
      "runtime-warning",
      "model-reinit",
      "long-run-estimate",
    ]);
  });
});
