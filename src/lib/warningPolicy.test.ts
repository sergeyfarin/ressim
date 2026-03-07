import { describe, expect, it } from "vitest";
import { buildWarningPolicy } from "./warningPolicy";
import type { AnalyticalStatus } from "./stores/phase2PresetContract";

const referenceStatus: AnalyticalStatus = {
  level: "reference",
  mode: "waterflood",
  warningSeverity: "none",
  reasonDetails: [],
  reasons: [],
};

describe("warningPolicy", () => {
  it("groups action-required, reliability, and run-note items separately", () => {
    const policy = buildWarningPolicy({
      validationErrors: {
        nx: "Nx must be an integer >= 1.",
      },
      validationWarnings: [
        {
          code: "pressure-step-large",
          message: "Large max dP per step may reduce numerical robustness.",
          surface: "non-physical",
          fieldKey: "max_pressure_change_per_step",
        },
        {
          code: "long-run-duration",
          message: "Requested run covers more than 10 years.",
          surface: "advisory",
          fieldKey: "steps",
        },
      ],
      analyticalStatus: referenceStatus,
      runtimeWarning: "Inputs changed during the run. Model reset to step 0.",
      solverWarning: "Pressure solve stalled; check timestep limits.",
      modelReinitNotice: "Model reset required after input changes.",
    });

    expect(policy.blockingValidation.title).toBe("Action Required");
    expect(policy.nonPhysical.title).toBe("Reliability Cautions");
    expect(policy.referenceCaveat.title).toBe("Reference Limits");
    expect(policy.advisory.title).toBe("Run Notes");
    expect(policy.blockingValidation.items).toHaveLength(1);
    expect(policy.nonPhysical.items.map((item) => item.code)).toEqual([
      "pressure-step-large",
      "solver-warning",
    ]);
    expect(policy.advisory.items.map((item) => item.code)).toEqual([
      "long-run-duration",
      "runtime-warning",
      "model-reinit",
    ]);
  });

  it("surfaces analytical approximation reasons as reference-limit items", () => {
    const policy = buildWarningPolicy({
      validationErrors: {},
      validationWarnings: [],
      analyticalStatus: {
        level: "approximate",
        mode: "waterflood",
        warningSeverity: "warning",
        reasonDetails: [
          {
            code: "sim-mode-exploratory",
            message: "Simulation mode is exploratory; analytical overlay is approximate guidance.",
            severity: "notice",
          },
          {
            code: "gravity-enabled",
            message: "Gravity is enabled, so analytical match is approximate.",
            severity: "warning",
          },
        ],
        reasons: [
          "Simulation mode is exploratory; analytical overlay is approximate guidance.",
          "Gravity is enabled, so analytical match is approximate.",
        ],
      },
    });

    expect(policy.referenceCaveat.items.map((item) => item.code)).toEqual([
      "sim-mode-exploratory",
      "gravity-enabled",
    ]);
    expect(policy.hasVisibleItems).toBe(true);
    expect(policy.totalCount).toBe(2);
  });
});
