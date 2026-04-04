import type { ScenarioTerminationCondition, ScenarioTerminationPolicy } from '../catalog/scenarios';
import type { RateHistoryPoint, SimulatorCreatePayload } from '../simulator-types';

export type TerminationMatch = {
  summary: string;
  condition: ScenarioTerminationCondition;
  actualValue: number;
};

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

function producerWatercut(point: RateHistoryPoint): number {
  const liquid = producerLiquidRate(point);
  if (liquid <= 1e-12) return 0;
  return Math.min(1, Math.max(0, producerWaterRate(point) / liquid));
}

function injectorPhaseRate(payload: SimulatorCreatePayload, point: RateHistoryPoint, phase: 'oil' | 'water' | 'gas'): number | null {
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
  point: RateHistoryPoint,
  payload: SimulatorCreatePayload,
): { matched: boolean; actualValue: number; summary: string } {
  if (condition.kind === 'watercut-threshold') {
    const actualValue = producerWatercut(point);
    return {
      matched: actualValue >= condition.value,
      actualValue,
      summary: `Watercut threshold reached (${formatValue(actualValue)} >= ${formatValue(condition.value)}).`,
    };
  }

  if (condition.kind === 'gor-threshold') {
    const actualValue = Math.max(0, toFiniteNumber(point.producing_gor, 0));
    return {
      matched: compareValue(actualValue, condition.relation, condition.value),
      actualValue,
      summary: `GOR threshold reached (${formatValue(actualValue)} ${condition.relation === 'lte' ? '<=' : '>='} ${formatValue(condition.value)}).`,
    };
  }

  const producerValue = condition.phase === 'oil'
    ? producerOilRate(point)
    : condition.phase === 'water'
      ? producerWaterRate(point)
      : producerGasRate(point);
  const injectorValue = injectorPhaseRate(payload, point, condition.phase);

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

  const evaluations = policy.conditions.map((condition) => ({
    condition,
    ...evaluateCondition(condition, point, payload),
  }));

  const matched = policy.mode === 'all'
    ? evaluations.every((evaluation) => evaluation.matched)
    : evaluations.find((evaluation) => evaluation.matched);

  if (!matched) return null;

  if (policy.mode === 'all') {
    const summary = evaluations.map((evaluation) => evaluation.summary).join(' ');
    return {
      summary,
      condition: evaluations[0].condition,
      actualValue: evaluations[0].actualValue,
    };
  }

  return {
    summary: matched.summary,
    condition: matched.condition,
    actualValue: matched.actualValue,
  };
}