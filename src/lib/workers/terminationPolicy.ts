import type { ScenarioTerminationCondition, ScenarioTerminationPolicy } from '../catalog/scenarios';
import type { RateHistoryPoint, SimulatorCreatePayload } from '../simulator-types';
import { producedSurfaceRates } from '../compositional/runSeries';
import type { CompositionalSnapshot } from '../compositional/types';

export type TerminationMatch = {
  summary: string;
  condition: ScenarioTerminationCondition;
  actualValue: number;
};

/**
 * The rates a termination condition is evaluated against.
 *
 * Black-oil and compositional runs report production in different vocabularies, so the evaluator
 * takes these neutral totals instead of either wire type. `liquidRate` is the denominator the
 * model itself uses for water cut: for black oil it is the reported total liquid (oil + water);
 * for a compositional run it is separator liquid + separator water.
 */
export interface TerminationRates {
  oilRate: number;
  waterRate: number;
  gasRate: number;
  liquidRate: number;
  /**
   * Producing gas/liquid ratio [m³/m³]. `null` means the ratio is not defined because nothing was
   * produced, which is **not** the same as a zero ratio and must not match an `lte` condition.
   */
  gor: number | null;
  /** Per-phase injection totals, when the run reports them. Missing phases are not evaluated. */
  injection?: {
    oil?: number;
    water?: number;
    gas?: number;
  };
}

function toFiniteNumber(value: unknown, fallback = 0): number {
  const numeric = Number(value);
  return Number.isFinite(numeric) ? numeric : fallback;
}

function formatValue(value: number): string {
  if (!Number.isFinite(value)) return 'NaN';
  if (Math.abs(value) >= 1000 || Math.abs(value) < 0.01) {
    return value.toExponential(3);
  }
  return value.toFixed(3);
}

function producerOilRate(point: RateHistoryPoint): number {
  return Math.max(0, Math.abs(toFiniteNumber(point.total_production_oil, 0)));
}

function producerLiquidRate(point: RateHistoryPoint): number {
  return Math.max(0, Math.abs(toFiniteNumber(point.total_production_liquid, 0)));
}

function producerWaterRate(point: RateHistoryPoint): number {
  return Math.max(0, producerLiquidRate(point) - producerOilRate(point));
}

function producerGasRate(point: RateHistoryPoint): number {
  return Math.max(0, Math.abs(toFiniteNumber(point.total_production_gas, 0)));
}

function producerWatercut(rates: TerminationRates): number {
  if (rates.liquidRate <= 1e-12) return 0;
  return Math.min(1, Math.max(0, rates.waterRate / rates.liquidRate));
}

function injectorPhaseRate(
  payload: SimulatorCreatePayload,
  point: RateHistoryPoint,
  phase: 'oil' | 'water' | 'gas',
): number | null {
  const totalInjection = Math.max(0, Math.abs(toFiniteNumber(point.total_injection, 0)));
  if (totalInjection <= 0) return 0;

  const injectedFluid = payload.threePhaseModeEnabled
    ? (payload.injectedFluid ?? 'gas')
    : 'water';

  if (phase === 'oil') return null;
  if (phase === 'water' && injectedFluid === 'water') return totalInjection;
  if (phase === 'gas' && injectedFluid === 'gas') return totalInjection;
  return 0;
}

function compareValue(actualValue: number, relation: 'lte' | 'gte', thresholdValue: number): boolean {
  return relation === 'lte'
    ? actualValue <= thresholdValue
    : actualValue >= thresholdValue;
}

function evaluateCondition(
  condition: ScenarioTerminationCondition,
  rates: TerminationRates,
): { matched: boolean; actualValue: number; summary: string } {
  if (condition.kind === 'watercut-threshold') {
    const actualValue = producerWatercut(rates);
    return {
      matched: actualValue >= condition.value,
      actualValue,
      summary: `Watercut threshold reached (${formatValue(actualValue)} >= ${formatValue(condition.value)}).`,
    };
  }

  if (condition.kind === 'gor-threshold') {
    if (rates.gor === null) {
      return {
        matched: false,
        actualValue: Number.NaN,
        summary: 'GOR threshold not evaluated because the run produced no liquid.',
      };
    }
    const actualValue = Math.max(0, rates.gor);
    return {
      matched: compareValue(actualValue, condition.relation, condition.value),
      actualValue,
      summary: `GOR threshold reached (${formatValue(actualValue)} ${condition.relation === 'lte' ? '<=' : '>='} ${formatValue(condition.value)}).`,
    };
  }

  const producerValue = condition.phase === 'oil'
    ? rates.oilRate
    : condition.phase === 'water'
      ? rates.waterRate
      : rates.gasRate;
  const injectorValue = rates.injection?.[condition.phase] ?? null;

  const candidateValues = (
    condition.scope === 'injector'
      ? [injectorValue]
      : condition.scope === 'any'
        ? [producerValue, injectorValue]
        : [producerValue]
  ).filter((value): value is number => value != null);

  const actualValue = candidateValues.length > 0
    ? (condition.relation === 'lte' ? Math.min(...candidateValues) : Math.max(...candidateValues))
    : Number.NaN;

  return {
    matched: Number.isFinite(actualValue) && compareValue(actualValue, condition.relation, condition.value),
    actualValue,
    summary: `${condition.phase} rate threshold reached (${formatValue(actualValue)} ${condition.relation === 'lte' ? '<=' : '>='} ${formatValue(condition.value)}).`,
  };
}

/**
 * Evaluate a policy against neutral rate totals.
 *
 * Shared by the black-oil and compositional adapters below so the two models cannot drift on what
 * a condition means. `scope` for the producer conditions remains aggregate, as the black-oil
 * runtime has always evaluated it.
 */
export function evaluateTerminationRates(
  policy: ScenarioTerminationPolicy | null | undefined,
  rates: TerminationRates,
): TerminationMatch | null {
  if (!policy || policy.conditions.length === 0) return null;

  const evaluations = policy.conditions.map((condition) => ({
    condition,
    ...evaluateCondition(condition, rates),
  }));

  if (policy.mode === 'all') {
    const allMatched = evaluations.every((evaluation) => evaluation.matched);
    if (!allMatched) return null;

    const summary = evaluations.map((evaluation) => evaluation.summary).join(' ');
    return {
      summary,
      condition: evaluations[0].condition,
      actualValue: evaluations[0].actualValue,
    };
  }

  const matched = evaluations.find((evaluation) => evaluation.matched);
  if (!matched) return null;

  return {
    summary: matched.summary,
    condition: matched.condition,
    actualValue: matched.actualValue,
  };
}

function blackOilRates(
  point: RateHistoryPoint,
  payload: SimulatorCreatePayload,
): TerminationRates {
  return {
    oilRate: producerOilRate(point),
    waterRate: producerWaterRate(point),
    gasRate: producerGasRate(point),
    liquidRate: producerLiquidRate(point),
    // An absent GOR reads as zero, which preserves the black-oil runtime's existing behaviour.
    gor: Math.max(0, toFiniteNumber(point.producing_gor, 0)),
    injection: {
      water: injectorPhaseRate(payload, point, 'water') ?? undefined,
      gas: injectorPhaseRate(payload, point, 'gas') ?? undefined,
    },
  };
}

export function cloneTerminationPolicy(
  policy: ScenarioTerminationPolicy | null | undefined,
): ScenarioTerminationPolicy | undefined {
  if (!policy) return undefined;
  return {
    mode: policy.mode,
    conditions: policy.conditions.map((condition) => ({ ...condition })),
  };
}

export function evaluateTerminationPolicy(
  policy: ScenarioTerminationPolicy | null | undefined,
  point: RateHistoryPoint | null | undefined,
  payload: SimulatorCreatePayload,
): TerminationMatch | null {
  if (!policy || !point || policy.conditions.length === 0) return null;
  return evaluateTerminationRates(policy, blackOilRates(point, payload));
}

/**
 * Evaluate a policy against a compositional snapshot's surface rates.
 *
 * The compositional engine reports separator **liquid** and **vapour**, not black-oil oil and gas;
 * this adapter maps them to the producer slots the condition vocabulary uses. A snapshot without
 * `well_surface_rates` returns no match rather than treating unreported rates as zero.
 *
 * This is a termination gate only. Nothing here feeds compositional output into a black-oil
 * analytical overlay.
 */
export function evaluateCompositionalTerminationPolicy(
  policy: ScenarioTerminationPolicy | null | undefined,
  snapshot: CompositionalSnapshot | null | undefined,
): TerminationMatch | null {
  if (!policy || !snapshot || policy.conditions.length === 0) return null;

  const produced = producedSurfaceRates(snapshot);
  if (!produced) return null;

  const injection = injectedSurfaceRates(snapshot);
  const water = produced.water ?? 0;
  return evaluateTerminationRates(policy, {
    oilRate: produced.liquid,
    waterRate: water,
    gasRate: produced.vapour,
    liquidRate: produced.liquid + water,
    gor: produced.liquid > 0 ? produced.vapour / produced.liquid : null,
    injection,
  });
}

/** Sum the injection side of each well's signed surface ledger. */
function injectedSurfaceRates(
  snapshot: CompositionalSnapshot,
): NonNullable<TerminationRates['injection']> {
  let oil = 0;
  let water = 0;
  let gas = 0;
  let hasWater = false;
  for (const well of snapshot.well_surface_rates ?? []) {
    oil += Math.max(0, toFiniteNumber(well.liquid_m3_per_day, 0));
    gas += Math.max(0, toFiniteNumber(well.vapour_m3_per_day, 0));
    if (well.water_m3_per_day !== undefined) {
      hasWater = true;
      water += Math.max(0, toFiniteNumber(well.water_m3_per_day, 0));
    }
  }
  return { oil, water: hasWater ? water : 0, gas };
}
