import { describe, expect, it } from "vitest";
import { buildWarningPolicy, resolveGeometryFacts } from "./warningPolicy";
import type { AnalyticalStatus } from "./warningPolicy";

const referenceStatus: AnalyticalStatus = {
  level: "reference",
  mode: "waterflood",
  warningSeverity: "none",
  reasonDetails: [],
  reasons: [],
};

describe("resolveGeometryFacts", () => {
  const grid = (over: Partial<Parameters<typeof resolveGeometryFacts>[1]> = {}) => ({
    nx: 96, ny: 1, nz: 1, injectorI: 0, injectorJ: 0, producerI: 95, producerJ: 0, ...over,
  } as NonNullable<Parameters<typeof resolveGeometryFacts>[1]>);

  it("reads a horizontal slab as 1D and end to end", () => {
    expect(resolveGeometryFacts({}, grid())).toMatchObject({ isOneDimensional: true, isEndToEnd: true });
  });

  it("reads a vertical column as 1D, using the perforated layers as the ends", () => {
    // 1 x 1 x 60 with the injector at the base and the producer at the top:
    // the flow path is the k axis, and both wells share the only i/j there is.
    expect(resolveGeometryFacts({}, grid({
      nx: 1, ny: 1, nz: 60, producerI: 0,
      injectorKLayers: [59], producerKLayers: [0],
    }))).toMatchObject({ isOneDimensional: true, isEndToEnd: true });
  });

  it("reads a vertical section as not 1D", () => {
    expect(resolveGeometryFacts({}, grid({ nx: 30, nz: 20, producerI: 29 })))
      .toMatchObject({ isOneDimensional: false });
  });

  it("does not call a partial-length flood end to end", () => {
    expect(resolveGeometryFacts({}, grid({ producerI: 40 }))).toMatchObject({ isEndToEnd: false });
  });

  it("recognises a centred producer on odd and even grids", () => {
    expect(resolveGeometryFacts({}, grid({ nx: 21, ny: 21, producerI: 10, producerJ: 10 })).isWellCentered).toBe(true);
    expect(resolveGeometryFacts({}, grid({ nx: 20, ny: 20, producerI: 9, producerJ: 10 })).isWellCentered).toBe(true);
    expect(resolveGeometryFacts({}, grid({ nx: 21, ny: 21, producerI: 0, producerJ: 0 })).isWellCentered).toBe(false);
  });

  it("falls back to the builder toggles when no geometry is supplied", () => {
    expect(resolveGeometryFacts({ geo: "1d", well: "e2e" }, undefined))
      .toEqual({ isOneDimensional: true, isEndToEnd: true, isWellCentered: false });
    expect(resolveGeometryFacts({ geo: "3d", well: "center" }, undefined))
      .toEqual({ isOneDimensional: false, isEndToEnd: false, isWellCentered: true });
  });
});

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
          code: "small-timestep",
          message: "Very small timestep: each requested step covers little simulated time.",
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
      "small-timestep",
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
            message: "Scenario Builder is exploratory; the reference solution is treated as approximate guidance.",
            severity: "notice",
          },
          {
            code: "gravity-enabled",
            message: "Gravity is enabled, which deviates from the reference solution assumptions.",
            severity: "warning",
          },
        ],
        reasons: [
          "Scenario Builder is exploratory; the reference solution is treated as approximate guidance.",
          "Gravity is enabled, which deviates from the reference solution assumptions.",
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
