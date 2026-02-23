<script lang="ts">
  import { onDestroy, onMount, tick } from 'svelte';
  import FormattedNumberInput from './FormattedNumberInput.svelte';
  import { randomNormal, percentile, summarize, type PercentileSeries } from '../utils/statistics';
  import {
    PROVINCES, MONTH_NAMES, DAY_TO_MONTH_INDEX, DAYS_IN_MONTH, TOTAL_DAYS,
    GAS_KWH_PER_M3, SUPPLY_TEMP_OPTIONS,
    getProvince, copAtTemperature, hddBaseTemp, windFactor,
    type WindExposure,
  } from '../data/weatherData';

  type CostSummary = {
    percentiles: PercentileSeries<number[]>;
    hpTotals: PercentileSeries<number>;
    gasTotals: PercentileSeries<number>;
    cheaperProbability: number;
    cheaperProbability10: number;
    cheaperProbabilityByHorizon: {
      y10: number | null;
      y15: number | null;
      y20: number | null;
    };
    paybackYear: number | null;
    payback: {
      p25: number | null;
      p50: number | null;
      p75: number | null;
    };
    emissionsGas: PercentileSeries<number>;
    emissionsHp: PercentileSeries<number>;
    energyGas: PercentileSeries<number>;
    energyHp: PercentileSeries<number>;
    scop: PercentileSeries<number>;
    snapshots: {
      y10: {
        savings: PercentileSeries<number>;
        hpTotals: PercentileSeries<number>;
        gasTotals: PercentileSeries<number>;
        emissionsGas: PercentileSeries<number>;
        emissionsHp: PercentileSeries<number>;
        emissionsSavings: PercentileSeries<number>;
        energyGas: PercentileSeries<number>;
        energyHp: PercentileSeries<number>;
        roi: PercentileSeries<number>;
      } | null;
      y15: {
        savings: PercentileSeries<number>;
        hpTotals: PercentileSeries<number>;
        gasTotals: PercentileSeries<number>;
        emissionsGas: PercentileSeries<number>;
        emissionsHp: PercentileSeries<number>;
        emissionsSavings: PercentileSeries<number>;
        energyGas: PercentileSeries<number>;
        energyHp: PercentileSeries<number>;
        roi: PercentileSeries<number>;
      } | null;
      y20: {
        savings: PercentileSeries<number>;
        hpTotals: PercentileSeries<number>;
        gasTotals: PercentileSeries<number>;
        emissionsGas: PercentileSeries<number>;
        emissionsHp: PercentileSeries<number>;
        emissionsSavings: PercentileSeries<number>;
        energyGas: PercentileSeries<number>;
        energyHp: PercentileSeries<number>;
        roi: PercentileSeries<number>;
      } | null;
    };
    dailyGas: PercentileSeries<number[]>;
    dailyHp: PercentileSeries<number[]>;
    annual: {
      gas: PercentileSeries<number[]>;
      hp: PercentileSeries<number[]>;
      delta: PercentileSeries<number[]>;
    };
  };

  let Plotly: any;
  let chartEl: HTMLDivElement | null = null;
  let monthlyChartEl: HTMLDivElement | null = null;
  let running = false;
  let errorMessage = '';
  let results: CostSummary | null = null;
  let runStatusMessage = '';
  let resultStage: 'idle' | 'preview' | 'final' = 'idle';
  let previewRecalcTimer: ReturnType<typeof setTimeout> | null = null;
  let relayoutHandlerAttached = false;
  let applyingTickRelayout = false;
  let defaultYAxisTickValues: number[] = [];
  let defaultYAxisTickLabels: string[] = [];
  let defaultYAxisRange: [number, number] = [0, 0];
  let defaultXAxisRange: [number, number] = [0, 0];
  let firstChartTooltipVisible = false;
  let firstChartTooltipHtml = '';
  let firstChartTooltipLeft = 12;
  let firstChartTooltipTop = 12;
  let firstChartSummary: CostSummary | null = null;
  let firstChartTooltipEl: HTMLDivElement | null = null;
  let secondChartTooltipVisible = false;
  let secondChartTooltipHtml = '';
  let secondChartTooltipLeft = 12;
  let secondChartTooltipTop = 12;
  let secondChartSummary: CostSummary | null = null;
  let secondChartTooltipEl: HTMLDivElement | null = null;
  let monthlyHoverHandlerAttached = false;

  let outputDecimalSeparator: '.' | ',' = '.';
  let outputNumberLocale: 'en-US' | 'nl-NL' = 'en-US';
  let plotlyNumberLocale = 'hp-dot';
  let outputThousandsSeparator = ',';

  let currencyFormatter = new Intl.NumberFormat(outputNumberLocale, {
    style: 'currency',
    currency: 'EUR',
    maximumFractionDigits: 0
  });
  let percentFormatter = new Intl.NumberFormat(outputNumberLocale, {
    style: 'percent',
    maximumFractionDigits: 1
  });

  let emissionsFormatter = new Intl.NumberFormat(outputNumberLocale, {
    maximumFractionDigits: 0
  });
  let energyFormatter = new Intl.NumberFormat(outputNumberLocale, {
    maximumFractionDigits: 0
  });
  let hoverNumberFormatter = new Intl.NumberFormat(outputNumberLocale, {
    maximumSignificantDigits: 3
  });

  $: {
    outputNumberLocale = outputDecimalSeparator === '.' ? 'en-US' : 'nl-NL';
    plotlyNumberLocale = outputDecimalSeparator === '.' ? 'hp-dot' : 'hp-comma';
    outputThousandsSeparator = outputDecimalSeparator === '.' ? ',' : '.';
    currencyFormatter = new Intl.NumberFormat(outputNumberLocale, {
      style: 'currency',
      currency: 'EUR',
      maximumFractionDigits: 0
    });
    percentFormatter = new Intl.NumberFormat(outputNumberLocale, {
      style: 'percent',
      maximumFractionDigits: 1
    });
    emissionsFormatter = new Intl.NumberFormat(outputNumberLocale, {
      maximumFractionDigits: 0
    });
    energyFormatter = new Intl.NumberFormat(outputNumberLocale, {
      maximumFractionDigits: 0
    });
    hoverNumberFormatter = new Intl.NumberFormat(outputNumberLocale, {
      maximumSignificantDigits: 3
    });
  }

  let input = {
    province: 'Zuid-Holland',
    dwellingType: 'Row house',
    floorArea: 120,
    insulationQuality: 'Average',
    annualHeatDemand: 10000,
    hotWaterDemand: 2500,
    windExposure: 'Normal' as WindExposure,
    gasPrice: 1.25,
    electricityPrice: 0.3,
    gasBoilerEfficiency: 0.9,
    heatPumpCop: 3.5,
    supplyTemp: 45,
    heatPumpCapex: 9000,
    boilerReplacement: 2500,
    boilerCapexStd: 400,
    annualMaintenanceGas: 220,
    annualMaintenanceHp: 180,
    solarAvailableKwh: 1800,
    solarAvailableStdPct: 20,
    solarEffectivePrice: 0.08,
    gridMixCo2: 0.28,
    gasCo2: 0.202,
    carbonPrice: 0,
    retrofitGrant: 2500,
    minCop: 1.5,
    backupThreshold: -15,
    
    // Simulation settings
    simulations: 1500,
    horizonYears: 20,
    gasEscalationPct: 2,
    electricityEscalationPct: 2,
    gasPriceVolatilityPct: 10,
    electricityPriceVolatilityPct: 10,
    capexStd: 1200,
    demandStdPct: 7,
  };

  type DemandInputMode = 'estimate' | 'known-demand' | 'gas-consumption';

  const HOT_WATER_KWH_PER_PERSON = 800;

  let demandInputMode: DemandInputMode = 'estimate';
  let householdSize = 2;
  let knownSpaceHeatingDemand = 10000;
  let knownHotWaterDemand = 2500;
  let yearlyGasConsumptionM3 = 1500;
  let summerGasConsumptionM3PerMonth = 90;

  $: nominalSpaceDemand = Math.round(input.floorArea * 85);
  $: demandAdjustmentFactor = getDemandMultiplier() * getWindExposureFactor();
  $: estimatedSpaceDemand = Math.round(nominalSpaceDemand * demandAdjustmentFactor);
  $: estimatedHotWaterDemand = Math.round(Math.max(0, householdSize) * HOT_WATER_KWH_PER_PERSON);
  $: inferredHotWaterGasM3Year = Math.max(0, Math.min(yearlyGasConsumptionM3, summerGasConsumptionM3PerMonth * 12));
  $: inferredSpaceHeatingGasM3Year = Math.max(0, yearlyGasConsumptionM3 - inferredHotWaterGasM3Year);
  $: inferredSpaceHeatingDemand = Math.round(inferredSpaceHeatingGasM3Year * GAS_KWH_PER_M3 * input.gasBoilerEfficiency);
  $: inferredHotWaterDemand = Math.round(inferredHotWaterGasM3Year * GAS_KWH_PER_M3 * input.gasBoilerEfficiency);

  function toModelSpaceHeatingInput(actualSpaceHeatDemand: number): number {
    return Math.max(0, Math.round(actualSpaceHeatDemand / Math.max(0.1, demandAdjustmentFactor)));
  }

  $: {
    if (demandInputMode === 'estimate') {
      input.annualHeatDemand = toModelSpaceHeatingInput(estimatedSpaceDemand);
      input.hotWaterDemand = Math.max(0, estimatedHotWaterDemand);
    } else if (demandInputMode === 'known-demand') {
      input.annualHeatDemand = toModelSpaceHeatingInput(Math.max(0, knownSpaceHeatingDemand));
      input.hotWaterDemand = Math.max(0, knownHotWaterDemand);
    } else {
      input.annualHeatDemand = toModelSpaceHeatingInput(inferredSpaceHeatingDemand);
      input.hotWaterDemand = Math.max(0, inferredHotWaterDemand);
    }
  }

  $: previewTriggerKey = [
    input.province,
    input.dwellingType,
    input.floorArea,
    input.insulationQuality,
    input.annualHeatDemand,
    input.hotWaterDemand,
    input.windExposure,
    input.gasPrice,
    input.electricityPrice,
    input.gasBoilerEfficiency,
    input.heatPumpCop,
    input.supplyTemp,
    input.heatPumpCapex,
    input.boilerReplacement,
    input.boilerCapexStd,
    input.annualMaintenanceGas,
    input.annualMaintenanceHp,
    input.solarAvailableKwh,
    input.solarAvailableStdPct,
    input.solarEffectivePrice,
    input.gridMixCo2,
    input.gasCo2,
    input.carbonPrice,
    input.retrofitGrant,
    input.minCop,
    input.backupThreshold,
    input.simulations,
    input.horizonYears,
    input.gasEscalationPct,
    input.electricityEscalationPct,
    input.gasPriceVolatilityPct,
    input.electricityPriceVolatilityPct,
    input.capexStd,
    input.demandStdPct
  ].join('|');

  const insulationOptions = [
    { label: 'Poor', multiplier: 1.2 },
    { label: 'Average', multiplier: 1.0 },
    { label: 'Good', multiplier: 0.8 },
    { label: 'Very good', multiplier: 0.65 }
  ];

  const dwellingDefaults = {
    Apartment: 80,
    'Row house': 120,
    'Semi-detached': 160,
    Detached: 200
  } as Record<string, number>;

  onMount(async () => {
    const module = await import('plotly.js-dist-min');
    Plotly = module.default ?? module;
    if (results) {
      await tick();
      drawChart(results);
      drawMonthlyChart(results);
    }
  });

  onDestroy(() => {
    if (previewRecalcTimer) {
      clearTimeout(previewRecalcTimer);
      previewRecalcTimer = null;
    }
  });

  $: if (previewTriggerKey) {
    schedulePreviewRecalculation();
  }


  function handleDwellingChange() {
    input.floorArea = dwellingDefaults[input.dwellingType] ?? input.floorArea;
  }

  function getWindExposureFactor(): number {
    return windFactor(input.windExposure);
  }

  function getDemandMultiplier() {
    return insulationOptions.find((opt) => opt.label === input.insulationQuality)?.multiplier ?? 1;
  }

  function ensurePlotlyLocaleRegistered() {
    if (!Plotly) return;
    if ((Plotly as any).__hpLocaleName === plotlyNumberLocale) return;

    Plotly.register({
      moduleType: 'locale',
      name: plotlyNumberLocale,
      format: {
        decimal: outputDecimalSeparator,
        thousands: outputThousandsSeparator,
        grouping: [3],
        currency: ['', '']
      }
    });
    (Plotly as any).__hpLocaleName = plotlyNumberLocale;
  }

  function getPlotlyChartConfig(resetHandler: (gd: any) => void) {
    return {
      responsive: true,
      locale: plotlyNumberLocale,
      modeBarButtonsToRemove: ['resetScale2d', 'autoScale2d'],
      modeBarButtonsToAdd: [
        {
          name: 'Reset axes',
          title: 'Reset axes',
          icon: (Plotly as any)?.Icons?.home ?? (Plotly as any)?.Icons?.autoscale,
          click: (gd: any) => {
            resetHandler(gd);
          }
        }
      ]
    };
  }

  function toSignificant(value: number, digits = 3): number {
    if (!Number.isFinite(value) || value === 0) return 0;
    return Number(value.toPrecision(digits));
  }

  function formatHoverCompactNumber(value: number): string {
    const sign = value < 0 ? '-' : '';
    const abs = Math.abs(value);
    const compactFormatter = new Intl.NumberFormat(outputNumberLocale, {
      useGrouping: false,
      maximumSignificantDigits: 3
    });
    if (abs >= 1_000_000) {
      const millions = compactFormatter.format(toSignificant(abs / 1_000_000, 3));
      return `${sign}${millions} mln`;
    }
    if (abs >= 1_000) {
      const thousands = compactFormatter.format(toSignificant(abs / 1_000, 3));
      return `${sign}${thousands}k`;
    }
    return `${sign}${hoverNumberFormatter.format(toSignificant(abs, 3))}`;
  }

  function formatHoverEuro(value: number): string {
    const sign = value < 0 ? '-' : '';
    return `${sign}€ ${formatHoverCompactNumber(Math.abs(value))}`;
  }

  type AdaptiveScale = {
    divisor: number;
    suffix: string;
    decimals: number;
  };

  function requiredFractionDigitsForSignificant(valueAbs: number, significantDigits: number): number {
    if (!Number.isFinite(valueAbs) || valueAbs <= 0) return 0;
    const exponent = Math.floor(Math.log10(valueAbs));
    return Math.max(0, significantDigits - 1 - exponent);
  }

  function chooseAdaptiveScale(
    values: number[],
    candidates: Array<{ divisor: number; suffix: string }>,
    significantDigits = 3,
    preferredMaxDecimals = 3,
    hardMaxDecimals = 6,
  ): AdaptiveScale {
    const absValues = values.map((value) => Math.abs(value)).filter((value) => Number.isFinite(value));
    const nonZero = absValues.filter((value) => value > 0);
    const minAbs = nonZero.length > 0 ? Math.min(...nonZero) : 0;
    if (minAbs === 0 || candidates.length === 0) {
      const fallback = candidates[candidates.length - 1] ?? { divisor: 1, suffix: '' };
      return { ...fallback, decimals: 0 };
    }

    let fallback: AdaptiveScale | null = null;
    for (const candidate of candidates) {
      const scaledMin = minAbs / candidate.divisor;
      const required = requiredFractionDigitsForSignificant(scaledMin, significantDigits);
      if (required <= preferredMaxDecimals) {
        return { ...candidate, decimals: required };
      }
      if (!fallback && required <= hardMaxDecimals) {
        fallback = { ...candidate, decimals: required };
      }
    }

    if (fallback) return fallback;
    const smallestUnit = candidates[candidates.length - 1] ?? { divisor: 1, suffix: '' };
    return {
      ...smallestUnit,
      decimals: Math.min(hardMaxDecimals, requiredFractionDigitsForSignificant(minAbs / smallestUnit.divisor, significantDigits))
    };
  }

  function formatByScale(value: number, scale: AdaptiveScale, prefix = ''): string {
    const formatter = new Intl.NumberFormat(outputNumberLocale, {
      minimumFractionDigits: scale.decimals,
      maximumFractionDigits: scale.decimals,
      useGrouping: false,
    });
    const scaled = value / scale.divisor;
    return `${prefix}${formatter.format(scaled)}${scale.suffix}`;
  }

  function chooseCardMoneyDisplayScale(values: number[]): HoverDisplayScale {
    const absValues = values.map((value) => Math.abs(value)).filter((value) => Number.isFinite(value));
    if (absValues.length === 0) return { divisor: 1, suffix: '' };
    const maxAbs = Math.max(...absValues);
    return maxAbs >= 1_000 ? { divisor: 1_000, suffix: 'k' } : { divisor: 1, suffix: '' };
  }

  function extractSnapshotValues(
    results: CostSummary | null,
    selector: (snapshot: NonNullable<CostSummary['snapshots']['y10']>) => number[],
  ): number[] {
    if (!results) return [];
    const values: number[] = [];
    if (results.snapshots.y10) values.push(...selector(results.snapshots.y10));
    if (results.snapshots.y15) values.push(...selector(results.snapshots.y15));
    if (results.snapshots.y20) values.push(...selector(results.snapshots.y20));
    return values.filter((value) => Number.isFinite(value));
  }

  type HoverRangeRow = {
    label: string;
    p25: number;
    p50: number;
    p75: number;
  };

  type CompactScaled = {
    scaledAbs: number;
    suffix: string;
  };

  type HoverDisplayScale = {
    divisor: number;
    suffix: string;
  };

  type HoverEuroParts = {
    left: string;
    separator: string;
    right: string;
  };

  function padMonospace(value: string, width: number, alignRight = true): string {
    const content = value;
    if (content.length >= width) return content;
    const padding = ' '.repeat(width - content.length);
    return alignRight ? `${padding}${content}` : `${content}${padding}`;
  }

  function getCompactScaled(value: number): CompactScaled {
    const abs = Math.abs(value);
    if (abs >= 1_000_000) return { scaledAbs: abs / 1_000_000, suffix: ' mln' };
    if (abs >= 1_000) return { scaledAbs: abs / 1_000, suffix: 'k' };
    return { scaledAbs: abs, suffix: '' };
  }

  function requiredFractionDigitsForTwoSignificant(minScaledAbs: number): number {
    if (!Number.isFinite(minScaledAbs) || minScaledAbs <= 0) return 0;
    if (minScaledAbs >= 1) {
      const digitsBefore = Math.floor(Math.log10(minScaledAbs)) + 1;
      return Math.max(0, 2 - digitsBefore);
    }
    return 2 + Math.floor(-Math.log10(minScaledAbs));
  }

  function buildScaleCandidates(maxAbs: number): HoverDisplayScale[] {
    if (maxAbs >= 1_000_000) {
      return [
        { divisor: 1_000_000, suffix: ' mln' },
        { divisor: 1_000, suffix: 'k' },
        { divisor: 1, suffix: '' }
      ];
    }
    if (maxAbs >= 1_000) {
      return [
        { divisor: 1_000, suffix: 'k' },
        { divisor: 1, suffix: '' }
      ];
    }
    return [{ divisor: 1, suffix: '' }];
  }

  function chooseHoverDisplayScale(values: number[]): HoverDisplayScale {
    const absValues = values.map((value) => Math.abs(value)).filter((value) => Number.isFinite(value));
    if (absValues.length === 0) return { divisor: 1, suffix: '' };

    const maxAbs = Math.max(...absValues);
    const nonZero = absValues.filter((value) => value > 0);
    if (nonZero.length === 0) return { divisor: 1, suffix: '' };

    const minAbs = Math.min(...nonZero);
    const candidates = buildScaleCandidates(maxAbs);

    const preferred = candidates.find((candidate) => {
      const minScaled = minAbs / candidate.divisor;
      return requiredFractionDigitsForTwoSignificant(minScaled) <= 2;
    });
    if (preferred) return preferred;

    const fallbackReadable = candidates.find((candidate) => {
      const minScaled = minAbs / candidate.divisor;
      return requiredFractionDigitsForTwoSignificant(minScaled) <= 4;
    });
    if (fallbackReadable) return fallbackReadable;

    return candidates[candidates.length - 1];
  }

  function chooseHoverFractionDigits(values: number[], scale: HoverDisplayScale): number {
    const scaled = values
      .map((value) => Math.abs(value) / scale.divisor)
      .filter((value) => Number.isFinite(value));

    if (scaled.length === 0) return 0;
    const nonZero = scaled.filter((value) => value > 0);
    const minScaled = nonZero.length > 0 ? Math.min(...nonZero) : 0;
    if (minScaled === 0) return 0;

    return Math.min(6, requiredFractionDigitsForTwoSignificant(minScaled));
  }

  function splitHoverEuroParts(value: number, fractionDigits: number, scale: HoverDisplayScale): HoverEuroParts {
    const sign = value < 0 ? '-' : '';
    const scaledAbs = Math.abs(value) / scale.divisor;
    const formatter = new Intl.NumberFormat(outputNumberLocale, {
      minimumFractionDigits: fractionDigits,
      maximumFractionDigits: fractionDigits
    });
    const formatted = formatter.format(scaledAbs);
    const separatorIndex = formatted.lastIndexOf(outputDecimalSeparator);

    if (separatorIndex === -1) {
      return {
        left: `${sign}€ ${formatted}`,
        separator: '',
        right: scale.suffix
      };
    }

    const intPart = formatted.slice(0, separatorIndex);
    const fracPart = formatted.slice(separatorIndex + 1);
    return {
      left: `${sign}€ ${intPart}`,
      separator: outputDecimalSeparator,
      right: `${fracPart}${scale.suffix}`
    };
  }

  function formatAlignedEuroColumn(values: number[], scale: HoverDisplayScale, fixedFractionDigits?: number): string[] {
    const fractionDigits = fixedFractionDigits ?? chooseHoverFractionDigits(values, scale);
    const parts = values.map((value) => splitHoverEuroParts(value, fractionDigits, scale));
    const leftWidth = Math.max(0, ...parts.map((part) => part.left.length));
    const rightWidth = Math.max(0, ...parts.map((part) => part.right.length));
    const hasDecimal = parts.some((part) => part.separator === outputDecimalSeparator);

    return parts.map((part) => {
      const left = padMonospace(part.left, leftWidth);
      const separator = hasDecimal ? (part.separator || ' ') : '';
      const right = rightWidth > 0 ? padMonospace(part.right, rightWidth, false) : '';
      return rightWidth > 0 ? `${left}${separator}${right}` : `${left}${separator}`;
    });
  }

  function formatAlignedEuroTriplet(v1: number, v2: number, v3: number): [string, string, string] {
    const values = [v1, v2, v3];
    const scale = chooseHoverDisplayScale(values);
    const formatted = formatAlignedEuroColumn(values, scale);
    return [formatted[0], formatted[1], formatted[2]];
  }

  function alignFormattedBySeparator(values: string[]): string[] {
    const detectDecimalSeparatorIndex = (text: string): number => {
      const localeDecimalIdx = text.lastIndexOf(outputDecimalSeparator);
      if (localeDecimalIdx > -1 && /\d/.test(text.slice(localeDecimalIdx + 1))) {
        return localeDecimalIdx;
      }

      for (const separator of ['.', ',']) {
        const idx = text.lastIndexOf(separator);
        if (idx > -1 && /\d/.test(text.slice(idx + 1))) {
          return idx;
        }
      }

      return -1;
    };

    const parts = values.map((value) => {
      const text = value.trim();
      const separatorIndex = detectDecimalSeparatorIndex(text);

      if (separatorIndex === -1) {
        return { left: text, separator: '', right: '' };
      }

      return {
        left: text.slice(0, separatorIndex),
        separator: text[separatorIndex],
        right: text.slice(separatorIndex + 1)
      };
    });

    const leftWidth = Math.max(0, ...parts.map((part) => part.left.length));
    const rightWidth = Math.max(0, ...parts.map((part) => part.right.length));
    const hasSeparator = parts.some((part) => part.separator !== '');

    return parts.map((part) => {
      const left = padMonospace(part.left, leftWidth);
      if (!hasSeparator) return left;
      const separator = part.separator || ' ';
      const right = padMonospace(part.right, rightWidth, false);
      return `${left}${separator}${right}`;
    });
  }

  function buildMonospaceRangeLines(rows: HoverRangeRow[], forcedScale?: HoverDisplayScale, fixedFractionDigits?: number): string[] {
    const allValues = rows.flatMap((row) => [row.p25, row.p50, row.p75]);
    const sharedScale = forcedScale ?? chooseHoverDisplayScale(allValues);
    const lowValues = formatAlignedEuroColumn(rows.map((row) => row.p25), sharedScale, fixedFractionDigits);
    const meanValues = formatAlignedEuroColumn(rows.map((row) => row.p50), sharedScale, fixedFractionDigits);
    const highValues = formatAlignedEuroColumn(rows.map((row) => row.p75), sharedScale, fixedFractionDigits);

    const labelWidth = Math.max(5, ...rows.map((row) => row.label.length));
    const lowWidth = Math.max(3, ...lowValues.map((value) => value.length));
    const meanWidth = Math.max(4, ...meanValues.map((value) => value.length));
    const highWidth = Math.max(4, ...highValues.map((value) => value.length));

    const header = [
      padMonospace('', labelWidth, false),
      padMonospace('Low', lowWidth),
      `<b>${padMonospace('Mean', meanWidth)}</b>`,
      padMonospace('High', highWidth)
    ].join('  ');

    const body = rows.map((row, index) => [
      padMonospace(row.label, labelWidth, false),
      padMonospace(lowValues[index], lowWidth),
      `<b>${padMonospace(meanValues[index], meanWidth)}</b>`,
      padMonospace(highValues[index], highWidth)
    ].join('  '));

    return [header, ...body];
  }

  function wrapMonospaceHover(lines: string[]): string {
    return `<span style="font-family:'JetBrains Mono',ui-monospace,monospace;white-space:pre;line-height:1.05;">${lines.join('<br>')}</span>`;
  }

  function deltaColor(value: number): string {
    return value >= 0 ? '#16a34a' : '#dc2626';
  }

  function colorizeEuroTokens(line: string, color: string): string {
    return line.replace(/(-?€\s*[0-9.,'\s]+(?:[.,][0-9]+)?(?:\s?mln|k)?)/g, `<span style="color:${color};">$1</span>`);
  }

  function formatNullableYear(value: number | null): string {
    return value === null ? 'N/A' : `${value.toFixed(1)}y`;
  }

  function formatNullableYearsWord(value: number | null): string {
    return value === null ? 'N/A' : `${value.toFixed(1)} years`;
  }

  function safeRoiDenominator(value: number): number {
    return Math.max(1, Math.abs(value));
  }

  let hpMoneyFormatter = (value: number) => currencyFormatter.format(value);
  let gasMoneyFormatter = (value: number) => currencyFormatter.format(value);
  let co2FormatterCard = (value: number) => `${emissionsFormatter.format(value)} kg`;
  let energyFormatterCard = (value: number) => `${energyFormatter.format(value)} kWh`;
  let selectedPayoutChance: number | null = null;
  let alignedCostP50: { gas: string; hp: string; delta: string } = { gas: 'N/A', hp: 'N/A', delta: 'N/A' };
  let alignedCo2P50: { gas: string; hp: string; delta: string } = { gas: 'N/A', hp: 'N/A', delta: 'N/A' };
  let alignedEnergyP50: { gas: string; hp: string; delta: string } = { gas: 'N/A', hp: 'N/A', delta: 'N/A' };
  let selectedOutputHorizon: 10 | 15 | 20 = 10;
  let selectedSnapshot: CostSummary['snapshots']['y10'] | CostSummary['snapshots']['y15'] | CostSummary['snapshots']['y20'] | null = null;

  $: if (selectedOutputHorizon === 20 && !results?.snapshots.y20) {
    selectedOutputHorizon = results?.snapshots.y15 ? 15 : 10;
  }

  $: if (selectedOutputHorizon === 15 && !results?.snapshots.y15) {
    selectedOutputHorizon = 10;
  }

  $: selectedSnapshot = results
    ? (selectedOutputHorizon === 10
      ? results.snapshots.y10
      : selectedOutputHorizon === 15
        ? results.snapshots.y15
        : results.snapshots.y20)
    : null;

  $: selectedPayoutChance = results
    ? (selectedOutputHorizon === 10
      ? results.cheaperProbabilityByHorizon.y10
      : selectedOutputHorizon === 15
        ? results.cheaperProbabilityByHorizon.y15
        : results.cheaperProbabilityByHorizon.y20)
    : null;

  $: {
    if (selectedSnapshot) {
      const tripletValues = [
        selectedSnapshot.gasTotals.p50,
        selectedSnapshot.hpTotals.p50,
        selectedSnapshot.gasTotals.p50 - selectedSnapshot.hpTotals.p50,
      ];
      const [gas, hp, delta] = formatAlignedEuroColumn(tripletValues, chooseCardMoneyDisplayScale(tripletValues));
      alignedCostP50 = { gas, hp, delta };
    } else {
      alignedCostP50 = { gas: 'N/A', hp: 'N/A', delta: 'N/A' };
    }
  }

  $: {
    const hpValues = extractSnapshotValues(results, (snapshot) => [
      snapshot.hpTotals.p25,
      snapshot.hpTotals.p50,
      snapshot.hpTotals.p75,
    ]);
    const hpScale = chooseAdaptiveScale(
      hpValues,
      [
        { divisor: 1_000, suffix: 'k' },
        { divisor: 1, suffix: '' }
      ]
    );
    hpMoneyFormatter = (value: number) => formatByScale(value, hpScale, '€ ');

    const gasValues = extractSnapshotValues(results, (snapshot) => [
      snapshot.gasTotals.p25,
      snapshot.gasTotals.p50,
      snapshot.gasTotals.p75,
    ]);
    const gasScale = chooseAdaptiveScale(
      gasValues,
      [
        { divisor: 1_000, suffix: 'k' },
        { divisor: 1, suffix: '' }
      ]
    );
    gasMoneyFormatter = (value: number) => formatByScale(value, gasScale, '€ ');

    const co2Values = extractSnapshotValues(results, (snapshot) => [
      snapshot.emissionsGas.p25,
      snapshot.emissionsGas.p50,
      snapshot.emissionsGas.p75,
      snapshot.emissionsHp.p25,
      snapshot.emissionsHp.p50,
      snapshot.emissionsHp.p75,
      snapshot.emissionsSavings.p25,
      snapshot.emissionsSavings.p50,
      snapshot.emissionsSavings.p75,
    ]);
    const co2Scale = chooseAdaptiveScale(
      co2Values,
      [
        { divisor: 1_000, suffix: ' t' },
        { divisor: 1, suffix: ' kg' }
      ]
    );
    co2FormatterCard = (value: number) => formatByScale(value, co2Scale);

    const energyValues = extractSnapshotValues(results, (snapshot) => [
      snapshot.energyGas.p25,
      snapshot.energyGas.p50,
      snapshot.energyGas.p75,
      snapshot.energyHp.p25,
      snapshot.energyHp.p50,
      snapshot.energyHp.p75,
      snapshot.energyGas.p25 - snapshot.energyHp.p25,
      snapshot.energyGas.p50 - snapshot.energyHp.p50,
      snapshot.energyGas.p75 - snapshot.energyHp.p75,
    ]);
    const energyScale = chooseAdaptiveScale(
      energyValues,
      [
        { divisor: 1_000_000, suffix: ' GWh' },
        { divisor: 1_000, suffix: ' MWh' },
        { divisor: 1, suffix: ' kWh' }
      ]
    );
    energyFormatterCard = (value: number) => formatByScale(value, energyScale);
  }

  $: {
    if (selectedSnapshot) {
      const [co2Gas, co2Hp, co2Delta] = alignFormattedBySeparator([
        co2FormatterCard(selectedSnapshot.emissionsGas.p50),
        co2FormatterCard(selectedSnapshot.emissionsHp.p50),
        co2FormatterCard(selectedSnapshot.emissionsSavings.p50),
      ]);
      alignedCo2P50 = { gas: co2Gas, hp: co2Hp, delta: co2Delta };

      const [energyGas, energyHp, energyDelta] = alignFormattedBySeparator([
        energyFormatterCard(selectedSnapshot.energyGas.p50),
        energyFormatterCard(selectedSnapshot.energyHp.p50),
        energyFormatterCard(selectedSnapshot.energyGas.p50 - selectedSnapshot.energyHp.p50),
      ]);
      alignedEnergyP50 = { gas: energyGas, hp: energyHp, delta: energyDelta };
    } else {
      alignedCo2P50 = { gas: 'N/A', hp: 'N/A', delta: 'N/A' };
      alignedEnergyP50 = { gas: 'N/A', hp: 'N/A', delta: 'N/A' };
    }
  }

  function buildSnapshotFromMoments(
    gasMean: number,
    gasVar: number,
    hpMean: number,
    hpVar: number,
    emissionsGasMean: number,
    emissionsGasVar: number,
    emissionsHpMean: number,
    emissionsHpVar: number,
    energyGasMean: number,
    energyGasVar: number,
    energyHpMean: number,
    energyHpVar: number,
    investmentBase: number,
  ) {
    const gasTotals = normalQuantiles(gasMean, gasVar);
    const hpTotals = normalQuantiles(hpMean, hpVar);
    const savings = normalQuantiles(gasMean - hpMean, gasVar + hpVar);
    const emissionsGas = normalQuantiles(emissionsGasMean, emissionsGasVar);
    const emissionsHp = normalQuantiles(emissionsHpMean, emissionsHpVar);
    const emissionsSavings = normalQuantiles(
      emissionsGasMean - emissionsHpMean,
      emissionsGasVar + emissionsHpVar
    );
    const energyGas = normalQuantiles(energyGasMean, energyGasVar);
    const energyHp = normalQuantiles(energyHpMean, energyHpVar);
    const roiDenominator = safeRoiDenominator(investmentBase);
    const roi: PercentileSeries<number> = {
      p10: savings.p10 / roiDenominator,
      p25: savings.p25 / roiDenominator,
      p50: savings.p50 / roiDenominator,
      p75: savings.p75 / roiDenominator,
      p90: savings.p90 / roiDenominator,
    };

    return {
      savings,
      hpTotals,
      gasTotals,
      emissionsGas,
      emissionsHp,
      emissionsSavings,
      energyGas,
      energyHp,
      roi,
    };
  }

  function paybackFromSeries(
    series: number[],
    predicate: (value: number) => boolean,
  ): number | null {
    const index = series.findIndex(predicate);
    return index === -1 ? null : Number((Math.max(0, index) / 12).toFixed(1));
  }

  function buildFirstChartHoverTable(summary: CostSummary, pointIndex: number, pointYears: number): string {
    const yearIndex = Math.floor(pointIndex / 12);
    const safeYear = Math.min(yearIndex, summary.annual.gas.p50.length - 1);
    const lines = buildMonospaceRangeLines([
      {
        label: 'Gas',
        p25: summary.annual.gas.p25[safeYear],
        p50: summary.annual.gas.p50[safeYear],
        p75: summary.annual.gas.p75[safeYear]
      },
      {
        label: 'HP',
        p25: summary.annual.hp.p25[safeYear],
        p50: summary.annual.hp.p50[safeYear],
        p75: summary.annual.hp.p75[safeYear]
      },
      {
        label: 'Δ',
        p25: summary.annual.delta.p25[safeYear],
        p50: summary.annual.delta.p50[safeYear],
        p75: summary.annual.delta.p75[safeYear]
      },
      {
        label: `Cum.${summary.percentiles.p50[pointIndex] >= 0 ? '+' : '-'}`,
        p25: summary.percentiles.p25[pointIndex],
        p50: summary.percentiles.p50[pointIndex],
        p75: summary.percentiles.p75[pointIndex]
      }
    ], { divisor: 1, suffix: '' });
    const pointLabel = pointYears.toFixed(3);
    const deltaLineColor = deltaColor(summary.annual.delta.p50[safeYear]);
    const cumulativeColor = deltaColor(summary.percentiles.p50[pointIndex]);
    const linesWithDeltaColor = [...lines];
    linesWithDeltaColor[3] = colorizeEuroTokens(lines[3], deltaLineColor);
    linesWithDeltaColor[4] = colorizeEuroTokens(lines[4], cumulativeColor);

    return `<b>Year ${safeYear + 1}</b> (${pointLabel})<br>${wrapMonospaceHover(linesWithDeltaColor)}`;
  }

  function hideFirstChartTooltip() {
    firstChartTooltipVisible = false;
  }

  function showFirstChartTooltip(eventData: any) {
    if (!chartEl) return;
    const points = eventData?.points as Array<any> | undefined;
    if (!points || points.length === 0) return;

    const targetPoint = points.find((point) => typeof point?.customdata === 'string') ?? points[0];
    let html = targetPoint?.customdata;
    let pointIndex: number | null = null;
    if (firstChartSummary) {
      const hoveredX = Number(targetPoint?.x);
      if (Number.isFinite(hoveredX)) {
        pointIndex = Math.min(
          firstChartSummary.percentiles.p50.length - 1,
          Math.max(0, Math.round(hoveredX * 12))
        );
      }
      if ((typeof html !== 'string' || html.length === 0) && pointIndex !== null) {
        html = buildFirstChartHoverTable(firstChartSummary, pointIndex, pointIndex / 12);
      }
    }
    if (typeof html !== 'string' || html.length === 0) return;

    const rect = chartEl.getBoundingClientRect();
    const mouseEvent = eventData?.event as MouseEvent | undefined;
    let left = 12;
    let top = 12;
    if (mouseEvent) {
      left = mouseEvent.clientX - rect.left + 12;
      top = mouseEvent.clientY - rect.top + 12;
    }

    if (firstChartSummary && pointIndex !== null) {
      const fullLayout = (chartEl as any)?._fullLayout;
      const xAxis = fullLayout?.xaxis;
      const yAxis = fullLayout?.yaxis;
      const plotOffsetLeft = Number(fullLayout?._size?.l ?? 0);
      const plotOffsetTop = Number(fullLayout?._size?.t ?? 0);
      const medianY = firstChartSummary.percentiles.p50[pointIndex];
      const pointYears = pointIndex / 12;
      if (typeof xAxis?.l2p === 'function') {
        left = plotOffsetLeft + xAxis.l2p(pointYears) + 14;
      }
      if (typeof yAxis?.l2p === 'function' && Number.isFinite(medianY)) {
        top = plotOffsetTop + yAxis.l2p(medianY) - 16;
      }
    }

    const tooltipWidth = Math.max(140, firstChartTooltipEl?.offsetWidth ?? 260);
    const tooltipHeight = Math.max(80, firstChartTooltipEl?.offsetHeight ?? 160);
    const maxLeft = Math.max(8, chartEl.clientWidth - tooltipWidth - 8);
    const maxTop = Math.max(8, chartEl.clientHeight - tooltipHeight - 8);
    firstChartTooltipLeft = Math.min(Math.max(8, left), maxLeft);
    firstChartTooltipTop = Math.min(Math.max(8, top), maxTop);
    firstChartTooltipHtml = html;
    firstChartTooltipVisible = true;
  }

  function buildDailyHoverRows(summary: CostSummary, dayIndex: number): {
    gas: string;
    hp: string;
    delta: string;
  } {
    const lines = buildMonospaceRangeLines([
      {
        label: 'Gas',
        p25: summary.dailyGas.p25[dayIndex],
        p50: summary.dailyGas.p50[dayIndex],
        p75: summary.dailyGas.p75[dayIndex]
      },
      {
        label: 'HP',
        p25: summary.dailyHp.p25[dayIndex],
        p50: summary.dailyHp.p50[dayIndex],
        p75: summary.dailyHp.p75[dayIndex]
      },
      {
        label: 'Δ',
        p25: summary.dailyGas.p25[dayIndex] - summary.dailyHp.p25[dayIndex],
        p50: summary.dailyGas.p50[dayIndex] - summary.dailyHp.p50[dayIndex],
        p75: summary.dailyGas.p75[dayIndex] - summary.dailyHp.p75[dayIndex]
      }
    ]);
    const deltaLineColor = deltaColor(summary.dailyGas.p50[dayIndex] - summary.dailyHp.p50[dayIndex]);

    return {
      gas: wrapMonospaceHover([lines[1]]),
      hp: wrapMonospaceHover([lines[2]]),
      delta: wrapMonospaceHover([colorizeEuroTokens(lines[3], deltaLineColor)])
    };
  }

  function buildSecondChartHoverTable(summary: CostSummary, dayIndex: number): string {
    const safeDay = Math.min(TOTAL_DAYS - 1, Math.max(0, dayIndex));
    const lines = buildMonospaceRangeLines([
      {
        label: 'Gas',
        p25: summary.dailyGas.p25[safeDay],
        p50: summary.dailyGas.p50[safeDay],
        p75: summary.dailyGas.p75[safeDay]
      },
      {
        label: 'HP',
        p25: summary.dailyHp.p25[safeDay],
        p50: summary.dailyHp.p50[safeDay],
        p75: summary.dailyHp.p75[safeDay]
      },
      {
        label: 'Δ',
        p25: summary.dailyGas.p25[safeDay] - summary.dailyHp.p25[safeDay],
        p50: summary.dailyGas.p50[safeDay] - summary.dailyHp.p50[safeDay],
        p75: summary.dailyGas.p75[safeDay] - summary.dailyHp.p75[safeDay]
      }
    ], { divisor: 1, suffix: '' }, 1);

    const deltaLineColor = deltaColor(summary.dailyGas.p50[safeDay] - summary.dailyHp.p50[safeDay]);
    const linesWithDeltaColor = [...lines];
    linesWithDeltaColor[3] = colorizeEuroTokens(lines[3], deltaLineColor);

    const date = new Date(Date.UTC(2021, 0, safeDay + 1));
    const dateLabel = new Intl.DateTimeFormat(outputNumberLocale, {
      day: 'numeric',
      month: 'long',
      timeZone: 'UTC'
    }).format(date);

    return `<b>${dateLabel}</b><br>${wrapMonospaceHover(linesWithDeltaColor)}`;
  }

  function hideSecondChartTooltip() {
    secondChartTooltipVisible = false;
  }

  function showSecondChartTooltip(eventData: any) {
    if (!monthlyChartEl || !secondChartSummary) return;
    const points = eventData?.points as Array<any> | undefined;
    if (!points || points.length === 0) return;

    const targetPoint = points.find((point) => Number.isFinite(Number(point?.pointIndex))) ?? points[0];
    let dayIndex = Number(targetPoint?.pointIndex);
    if (!Number.isFinite(dayIndex)) {
      const dateValue = targetPoint?.x;
      if (typeof dateValue === 'string') {
        const date = new Date(`${dateValue}T00:00:00Z`);
        if (!Number.isNaN(date.getTime())) {
          const start = Date.UTC(2021, 0, 1);
          dayIndex = Math.floor((date.getTime() - start) / 86_400_000);
        }
      }
    }
    if (!Number.isFinite(dayIndex)) return;

    const safeDayIndex = Math.min(TOTAL_DAYS - 1, Math.max(0, Math.round(dayIndex)));
    const html = buildSecondChartHoverTable(secondChartSummary, safeDayIndex);

    const rect = monthlyChartEl.getBoundingClientRect();
    const mouseEvent = eventData?.event as MouseEvent | undefined;
    let left = 12;
    let top = 12;
    if (mouseEvent) {
      left = mouseEvent.clientX - rect.left + 12;
      top = mouseEvent.clientY - rect.top + 12;
    }

    const fullLayout = (monthlyChartEl as any)?._fullLayout;
    const xAxis = fullLayout?.xaxis;
    const yAxis = fullLayout?.yaxis;
    const plotOffsetLeft = Number(fullLayout?._size?.l ?? 0);
    const plotOffsetTop = Number(fullLayout?._size?.t ?? 0);
    const dateIso = new Date(Date.UTC(2021, 0, safeDayIndex + 1)).toISOString().slice(0, 10);
    const gasMedian = secondChartSummary.dailyGas.p50[safeDayIndex];
    const hpMedian = secondChartSummary.dailyHp.p50[safeDayIndex];
    const anchorY = (gasMedian + hpMedian) / 2;

    let axisLeft = Number.NaN;
    if (typeof targetPoint?.xaxis?.d2p === 'function') {
      axisLeft = plotOffsetLeft + targetPoint.xaxis.d2p(targetPoint.x) + 14;
    } else if (typeof xAxis?.d2p === 'function') {
      axisLeft = plotOffsetLeft + xAxis.d2p(dateIso) + 14;
    } else if (typeof xAxis?.l2p === 'function') {
      axisLeft = plotOffsetLeft + xAxis.l2p(Date.parse(`${dateIso}T00:00:00Z`)) + 14;
    }
    if (Number.isFinite(axisLeft)) {
      left = axisLeft;
    }

    if (typeof yAxis?.l2p === 'function' && Number.isFinite(anchorY)) {
      const axisTop = plotOffsetTop + yAxis.l2p(anchorY) - 16;
      if (Number.isFinite(axisTop)) {
        top = axisTop;
      }
    }

    const tooltipWidth = Math.max(140, secondChartTooltipEl?.offsetWidth ?? 260);
    const tooltipHeight = Math.max(80, secondChartTooltipEl?.offsetHeight ?? 160);
    const maxLeft = Math.max(8, monthlyChartEl.clientWidth - tooltipWidth - 8);
    const maxTop = Math.max(8, monthlyChartEl.clientHeight - tooltipHeight - 8);
    secondChartTooltipLeft = Math.min(Math.max(8, left), maxLeft);
    secondChartTooltipTop = Math.min(Math.max(8, top), maxTop);
    secondChartTooltipHtml = html;
    secondChartTooltipVisible = true;
  }

  const Z_P10 = -1.2815515655446004;
  const Z_P25 = -0.6744897501960817;
  const Z_P75 = 0.6744897501960817;
  const Z_P90 = 1.2815515655446004;

  function safeStd(variance: number): number {
    if (!Number.isFinite(variance) || variance <= 0) return 0;
    return Math.sqrt(variance);
  }

  function normalQuantiles(mean: number, variance: number): PercentileSeries<number> {
    const std = safeStd(variance);
    return {
      p10: mean + Z_P10 * std,
      p25: mean + Z_P25 * std,
      p50: mean,
      p75: mean + Z_P75 * std,
      p90: mean + Z_P90 * std,
    };
  }

  function erfApprox(x: number): number {
    const sign = x < 0 ? -1 : 1;
    const absX = Math.abs(x);
    const t = 1 / (1 + 0.3275911 * absX);
    const y = 1 - (((((1.061405429 * t - 1.453152027) * t + 1.421413741) * t - 0.284496736) * t + 0.254829592) * t) * Math.exp(-absX * absX);
    return sign * y;
  }

  function normalCdf(x: number): number {
    return 0.5 * (1 + erfApprox(x / Math.SQRT2));
  }

  function derivativeCopAtTemperature(outdoorTemp: number): number {
    const step = 0.25;
    const low = copAtTemperature(input.heatPumpCop, input.supplyTemp, outdoorTemp - step, input.minCop, input.backupThreshold);
    const high = copAtTemperature(input.heatPumpCop, input.supplyTemp, outdoorTemp + step, input.minCop, input.backupThreshold);
    return (high - low) / (2 * step);
  }

  function derivativeHotWaterCop(outdoorTemp: number): number {
    const step = 0.25;
    const low = copAtTemperature(input.heatPumpCop, 55, outdoorTemp - step, input.minCop, input.backupThreshold);
    const high = copAtTemperature(input.heatPumpCop, 55, outdoorTemp + step, input.minCop, input.backupThreshold);
    return (high - low) / (2 * step);
  }

  function schedulePreviewRecalculation() {
    if (previewRecalcTimer) clearTimeout(previewRecalcTimer);
    previewRecalcTimer = setTimeout(() => {
      if (running) {
        schedulePreviewRecalculation();
        return;
      }
      void recomputeApproximatePreview();
    }, 120);
  }

  async function recomputeApproximatePreview() {
    errorMessage = '';
    const horizonMonths = Math.max(12, Math.round(input.horizonYears * 12));
    if (input.annualHeatDemand <= 0) {
      return;
    }
    const province = getProvince(input.province);
    if (!province) {
      return;
    }

    const previewSummary = buildErrorPropagationPreview(province, horizonMonths);
    results = previewSummary;
    resultStage = 'preview';
    runStatusMessage = 'Run probabilistic for better estimates.';
    await tick();
    drawChart(previewSummary);
    drawMonthlyChart(previewSummary);
  }

  function buildErrorPropagationPreview(
    province: NonNullable<ReturnType<typeof getProvince>>,
    horizonMonths: number,
  ): CostSummary {
    const horizonYears = Math.ceil(horizonMonths / 12);
    const insulationMultiplier = getDemandMultiplier();
    const wFactor = getWindExposureFactor();
    const baseDemand = input.annualHeatDemand * insulationMultiplier * wFactor;
    const sigmaYearDemand = baseDemand * (input.demandStdPct / 100);
    const baseTemp = hddBaseTemp(input.insulationQuality);
    const hotWaterDaily = input.hotWaterDemand / TOTAL_DAYS;
    const gasPriceMean0 = input.gasPrice / GAS_KWH_PER_M3;
    const gasPriceStd0 = (input.gasPrice * (input.gasPriceVolatilityPct / 100)) / GAS_KWH_PER_M3;

    const dailyHddMean = new Array(TOTAL_DAYS).fill(0);
    const dailyHddStd = new Array(TOTAL_DAYS).fill(0);
    const dailyCopHeatMean = new Array(TOTAL_DAYS).fill(0);
    const dailyCopHeatStd = new Array(TOTAL_DAYS).fill(0);
    const dailyCopHwMean = new Array(TOTAL_DAYS).fill(0);
    const dailyCopHwStd = new Array(TOTAL_DAYS).fill(0);

    for (let day = 0; day < TOTAL_DAYS; day += 1) {
      const month = DAY_TO_MONTH_INDEX[day];
      const tempMean = province.dailyMeanByDay[day];
      const tempStd = Math.sqrt(province.dailyStdByDay[day] ** 2 + province.yearStd[month] ** 2);
      const hddMean = Math.max(0, baseTemp - tempMean);
      const hddStd = tempMean < baseTemp ? tempStd : 0;
      dailyHddMean[day] = hddMean;
      dailyHddStd[day] = hddStd;

      const heatCopMean = copAtTemperature(input.heatPumpCop, input.supplyTemp, tempMean, input.minCop, input.backupThreshold);
      const heatCopDeriv = derivativeCopAtTemperature(tempMean);
      dailyCopHeatMean[day] = heatCopMean;
      dailyCopHeatStd[day] = Math.abs(heatCopDeriv) * tempStd;

      const hwCopMean = copAtTemperature(input.heatPumpCop, 55, tempMean, input.minCop, input.backupThreshold);
      const hwCopDeriv = derivativeHotWaterCop(tempMean);
      dailyCopHwMean[day] = hwCopMean;
      dailyCopHwStd[day] = Math.abs(hwCopDeriv) * tempStd;
    }

    const totalHddMean = dailyHddMean.reduce((sum, value) => sum + value, 0);
    const dayDemandShare = dailyHddMean.map((value) => (totalHddMean > 0 ? value / totalHddMean : 0));

    const dayGasUsageMean: number[] = new Array(TOTAL_DAYS).fill(0);
    const dayGasUsageVar: number[] = new Array(TOTAL_DAYS).fill(0);
    const dayHpUsageMean: number[] = new Array(TOTAL_DAYS).fill(0);
    const dayHpUsageVar: number[] = new Array(TOTAL_DAYS).fill(0);

    const effMean = Math.max(0.5, input.gasBoilerEfficiency);
    const effStd = 0.02;

    for (let day = 0; day < TOTAL_DAYS; day += 1) {
      const share = dayDemandShare[day];
      const muDemand = baseDemand * share;
      const sigmaDemand = sigmaYearDemand * share + (baseDemand * dailyHddStd[day] / Math.max(totalHddMean, 1e-9));

      const heatCopMean = Math.max(input.minCop, dailyCopHeatMean[day]);
      const heatCopStd = dailyCopHeatStd[day];

      const muHpSpace = muDemand / heatCopMean;
      const varHpSpace = (sigmaDemand / heatCopMean) ** 2 + ((muDemand * heatCopStd) / (heatCopMean ** 2)) ** 2;

      const muGasSpace = muDemand / effMean;
      const varGasSpace = (sigmaDemand / effMean) ** 2 + ((muDemand * effStd) / (effMean ** 2)) ** 2;

      const hwCopMean = Math.max(input.minCop, dailyCopHwMean[day]);
      const hwCopStd = dailyCopHwStd[day];

      const muHpHw = hotWaterDaily / hwCopMean;
      const varHpHw = ((hotWaterDaily * hwCopStd) / (hwCopMean ** 2)) ** 2;

      const muGasHw = hotWaterDaily / effMean;
      const varGasHw = ((hotWaterDaily * effStd) / (effMean ** 2)) ** 2;

      dayHpUsageMean[day] = muHpSpace + muHpHw;
      dayHpUsageVar[day] = varHpSpace + varHpHw;
      dayGasUsageMean[day] = muGasSpace + muGasHw;
      dayGasUsageVar[day] = varGasSpace + varGasHw;
    }

    const dailyGasCostsSeries: PercentileSeries<number[]> = { p10: [], p25: [], p50: [], p75: [], p90: [] };
    const dailyHpCostsSeries: PercentileSeries<number[]> = { p10: [], p25: [], p50: [], p75: [], p90: [] };
    const percentilesDiff: PercentileSeries<number[]> = { p10: [], p25: [], p50: [], p75: [], p90: [] };

    let cumulativeGasMean = input.boilerReplacement;
    let cumulativeHpMean = Math.max(0, input.heatPumpCapex - Math.max(0, input.retrofitGrant));
    let cumulativeGasVar = input.boilerCapexStd ** 2;
    let cumulativeHpVar = input.capexStd ** 2;

    const monthlyDaysStart: number[] = [];
    let cursor = 0;
    for (const days of DAYS_IN_MONTH) {
      monthlyDaysStart.push(cursor);
      cursor += days;
    }

    let totalGasEnergyMean = 0;
    let totalGasEnergyVar = 0;
    let totalHpEnergyMean = 0;
    let totalHpEnergyVar = 0;
    let totalGasEmissionsMean = 0;
    let totalGasEmissionsVar = 0;
    let totalHpEmissionsMean = 0;
    let totalHpEmissionsVar = 0;
    let totalHeatDeliveredMean = 0;
    let totalHeatDeliveredVar = 0;
    const annualGasMean = new Array(horizonYears).fill(0);
    const annualGasVar = new Array(horizonYears).fill(0);
    const annualHpMean = new Array(horizonYears).fill(0);
    const annualHpVar = new Array(horizonYears).fill(0);
    const investmentBase = Math.max(0, input.heatPumpCapex - Math.max(0, input.retrofitGrant)) - input.boilerReplacement;
    const snapshotMonths10 = 10 * 12;
    const snapshotMonths15 = 15 * 12;
    const snapshotMonths20 = 20 * 12;
    let snapshot10: CostSummary['snapshots']['y10'] = null;
    let snapshot15: CostSummary['snapshots']['y15'] = null;
    let snapshot20: CostSummary['snapshots']['y20'] = null;
    let cheaperProbability10 = 0;
    let cheaperProbability15: number | null = null;
    let cheaperProbability20: number | null = null;

    const solarDailyMean = Math.max(0, input.solarAvailableKwh) / TOTAL_DAYS;
    const solarDailyStd = solarDailyMean * (Math.max(0, input.solarAvailableStdPct) / 100);

    const firstYearGasPriceMean = gasPriceMean0;
    const firstYearGasPriceVar = gasPriceStd0 ** 2;
    const firstYearElecPriceMean = input.electricityPrice;
    const firstYearElecPriceVar = (input.electricityPrice * (input.electricityPriceVolatilityPct / 100)) ** 2;

    for (let day = 0; day < TOTAL_DAYS; day += 1) {
      const gasMu = dayGasUsageMean[day] * firstYearGasPriceMean + (dayGasUsageMean[day] * input.gasCo2) * input.carbonPrice + input.annualMaintenanceGas / TOTAL_DAYS;
      const gasVar = dayGasUsageVar[day] * firstYearGasPriceMean ** 2 + dayGasUsageMean[day] ** 2 * firstYearGasPriceVar + dayGasUsageVar[day] * (input.gasCo2 * input.carbonPrice) ** 2;
      const daySolarRatio = Math.max(0, Math.min(1, solarDailyMean / Math.max(dayHpUsageMean[day], 1e-9)));
      const dayGridRatio = 1 - daySolarRatio;
      const daySolarRatioStd = Math.max(0, Math.min(1, solarDailyStd / Math.max(dayHpUsageMean[day], 1e-9)));
      const blendedDayPriceMean = dayGridRatio * firstYearElecPriceMean + daySolarRatio * input.solarEffectivePrice;
      const blendedDayPriceVar =
        (dayGridRatio ** 2) * firstYearElecPriceVar +
        (((firstYearElecPriceMean - input.solarEffectivePrice) * daySolarRatioStd) ** 2);
      const hpMu = dayHpUsageMean[day] * blendedDayPriceMean + (dayHpUsageMean[day] * input.gridMixCo2 * dayGridRatio) * input.carbonPrice + input.annualMaintenanceHp / TOTAL_DAYS;
      const hpVar =
        dayHpUsageVar[day] * blendedDayPriceMean ** 2 +
        dayHpUsageMean[day] ** 2 * blendedDayPriceVar +
        dayHpUsageVar[day] * (input.gridMixCo2 * dayGridRatio * input.carbonPrice) ** 2;
      const gasQ = normalQuantiles(gasMu, gasVar);
      const hpQ = normalQuantiles(hpMu, hpVar);
      dailyGasCostsSeries.p10.push(gasQ.p10);
      dailyGasCostsSeries.p25.push(gasQ.p25);
      dailyGasCostsSeries.p50.push(gasQ.p50);
      dailyGasCostsSeries.p75.push(gasQ.p75);
      dailyGasCostsSeries.p90.push(gasQ.p90);
      dailyHpCostsSeries.p10.push(hpQ.p10);
      dailyHpCostsSeries.p25.push(hpQ.p25);
      dailyHpCostsSeries.p50.push(hpQ.p50);
      dailyHpCostsSeries.p75.push(hpQ.p75);
      dailyHpCostsSeries.p90.push(hpQ.p90);
    }

    for (let monthIndexAll = 0; monthIndexAll < horizonMonths; monthIndexAll += 1) {
      const year = Math.floor(monthIndexAll / 12);
      const month = monthIndexAll % 12;
      const monthStart = monthlyDaysStart[month];
      const monthDays = DAYS_IN_MONTH[month];

      const gasPriceMeanYear = gasPriceMean0 * (1 + input.gasEscalationPct / 100) ** year;
      const elecPriceMeanYear = input.electricityPrice * (1 + input.electricityEscalationPct / 100) ** year;
      const gasPriceVarYear = gasPriceStd0 ** 2 + (gasPriceMeanYear * 0.01 * Math.sqrt(year)) ** 2;
      const elecPriceVarYear = (input.electricityPrice * (input.electricityPriceVolatilityPct / 100)) ** 2 + (elecPriceMeanYear * 0.01 * Math.sqrt(year)) ** 2;

      let monthGasUsageMean = 0;
      let monthGasUsageVar = 0;
      let monthHpUsageMean = 0;
      let monthHpUsageVar = 0;
      let monthHeatMean = 0;
      let monthHeatVar = 0;

      for (let dayOffset = 0; dayOffset < monthDays; dayOffset += 1) {
        const dayIndex = monthStart + dayOffset;
        monthGasUsageMean += dayGasUsageMean[dayIndex];
        monthGasUsageVar += dayGasUsageVar[dayIndex];
        monthHpUsageMean += dayHpUsageMean[dayIndex];
        monthHpUsageVar += dayHpUsageVar[dayIndex];

        const muDemand = baseDemand * dayDemandShare[dayIndex];
        const sigmaDemand = sigmaYearDemand * dayDemandShare[dayIndex];
        monthHeatMean += muDemand + hotWaterDaily;
        monthHeatVar += sigmaDemand ** 2;
      }

      totalGasEnergyMean += monthGasUsageMean;
      totalGasEnergyVar += monthGasUsageVar;
      totalHpEnergyMean += monthHpUsageMean;
      totalHpEnergyVar += monthHpUsageVar;
      totalHeatDeliveredMean += monthHeatMean;
      totalHeatDeliveredVar += monthHeatVar;

      const monthGasEmissionsMean = monthGasUsageMean * input.gasCo2;
      const monthGasEmissionsVar = monthGasUsageVar * input.gasCo2 ** 2;
      const solarMonthlyMean = Math.max(0, input.solarAvailableKwh) * monthDays / TOTAL_DAYS;
      const solarMonthlyStd = solarMonthlyMean * (Math.max(0, input.solarAvailableStdPct) / 100);
      const monthSolarRatio = Math.max(0, Math.min(1, solarMonthlyMean / Math.max(monthHpUsageMean, 1e-9)));
      const monthGridRatio = 1 - monthSolarRatio;
      const monthSolarRatioStd = Math.max(0, Math.min(1, solarMonthlyStd / Math.max(monthHpUsageMean, 1e-9)));
      const monthHpEmissionsMean = monthHpUsageMean * input.gridMixCo2 * monthGridRatio;
      const monthHpEmissionsVar = monthHpUsageVar * (input.gridMixCo2 * monthGridRatio) ** 2;
      totalGasEmissionsMean += monthGasEmissionsMean;
      totalGasEmissionsVar += monthGasEmissionsVar;
      totalHpEmissionsMean += monthHpEmissionsMean;
      totalHpEmissionsVar += monthHpEmissionsVar;

      const monthGasCostMean =
        monthGasUsageMean * gasPriceMeanYear +
        monthGasEmissionsMean * input.carbonPrice +
        input.annualMaintenanceGas * monthDays / TOTAL_DAYS;
      const monthGasCostVar =
        monthGasUsageVar * gasPriceMeanYear ** 2 +
        monthGasUsageMean ** 2 * gasPriceVarYear +
        monthGasEmissionsVar * input.carbonPrice ** 2;

      const blendedMonthPriceMean = monthGridRatio * elecPriceMeanYear + monthSolarRatio * input.solarEffectivePrice;
      const blendedMonthPriceVar =
        (monthGridRatio ** 2) * elecPriceVarYear +
        (((elecPriceMeanYear - input.solarEffectivePrice) * monthSolarRatioStd) ** 2);
      const monthHpCostMean =
        monthHpUsageMean * blendedMonthPriceMean +
        monthHpEmissionsMean * input.carbonPrice +
        input.annualMaintenanceHp * monthDays / TOTAL_DAYS;
      const monthHpCostVar =
        monthHpUsageVar * blendedMonthPriceMean ** 2 +
        monthHpUsageMean ** 2 * blendedMonthPriceVar +
        monthHpEmissionsVar * input.carbonPrice ** 2;

      cumulativeGasMean += monthGasCostMean;
      cumulativeHpMean += monthHpCostMean;
      cumulativeGasVar += monthGasCostVar;
      cumulativeHpVar += monthHpCostVar;

      annualGasMean[year] += monthGasCostMean;
      annualGasVar[year] += monthGasCostVar;
      annualHpMean[year] += monthHpCostMean;
      annualHpVar[year] += monthHpCostVar;

      const diffQ = normalQuantiles(cumulativeGasMean - cumulativeHpMean, cumulativeHpVar + cumulativeGasVar);
      percentilesDiff.p10.push(diffQ.p10);
      percentilesDiff.p25.push(diffQ.p25);
      percentilesDiff.p50.push(diffQ.p50);
      percentilesDiff.p75.push(diffQ.p75);
      percentilesDiff.p90.push(diffQ.p90);

      const monthCount = monthIndexAll + 1;
      if (monthCount === snapshotMonths10) {
        snapshot10 = buildSnapshotFromMoments(
          cumulativeGasMean,
          cumulativeGasVar,
          cumulativeHpMean,
          cumulativeHpVar,
          totalGasEmissionsMean,
          totalGasEmissionsVar,
          totalHpEmissionsMean,
          totalHpEmissionsVar,
          totalGasEnergyMean,
          totalGasEnergyVar,
          totalHpEnergyMean,
          totalHpEnergyVar,
          investmentBase,
        );
        const diffStd10 = safeStd(cumulativeHpVar + cumulativeGasVar);
        const diffMean10 = cumulativeGasMean - cumulativeHpMean;
        cheaperProbability10 = diffStd10 > 0 ? normalCdf(diffMean10 / diffStd10) : diffMean10 > 0 ? 1 : 0;
      }

      if (monthCount === snapshotMonths15) {
        snapshot15 = buildSnapshotFromMoments(
          cumulativeGasMean,
          cumulativeGasVar,
          cumulativeHpMean,
          cumulativeHpVar,
          totalGasEmissionsMean,
          totalGasEmissionsVar,
          totalHpEmissionsMean,
          totalHpEmissionsVar,
          totalGasEnergyMean,
          totalGasEnergyVar,
          totalHpEnergyMean,
          totalHpEnergyVar,
          investmentBase,
        );
        const diffStd15 = safeStd(cumulativeHpVar + cumulativeGasVar);
        const diffMean15 = cumulativeGasMean - cumulativeHpMean;
        cheaperProbability15 = diffStd15 > 0 ? normalCdf(diffMean15 / diffStd15) : diffMean15 > 0 ? 1 : 0;
      }

      if (monthCount === snapshotMonths20) {
        snapshot20 = buildSnapshotFromMoments(
          cumulativeGasMean,
          cumulativeGasVar,
          cumulativeHpMean,
          cumulativeHpVar,
          totalGasEmissionsMean,
          totalGasEmissionsVar,
          totalHpEmissionsMean,
          totalHpEmissionsVar,
          totalGasEnergyMean,
          totalGasEnergyVar,
          totalHpEnergyMean,
          totalHpEnergyVar,
          investmentBase,
        );
        const diffStd20 = safeStd(cumulativeHpVar + cumulativeGasVar);
        const diffMean20 = cumulativeGasMean - cumulativeHpMean;
        cheaperProbability20 = diffStd20 > 0 ? normalCdf(diffMean20 / diffStd20) : diffMean20 > 0 ? 1 : 0;
      }
    }

    const hpTotals = normalQuantiles(cumulativeHpMean, cumulativeHpVar);
    const gasTotals = normalQuantiles(cumulativeGasMean, cumulativeGasVar);
    const diffMean = cumulativeGasMean - cumulativeHpMean;
    const diffStd = safeStd(cumulativeHpVar + cumulativeGasVar);
    const cheaperProbability = diffStd > 0 ? normalCdf(diffMean / diffStd) : diffMean > 0 ? 1 : 0;

    const emissionsGas = normalQuantiles(totalGasEmissionsMean, totalGasEmissionsVar);
    const emissionsHp = normalQuantiles(totalHpEmissionsMean, totalHpEmissionsVar);
    const energyGas = normalQuantiles(totalGasEnergyMean, totalGasEnergyVar);
    const energyHp = normalQuantiles(totalHpEnergyMean, totalHpEnergyVar);
    const annualGasSeries: PercentileSeries<number[]> = { p10: [], p25: [], p50: [], p75: [], p90: [] };
    const annualHpSeries: PercentileSeries<number[]> = { p10: [], p25: [], p50: [], p75: [], p90: [] };
    const annualDeltaSeries: PercentileSeries<number[]> = { p10: [], p25: [], p50: [], p75: [], p90: [] };
    for (let year = 0; year < horizonYears; year += 1) {
      const gasQ = normalQuantiles(annualGasMean[year], annualGasVar[year]);
      const hpQ = normalQuantiles(annualHpMean[year], annualHpVar[year]);
      const deltaQ = normalQuantiles(
        annualGasMean[year] - annualHpMean[year],
        annualHpVar[year] + annualGasVar[year]
      );
      annualGasSeries.p10.push(gasQ.p10);
      annualGasSeries.p25.push(gasQ.p25);
      annualGasSeries.p50.push(gasQ.p50);
      annualGasSeries.p75.push(gasQ.p75);
      annualGasSeries.p90.push(gasQ.p90);
      annualHpSeries.p10.push(hpQ.p10);
      annualHpSeries.p25.push(hpQ.p25);
      annualHpSeries.p50.push(hpQ.p50);
      annualHpSeries.p75.push(hpQ.p75);
      annualHpSeries.p90.push(hpQ.p90);
      annualDeltaSeries.p10.push(deltaQ.p10);
      annualDeltaSeries.p25.push(deltaQ.p25);
      annualDeltaSeries.p50.push(deltaQ.p50);
      annualDeltaSeries.p75.push(deltaQ.p75);
      annualDeltaSeries.p90.push(deltaQ.p90);
    }

    const scopMean = totalHeatDeliveredMean / Math.max(1e-9, totalHpEnergyMean);
    const scopVar =
      totalHeatDeliveredVar / Math.max(1e-9, totalHpEnergyMean ** 2) +
      (totalHeatDeliveredMean ** 2 * totalHpEnergyVar) / Math.max(1e-9, totalHpEnergyMean ** 4);
    const scop = normalQuantiles(scopMean, scopVar);

    const payback = {
      p25: paybackFromSeries(percentilesDiff.p25, (value) => value > 0),
      p50: paybackFromSeries(percentilesDiff.p50, (value) => value > 0),
      p75: paybackFromSeries(percentilesDiff.p75, (value) => value > 0),
    };

    if (horizonMonths < snapshotMonths10) {
      cheaperProbability10 = 0;
      snapshot10 = null;
    }
    if (horizonMonths < snapshotMonths15) {
      snapshot15 = null;
      cheaperProbability15 = null;
    }
    if (horizonMonths < snapshotMonths20) {
      snapshot20 = null;
      cheaperProbability20 = null;
    }

    return {
      percentiles: percentilesDiff,
      hpTotals,
      gasTotals,
      cheaperProbability,
      cheaperProbability10,
      cheaperProbabilityByHorizon: {
        y10: horizonMonths >= snapshotMonths10 ? cheaperProbability10 : null,
        y15: cheaperProbability15,
        y20: cheaperProbability20,
      },
      paybackYear: payback.p50,
      payback,
      emissionsGas,
      emissionsHp,
      energyGas,
      energyHp,
      scop,
      snapshots: {
        y10: snapshot10,
        y15: snapshot15,
        y20: snapshot20,
      },
      dailyGas: dailyGasCostsSeries,
      dailyHp: dailyHpCostsSeries,
      annual: {
        gas: annualGasSeries,
        hp: annualHpSeries,
        delta: annualDeltaSeries,
      },
    };
  }

  async function runMonteCarloFinal(
    province: NonNullable<ReturnType<typeof getProvince>>,
    horizonMonths: number,
  ): Promise<CostSummary> {
    const simCount = Math.round(input.simulations);
    const horizonYears = Math.ceil(horizonMonths / 12);
    const insulationMultiplier = getDemandMultiplier();
    const baseDemand = input.annualHeatDemand * insulationMultiplier;
    const demandStd = baseDemand * (input.demandStdPct / 100);
    const wFactor = getWindExposureFactor();
    const hotWaterDaily = input.hotWaterDemand / TOTAL_DAYS;
    const baseTemp = hddBaseTemp(input.insulationQuality);
    const gasPricePerKwh = input.gasPrice / GAS_KWH_PER_M3;
    const gasPriceStdPerKwh = (input.gasPrice * (input.gasPriceVolatilityPct / 100)) / GAS_KWH_PER_M3;

    const costSeriesGas: number[][] = Array.from({ length: simCount }, () => new Array(horizonMonths).fill(0));
    const costSeriesHp: number[][] = Array.from({ length: simCount }, () => new Array(horizonMonths).fill(0));
    const emissionsSeriesGas: number[] = [];
    const emissionsSeriesHp: number[] = [];
    const energySeriesGas: number[] = [];
    const energySeriesHp: number[] = [];
    const scopValues: number[] = [];
    const dailyGasCosts: number[][] = Array.from({ length: simCount }, () => new Array(TOTAL_DAYS).fill(0));
    const dailyHpCosts: number[][] = Array.from({ length: simCount }, () => new Array(TOTAL_DAYS).fill(0));
    const annualGasCosts: number[][] = Array.from({ length: simCount }, () => new Array(horizonYears).fill(0));
    const annualHpCosts: number[][] = Array.from({ length: simCount }, () => new Array(horizonYears).fill(0));
    const snapshotMonths10 = 10 * 12;
    const snapshotMonths15 = 15 * 12;
    const snapshotMonths20 = 20 * 12;
    const snapshot10Savings: number[] = [];
    const snapshot10GasTotals: number[] = [];
    const snapshot10HpTotals: number[] = [];
    const snapshot10EmissionsGas: number[] = [];
    const snapshot10EmissionsHp: number[] = [];
    const snapshot10EmissionSavings: number[] = [];
    const snapshot10EnergyGas: number[] = [];
    const snapshot10EnergyHp: number[] = [];
    const snapshot10Roi: number[] = [];
    const snapshot15Savings: number[] = [];
    const snapshot15GasTotals: number[] = [];
    const snapshot15HpTotals: number[] = [];
    const snapshot15EmissionsGas: number[] = [];
    const snapshot15EmissionsHp: number[] = [];
    const snapshot15EmissionSavings: number[] = [];
    const snapshot15EnergyGas: number[] = [];
    const snapshot15EnergyHp: number[] = [];
    const snapshot15Roi: number[] = [];
    const snapshot20Savings: number[] = [];
    const snapshot20GasTotals: number[] = [];
    const snapshot20HpTotals: number[] = [];
    const snapshot20EmissionsGas: number[] = [];
    const snapshot20EmissionsHp: number[] = [];
    const snapshot20EmissionSavings: number[] = [];
    const snapshot20EnergyGas: number[] = [];
    const snapshot20EnergyHp: number[] = [];
    const snapshot20Roi: number[] = [];

    for (let sim = 0; sim < simCount; sim += 1) {
      let gasPrice = Math.max(0.01, randomNormal(gasPricePerKwh, gasPriceStdPerKwh));
      let electricityPrice = Math.max(0.05, randomNormal(input.electricityPrice, input.electricityPrice * (input.electricityPriceVolatilityPct / 100)));
      let gasTotal = Math.max(0, randomNormal(input.boilerReplacement, input.boilerCapexStd));
      let hpTotal = Math.max(0, randomNormal(input.heatPumpCapex, input.capexStd) - Math.max(0, input.retrofitGrant));
      const investmentBaseSim = hpTotal - gasTotal;
      const boilerEff = Math.max(0.5, randomNormal(input.gasBoilerEfficiency, 0.02));
      let totalEmissionsGas = 0;
      let totalEmissionsHp = 0;
      let totalEnergyGas = 0;
      let totalEnergyHp = 0;
      let totalHeatDelivered = 0;
      let totalHpElectricity = 0;

      for (let year = 0; year < horizonYears; year += 1) {
        if (year > 0) {
          gasPrice *= 1 + randomNormal(input.gasEscalationPct / 100, 0.01);
          electricityPrice *= 1 + randomNormal(input.electricityEscalationPct / 100, 0.01);
        }
        const yearlySolarAvailable = Math.max(
          0,
          randomNormal(
            input.solarAvailableKwh,
            input.solarAvailableKwh * (Math.max(0, input.solarAvailableStdPct) / 100)
          )
        );
        const yearlySpaceHeat = Math.max(0, randomNormal(baseDemand, demandStd)) * wFactor;

        const monthAnomalies: number[] = [];
        for (let m = 0; m < 12; m += 1) {
          monthAnomalies.push(randomNormal(0, province.yearStd[m]));
        }

        const dailyTemps: number[] = [];
        const dailyHDDs: number[] = [];
        let totalHDD = 0;
        for (let dayOfYear = 0; dayOfYear < TOTAL_DAYS; dayOfYear += 1) {
          const monthIndex = DAY_TO_MONTH_INDEX[dayOfYear];
          const dayMean = province.dailyMeanByDay[dayOfYear] + monthAnomalies[monthIndex];
          const dayStd = province.dailyStdByDay[dayOfYear];
          const temperature = randomNormal(dayMean, dayStd);
          const hdd = Math.max(0, baseTemp - temperature);
          dailyTemps.push(temperature);
          dailyHDDs.push(hdd);
          totalHDD += hdd;
        }

        let dayIdx = 0;
        for (let m = 0; m < 12; m += 1) {
          const monthInHorizon = year * 12 + m;
          if (monthInHorizon >= horizonMonths) break;

          let monthSpaceHeatHp = 0;
          let monthSpaceHeatGas = 0;
          let monthHwHpElec = 0;
          let monthHwGas = 0;
          let monthHeatDelivered = 0;
          let monthGasEmissions = 0;
          let monthHpEmissions = 0;
          const monthSolarBudget = yearlySolarAvailable * DAYS_IN_MONTH[m] / TOTAL_DAYS;
          const daySolarBudget = yearlySolarAvailable / TOTAL_DAYS;

          const dailyMaintenanceGas = input.annualMaintenanceGas / TOTAL_DAYS;
          const dailyMaintenanceHp = input.annualMaintenanceHp / TOTAL_DAYS;

          for (let d = 0; d < DAYS_IN_MONTH[m]; d += 1) {
            const dayOfYear = dayIdx;
            let daySpaceHeatGas = 0;
            let daySpaceHeatHp = 0;
            if (totalHDD > 0 && dailyHDDs[dayIdx] > 0) {
              const dayDemand = yearlySpaceHeat * dailyHDDs[dayIdx] / totalHDD;
              const cop = copAtTemperature(
                input.heatPumpCop,
                input.supplyTemp,
                dailyTemps[dayIdx],
                input.minCop,
                input.backupThreshold
              );
              daySpaceHeatHp = dayDemand / cop;
              daySpaceHeatGas = dayDemand / boilerEff;
              monthSpaceHeatHp += daySpaceHeatHp;
              monthSpaceHeatGas += daySpaceHeatGas;
              monthHeatDelivered += dayDemand;
            }

            const hwCop = copAtTemperature(
              input.heatPumpCop,
              55,
              dailyTemps[dayIdx],
              input.minCop,
              input.backupThreshold
            );
            const hwHpElec = hotWaterDaily / hwCop;
            const hwGas = hotWaterDaily / boilerEff;
            monthHwHpElec += hwHpElec;
            monthHwGas += hwGas;

            const dayGasUsage = daySpaceHeatGas + hwGas;
            const dayHpUsage = daySpaceHeatHp + hwHpElec;
            const daySolarUsed = Math.min(dayHpUsage, daySolarBudget);
            const dayGridUsed = Math.max(0, dayHpUsage - daySolarUsed);
            const dayGasEmissions = dayGasUsage * input.gasCo2;
            const dayHpEmissions = dayGridUsed * input.gridMixCo2;
            monthGasEmissions += dayGasEmissions;
            monthHpEmissions += dayHpEmissions;

            if (year === 0) {
              const dayGasCost = dayGasUsage * gasPrice + dayGasEmissions * input.carbonPrice + dailyMaintenanceGas;
              const dayHpCost = dayGridUsed * electricityPrice + daySolarUsed * input.solarEffectivePrice + dayHpEmissions * input.carbonPrice + dailyMaintenanceHp;
              dailyGasCosts[sim][dayOfYear] = dayGasCost;
              dailyHpCosts[sim][dayOfYear] = dayHpCost;
            }

            dayIdx += 1;
          }

          const monthGasUsage = monthSpaceHeatGas + monthHwGas;
          const monthHpElec = monthSpaceHeatHp + monthHwHpElec;
          const monthSolarUsed = Math.min(monthHpElec, monthSolarBudget);
          const monthGridUsed = Math.max(0, monthHpElec - monthSolarUsed);

          const gasEnergyCost = monthGasUsage * gasPrice;
          const hpEnergyCost = monthGridUsed * electricityPrice + monthSolarUsed * input.solarEffectivePrice;

          const gasEmissions = monthGasEmissions;
          const hpEmissions = monthHpEmissions;

          const gasCarbonCost = gasEmissions * input.carbonPrice;
          const hpCarbonCost = hpEmissions * input.carbonPrice;

          const monthlyMaintenanceGas = input.annualMaintenanceGas * DAYS_IN_MONTH[m] / TOTAL_DAYS;
          const monthlyMaintenanceHp = input.annualMaintenanceHp * DAYS_IN_MONTH[m] / TOTAL_DAYS;

          const discountedGasCost = (gasEnergyCost + gasCarbonCost + monthlyMaintenanceGas);
          const discountedHpCost = (hpEnergyCost + hpCarbonCost + monthlyMaintenanceHp);

          gasTotal += discountedGasCost;
          hpTotal += discountedHpCost;
          annualGasCosts[sim][year] += discountedGasCost;
          annualHpCosts[sim][year] += discountedHpCost;

          costSeriesGas[sim][monthInHorizon] = gasTotal;
          costSeriesHp[sim][monthInHorizon] = hpTotal;

          totalEmissionsGas += gasEmissions;
          totalEmissionsHp += hpEmissions;
          totalEnergyGas += monthGasUsage;
          totalEnergyHp += monthHpElec;
          totalHeatDelivered += monthHeatDelivered + hotWaterDaily * DAYS_IN_MONTH[m];
          totalHpElectricity += monthSpaceHeatHp + monthHwHpElec;

          const monthCount = monthInHorizon + 1;
          if (monthCount === snapshotMonths10) {
            const savings = gasTotal - hpTotal;
            snapshot10Savings.push(savings);
            snapshot10GasTotals.push(gasTotal);
            snapshot10HpTotals.push(hpTotal);
            snapshot10EmissionsGas.push(totalEmissionsGas);
            snapshot10EmissionsHp.push(totalEmissionsHp);
            snapshot10EmissionSavings.push(totalEmissionsGas - totalEmissionsHp);
            snapshot10EnergyGas.push(totalEnergyGas);
            snapshot10EnergyHp.push(totalEnergyHp);
            snapshot10Roi.push(savings / safeRoiDenominator(investmentBaseSim));
          }
          if (monthCount === snapshotMonths15) {
            const savings = gasTotal - hpTotal;
            snapshot15Savings.push(savings);
            snapshot15GasTotals.push(gasTotal);
            snapshot15HpTotals.push(hpTotal);
            snapshot15EmissionsGas.push(totalEmissionsGas);
            snapshot15EmissionsHp.push(totalEmissionsHp);
            snapshot15EmissionSavings.push(totalEmissionsGas - totalEmissionsHp);
            snapshot15EnergyGas.push(totalEnergyGas);
            snapshot15EnergyHp.push(totalEnergyHp);
            snapshot15Roi.push(savings / safeRoiDenominator(investmentBaseSim));
          }
          if (monthCount === snapshotMonths20) {
            const savings = gasTotal - hpTotal;
            snapshot20Savings.push(savings);
            snapshot20GasTotals.push(gasTotal);
            snapshot20HpTotals.push(hpTotal);
            snapshot20EmissionsGas.push(totalEmissionsGas);
            snapshot20EmissionsHp.push(totalEmissionsHp);
            snapshot20EmissionSavings.push(totalEmissionsGas - totalEmissionsHp);
            snapshot20EnergyGas.push(totalEnergyGas);
            snapshot20EnergyHp.push(totalEnergyHp);
            snapshot20Roi.push(savings / safeRoiDenominator(investmentBaseSim));
          }
        }
      }

      emissionsSeriesGas.push(totalEmissionsGas);
      emissionsSeriesHp.push(totalEmissionsHp);
      energySeriesGas.push(totalEnergyGas);
      energySeriesHp.push(totalEnergyHp);
      scopValues.push(totalHpElectricity > 0 ? totalHeatDelivered / totalHpElectricity : 0);
    }

    const diffSeries: number[][] = Array.from({ length: simCount }, () => new Array(horizonMonths).fill(0));
    for (let month = 0; month < horizonMonths; month += 1) {
      for (let sim = 0; sim < simCount; sim += 1) {
        diffSeries[sim][month] = costSeriesGas[sim][month] - costSeriesHp[sim][month];
      }
    }

    const finalGas = costSeriesGas.map((series) => series[horizonMonths - 1]);
    const finalHp = costSeriesHp.map((series) => series[horizonMonths - 1]);
    const gasTotals = summarize(finalGas);
    const hpTotals = summarize(finalHp);
    const cheaperCount = finalHp.reduce((count, value, index) => (finalGas[index] > value ? count + 1 : count), 0);
    const emissionsGas = summarize(emissionsSeriesGas);
    const emissionsHp = summarize(emissionsSeriesHp);
    const energyGas = summarize(energySeriesGas);
    const energyHp = summarize(energySeriesHp);
    const scop = summarize(scopValues);
    const percentilesDiff: PercentileSeries<number[]> = {
      p10: [],
      p25: [],
      p50: [],
      p75: [],
      p90: [],
    };
    const diffColumn = new Array(simCount);
    for (let month = 0; month < horizonMonths; month += 1) {
      for (let sim = 0; sim < simCount; sim += 1) {
        diffColumn[sim] = diffSeries[sim][month];
      }
      diffColumn.sort((a, b) => a - b);
      percentilesDiff.p10.push(percentile(diffColumn, 0.1));
      percentilesDiff.p25.push(percentile(diffColumn, 0.25));
      percentilesDiff.p50.push(percentile(diffColumn, 0.5));
      percentilesDiff.p75.push(percentile(diffColumn, 0.75));
      percentilesDiff.p90.push(percentile(diffColumn, 0.9));
    }

    const paybackSamples: number[] = [];
    for (let sim = 0; sim < simCount; sim += 1) {
      const paybackIdx = diffSeries[sim].findIndex((value) => value > 0);
      if (paybackIdx !== -1) {
        paybackSamples.push(Number((Math.max(0, paybackIdx) / 12).toFixed(1)));
      }
    }
    paybackSamples.sort((a, b) => a - b);
    const payback = {
      p25: paybackSamples.length > 0 ? percentile(paybackSamples, 0.25) : null,
      p50: paybackSamples.length > 0 ? percentile(paybackSamples, 0.5) : null,
      p75: paybackSamples.length > 0 ? percentile(paybackSamples, 0.75) : null,
    };

    const cheaperWithin10YearsCount = horizonMonths >= snapshotMonths10
      ? diffSeries.reduce((count, row) => (row[snapshotMonths10 - 1] > 0 ? count + 1 : count), 0)
      : 0;
    const cheaperProbability10 = horizonMonths >= snapshotMonths10 ? cheaperWithin10YearsCount / simCount : 0;
    const cheaperWithin15YearsCount = horizonMonths >= snapshotMonths15
      ? diffSeries.reduce((count, row) => (row[snapshotMonths15 - 1] > 0 ? count + 1 : count), 0)
      : 0;
    const cheaperProbability15 = horizonMonths >= snapshotMonths15 ? cheaperWithin15YearsCount / simCount : null;
    const cheaperWithin20YearsCount = horizonMonths >= snapshotMonths20
      ? diffSeries.reduce((count, row) => (row[snapshotMonths20 - 1] > 0 ? count + 1 : count), 0)
      : 0;
    const cheaperProbability20 = horizonMonths >= snapshotMonths20 ? cheaperWithin20YearsCount / simCount : null;

    const snapshot10 = horizonMonths >= snapshotMonths10 && snapshot10Savings.length > 0
      ? {
          savings: summarize(snapshot10Savings),
          hpTotals: summarize(snapshot10HpTotals),
          gasTotals: summarize(snapshot10GasTotals),
          emissionsGas: summarize(snapshot10EmissionsGas),
          emissionsHp: summarize(snapshot10EmissionsHp),
          emissionsSavings: summarize(snapshot10EmissionSavings),
          energyGas: summarize(snapshot10EnergyGas),
          energyHp: summarize(snapshot10EnergyHp),
          roi: summarize(snapshot10Roi),
        }
      : null;

    const snapshot15 = horizonMonths >= snapshotMonths15 && snapshot15Savings.length > 0
      ? {
          savings: summarize(snapshot15Savings),
          hpTotals: summarize(snapshot15HpTotals),
          gasTotals: summarize(snapshot15GasTotals),
          emissionsGas: summarize(snapshot15EmissionsGas),
          emissionsHp: summarize(snapshot15EmissionsHp),
          emissionsSavings: summarize(snapshot15EmissionSavings),
          energyGas: summarize(snapshot15EnergyGas),
          energyHp: summarize(snapshot15EnergyHp),
          roi: summarize(snapshot15Roi),
        }
      : null;

    const snapshot20 = horizonMonths >= snapshotMonths20 && snapshot20Savings.length > 0
      ? {
          savings: summarize(snapshot20Savings),
          hpTotals: summarize(snapshot20HpTotals),
          gasTotals: summarize(snapshot20GasTotals),
          emissionsGas: summarize(snapshot20EmissionsGas),
          emissionsHp: summarize(snapshot20EmissionsHp),
          emissionsSavings: summarize(snapshot20EmissionSavings),
          energyGas: summarize(snapshot20EnergyGas),
          energyHp: summarize(snapshot20EnergyHp),
          roi: summarize(snapshot20Roi),
        }
      : null;

    const dailyGasP: PercentileSeries<number[]> = { p10: [], p25: [], p50: [], p75: [], p90: [] };
    const dailyHpP: PercentileSeries<number[]> = { p10: [], p25: [], p50: [], p75: [], p90: [] };
    for (let day = 0; day < TOTAL_DAYS; day += 1) {
      const colGas = dailyGasCosts.map((row) => row[day]);
      const colHp = dailyHpCosts.map((row) => row[day]);
      colGas.sort((a, b) => a - b);
      colHp.sort((a, b) => a - b);
      dailyGasP.p10.push(percentile(colGas, 0.1));
      dailyGasP.p25.push(percentile(colGas, 0.25));
      dailyGasP.p50.push(percentile(colGas, 0.5));
      dailyGasP.p75.push(percentile(colGas, 0.75));
      dailyGasP.p90.push(percentile(colGas, 0.9));
      dailyHpP.p10.push(percentile(colHp, 0.1));
      dailyHpP.p25.push(percentile(colHp, 0.25));
      dailyHpP.p50.push(percentile(colHp, 0.5));
      dailyHpP.p75.push(percentile(colHp, 0.75));
      dailyHpP.p90.push(percentile(colHp, 0.9));
    }

    const annualGasSeries: PercentileSeries<number[]> = { p10: [], p25: [], p50: [], p75: [], p90: [] };
    const annualHpSeries: PercentileSeries<number[]> = { p10: [], p25: [], p50: [], p75: [], p90: [] };
    const annualDeltaSeries: PercentileSeries<number[]> = { p10: [], p25: [], p50: [], p75: [], p90: [] };
    for (let year = 0; year < horizonYears; year += 1) {
      const colGas = annualGasCosts.map((row) => row[year]);
      const colHp = annualHpCosts.map((row) => row[year]);
      const colDelta = annualGasCosts.map((row, simIndex) => row[year] - annualHpCosts[simIndex][year]);
      colGas.sort((a, b) => a - b);
      colHp.sort((a, b) => a - b);
      colDelta.sort((a, b) => a - b);
      annualGasSeries.p10.push(percentile(colGas, 0.1));
      annualGasSeries.p25.push(percentile(colGas, 0.25));
      annualGasSeries.p50.push(percentile(colGas, 0.5));
      annualGasSeries.p75.push(percentile(colGas, 0.75));
      annualGasSeries.p90.push(percentile(colGas, 0.9));
      annualHpSeries.p10.push(percentile(colHp, 0.1));
      annualHpSeries.p25.push(percentile(colHp, 0.25));
      annualHpSeries.p50.push(percentile(colHp, 0.5));
      annualHpSeries.p75.push(percentile(colHp, 0.75));
      annualHpSeries.p90.push(percentile(colHp, 0.9));
      annualDeltaSeries.p10.push(percentile(colDelta, 0.1));
      annualDeltaSeries.p25.push(percentile(colDelta, 0.25));
      annualDeltaSeries.p50.push(percentile(colDelta, 0.5));
      annualDeltaSeries.p75.push(percentile(colDelta, 0.75));
      annualDeltaSeries.p90.push(percentile(colDelta, 0.9));
    }

    return {
      percentiles: percentilesDiff,
      hpTotals,
      gasTotals,
      cheaperProbability: cheaperCount / simCount,
      cheaperProbability10,
      cheaperProbabilityByHorizon: {
        y10: horizonMonths >= snapshotMonths10 ? cheaperProbability10 : null,
        y15: cheaperProbability15,
        y20: cheaperProbability20,
      },
      paybackYear: payback.p50,
      payback,
      emissionsGas,
      emissionsHp,
      energyGas,
      energyHp,
      scop,
      snapshots: {
        y10: snapshot10,
        y15: snapshot15,
        y20: snapshot20,
      },
      dailyGas: dailyGasP,
      dailyHp: dailyHpP,
      annual: {
        gas: annualGasSeries,
        hp: annualHpSeries,
        delta: annualDeltaSeries,
      },
    };
  }

  async function runSimulation() {
    errorMessage = '';
    const horizonMonths = Math.max(12, Math.round(input.horizonYears * 12));
    if (input.annualHeatDemand <= 0) {
      errorMessage = 'Please provide a positive heat demand.';
      return;
    }
    if (input.simulations < 200) {
      errorMessage = 'Please run at least 200 simulations for stability.';
      return;
    }
    const province = getProvince(input.province);
    if (!province) {
      errorMessage = 'Please select a valid province.';
      return;
    }

    running = true;
    runStatusMessage = `Monte-Carlo simulation with ${Math.round(input.simulations)} samples…`;

    await new Promise((resolve) => setTimeout(resolve, 0));
    const finalSummary = await runMonteCarloFinal(province, horizonMonths);
    results = finalSummary;
    resultStage = 'final';
    runStatusMessage = `${Math.round(input.simulations)} probabilistic runs completed`;
    running = false;
    await tick();
    drawChart(finalSummary);
    drawMonthlyChart(finalSummary);
  }

  function drawChart(summary: CostSummary) {
    if (!Plotly || !chartEl) return;
    ensurePlotlyLocaleRegistered();
    hideFirstChartTooltip();
    firstChartSummary = summary;
    const years = summary.percentiles.p50.map((_, idx) => idx / 12);
    const hoverRows = years.map((yearPoint, idx) => buildFirstChartHoverTable(summary, idx, yearPoint));
    const yRange = computeClippedYAxisRange(summary);
    const yTicks = buildYAxisTicksForRange(yRange[0], yRange[1], 8);
    defaultXAxisRange = [years[0], years[years.length - 1]];
    defaultYAxisRange = [...yRange];
    defaultYAxisTickValues = [...yTicks.values];
    defaultYAxisTickLabels = [...yTicks.labels];
    const initialXAxisRange: [number, number] = [defaultXAxisRange[0], defaultXAxisRange[1]];
    const initialYAxisRange: [number, number] = [defaultYAxisRange[0], defaultYAxisRange[1]];

    const traces = [
      {
        x: years,
        y: summary.percentiles.p50,
        name: 'Median Δ cost (gas - heat pump)',
        line: { color: '#1d4ed8', width: 2 },
        type: 'scatter',
        customdata: hoverRows,
        hoverinfo: 'none'
      },
      {
        x: [...years, ...years.slice().reverse()],
        y: [...summary.percentiles.p75, ...summary.percentiles.p25.slice().reverse()],
        fill: 'toself',
        fillcolor: 'rgba(59,130,246,0.15)',
        line: { width: 0 },
        name: '50% range',
        type: 'scatter',
        hoverinfo: 'skip'
      },
      {
        x: [...years, ...years.slice().reverse()],
        y: [...summary.percentiles.p90, ...summary.percentiles.p10.slice().reverse()],
        fill: 'toself',
        fillcolor: 'rgba(59,130,246,0.08)',
        line: { width: 0 },
        name: '80% range',
        type: 'scatter',
        hoverinfo: 'skip'
      }
    ];

    const layout = {
      title: {
        text: 'Cumulative cost difference (gas minus heat pump)',
        font: { size: 15, color: '#334155', family: 'Inter, system-ui, sans-serif' },
        y: 1.07,
        pad: { t: 0, b: 2 },
        yanchor: 'top'
      },
      xaxis: {
        title: { text: 'Years', font: { size: 12, color: '#64748b', family: 'Inter, system-ui, sans-serif' } },
        showgrid: false,
        linecolor: '#e2e8f0',
        tickfont: { family: "'JetBrains Mono', monospace", size: 11 },
        autorange: false,
        range: initialXAxisRange,
        showspikes: true,
        spikemode: 'across',
        spikesnap: 'cursor',
        spikedash: 'dot',
        spikethickness: 1,
        spikecolor: 'rgba(0,0,0,0.9)'
      },
      yaxis: {
        title: { text: 'Discounted cost savings (€)', font: { size: 12, color: '#64748b', family: 'Inter, system-ui, sans-serif' } },
        showgrid: true,
        gridwidth: 1,
        gridcolor: '#e2e8f0',
        linecolor: '#e2e8f0',
        tickfont: { family: "'JetBrains Mono', monospace", size: 11 },
        ticks: 'outside',
        ticklen: 5,
        tickwidth: 1,
        tickcolor: '#cbd5e1',
        autorange: false,
        range: initialYAxisRange,
        tickmode: 'array',
        tickvals: defaultYAxisTickValues,
        ticktext: defaultYAxisTickLabels
      },
      font: { family: 'Inter, system-ui, sans-serif', color: '#475569', size: 11 },
      hoverlabel: {
        font: { family: 'Inter, system-ui, sans-serif', size: 11 },
        bgcolor: 'rgba(255,255,255,0.82)',
        bordercolor: 'rgba(255,255,255,0)'
      },
      showlegend: true,
      legend: {
        orientation: 'h',
        yanchor: 'top',
        y: .99,
        xanchor: 'right',
        x: 1,
        font: { size: 11, family: 'Inter, system-ui, sans-serif' },
        bgcolor: 'rgba(255,255,255,0.7)',
        borderwidth: 0
      },
      plot_bgcolor: 'rgba(255,255,255,0.5)',
      paper_bgcolor: 'transparent',
      margin: { t: 30, l: 48, r: 0, b: 28 },
      hovermode: 'x',
      hoverdistance: -1,
      spikedistance: -1
    };

    const config = getPlotlyChartConfig((gd: any) => restoreDefaultAxes(gd));

    Promise.resolve(Plotly.react(chartEl, traces, layout, config))
      .then(() => {
        ensureRelayoutHandler();
      });
  }

  function formatYAxisValue(value: number): string {
    const abs = Math.abs(value);
    if (abs >= 1_000_000) {
      const millions = abs / 1_000_000;
      const shown = millions >= 10 ? Math.round(millions).toString() : millions.toFixed(1).replace(/\.0$/, '');
      return `${value < 0 ? '-' : ''}${shown} mln`;
    }
    if (abs >= 1_000) {
      const thousands = abs / 1_000;
      const shown = thousands >= 10 ? Math.round(thousands).toString() : thousands.toFixed(1).replace(/\.0$/, '');
      return `${value < 0 ? '-' : ''}${shown}k`;
    }
    return Math.round(value).toString();
  }

  function buildYAxisTicksForRange(minValue: number, maxValue: number, targetSteps = 8): { values: number[]; labels: string[] } {
    if (!Number.isFinite(minValue) || !Number.isFinite(maxValue) || maxValue <= minValue) {
      return { values: [0], labels: ['0'] };
    }

    const roughStep = (maxValue - minValue) / Math.max(1, targetSteps);
    const magnitude = 10 ** Math.floor(Math.log10(roughStep));
    const normalized = roughStep / magnitude;
    const niceFactor = normalized <= 1 ? 1 : normalized <= 2 ? 2 : normalized <= 5 ? 5 : 10;
    const step = niceFactor * magnitude;
    const minTick = Math.floor(minValue / step) * step;
    const maxTick = Math.ceil(maxValue / step) * step;

    const values: number[] = [];
    for (let current = minTick; current <= maxTick + step * 0.25; current += step) {
      const normalizedValue = Math.abs(current) < 1e-10 ? 0 : Number(current.toFixed(8));
      values.push(normalizedValue);
    }

    return {
      values,
      labels: values.map(formatYAxisValue)
    };
  }

  function restoreDefaultAxes(targetEl: any = chartEl) {
    if (!targetEl || !Plotly) return;
    if (!Number.isFinite(defaultXAxisRange[0]) || !Number.isFinite(defaultXAxisRange[1])) return;
    if (!Number.isFinite(defaultYAxisRange[0]) || !Number.isFinite(defaultYAxisRange[1])) return;

    const restoredX0 = Number(defaultXAxisRange[0]);
    const restoredX1 = Number(defaultXAxisRange[1]);
    const restoredY0 = Number(defaultYAxisRange[0]);
    const restoredY1 = Number(defaultYAxisRange[1]);

    applyingTickRelayout = true;
    Promise.resolve(
      Plotly.relayout(targetEl, {
        'xaxis.autorange': false,
        'xaxis.range[0]': restoredX0,
        'xaxis.range[1]': restoredX1,
        'yaxis.autorange': false,
        'yaxis.range[0]': restoredY0,
        'yaxis.range[1]': restoredY1,
        'yaxis.tickmode': 'array',
        'yaxis.tickvals': [...defaultYAxisTickValues],
        'yaxis.ticktext': [...defaultYAxisTickLabels]
      })
    ).finally(() => {
      applyingTickRelayout = false;
    });
  }

  function handleChartRelayout(eventData: Record<string, unknown>) {
    if (!chartEl || !Plotly || applyingTickRelayout) return;

    const resetRequested = eventData['xaxis.autorange'] === true || eventData['yaxis.autorange'] === true;
    if (resetRequested) {
      restoreDefaultAxes();
      return;
    }

    const yRangeStart = Number(eventData['yaxis.range[0]']);
    const yRangeEnd = Number(eventData['yaxis.range[1]']);
    if (!Number.isFinite(yRangeStart) || !Number.isFinite(yRangeEnd)) return;

    const minY = Math.min(yRangeStart, yRangeEnd);
    const maxY = Math.max(yRangeStart, yRangeEnd);
    if (maxY - minY <= 0) return;

    const refinedTicks = buildYAxisTicksForRange(minY, maxY, 12);

    applyingTickRelayout = true;
    Promise.resolve(
      Plotly.relayout(chartEl, {
        'yaxis.tickmode': 'array',
        'yaxis.tickvals': refinedTicks.values,
        'yaxis.ticktext': refinedTicks.labels
      })
    ).finally(() => {
      applyingTickRelayout = false;
    });
  }

  function ensureRelayoutHandler() {
    if (!chartEl || relayoutHandlerAttached) return;
    (chartEl as any).on('plotly_relayout', handleChartRelayout);
    (chartEl as any).on('plotly_hover', showFirstChartTooltip);
    (chartEl as any).on('plotly_unhover', hideFirstChartTooltip);
    (chartEl as any).on('plotly_doubleclick', () => {
      hideFirstChartTooltip();
      restoreDefaultAxes();
      return false;
    });
    relayoutHandlerAttached = true;
  }

  function ensureMonthlyHoverHandler() {
    if (!monthlyChartEl || monthlyHoverHandlerAttached) return;
    (monthlyChartEl as any).on('plotly_hover', showSecondChartTooltip);
    (monthlyChartEl as any).on('plotly_unhover', hideSecondChartTooltip);
    monthlyHoverHandlerAttached = true;
  }

  function computeClippedYAxisRange(summary: CostSummary): [number, number] {
    const p10 = summary.percentiles.p10.filter((value) => Number.isFinite(value));
    const p50 = summary.percentiles.p50.filter((value) => Number.isFinite(value));
    const p75Positive = summary.percentiles.p75
      .filter((value) => Number.isFinite(value))
      .map((value) => Math.max(0, value));
    const p90Positive = summary.percentiles.p90
      .filter((value) => Number.isFinite(value))
      .map((value) => Math.max(0, value));

    const rawMin = Math.min(...p10, ...p50, 0);
    const rawMax = Math.max(...p90Positive, ...p50, 0);

    if (p90Positive.length < 8 || !Number.isFinite(rawMin) || !Number.isFinite(rawMax)) {
      return [rawMin, rawMax];
    }

    const sortedP90 = [...p90Positive].sort((a, b) => a - b);
    const typicalUpper = percentile(sortedP90, 0.7);
    const centralUpper = Math.max(...p75Positive, 0);
    const clipCap = Math.max(typicalUpper * 1.1, centralUpper * 1.2, 0);
    const clippedMax = rawMax > clipCap * 1.25 ? clipCap : rawMax;

    const span = Math.max(clippedMax - rawMin, 1);
    return [rawMin - span * 0.05, clippedMax + span * 0.05];
  }

  function drawMonthlyChart(summary: CostSummary) {
    if (!Plotly || !monthlyChartEl) return;
    ensurePlotlyLocaleRegistered();
    hideSecondChartTooltip();
    secondChartSummary = summary;

    const days = Array.from({ length: TOTAL_DAYS }, (_, idx) => idx + 1);
    const dayIso = days.map((day) => new Date(Date.UTC(2021, 0, day)).toISOString().slice(0, 10));
    const monthTickVals: string[] = [];
    let dayCursor = 1;
    for (let monthIndex = 0; monthIndex < DAYS_IN_MONTH.length; monthIndex += 1) {
      const span = DAYS_IN_MONTH[monthIndex];
      const midDay = dayCursor + Math.floor((span - 1) / 2);
      monthTickVals.push(new Date(Date.UTC(2021, 0, midDay)).toISOString().slice(0, 10));
      dayCursor += span;
    }

    const deltaDaily = summary.dailyGas.p50.map((value, index) => value - summary.dailyHp.p50[index]);
    const visibleDailyValues = [
      ...summary.dailyGas.p25,
      ...summary.dailyGas.p75,
      ...summary.dailyGas.p50,
      ...summary.dailyHp.p25,
      ...summary.dailyHp.p75,
      ...summary.dailyHp.p50
    ].filter((value) => Number.isFinite(value));
    const visibleMin = visibleDailyValues.length > 0 ? Math.min(...visibleDailyValues) : 0;
    const visibleMax = visibleDailyValues.length > 0 ? Math.max(...visibleDailyValues) : 0;
    const hasVisibleNegative = visibleMin < 0;
    const nonNegativeUpper = Math.max(1, visibleMax * 1.05);

    const traces = [
      {
        x: [...dayIso, ...dayIso.slice().reverse()],
        y: [...summary.dailyGas.p75, ...summary.dailyGas.p25.slice().reverse()],
        fill: 'toself',
        fillcolor: 'rgba(239, 68, 68, 0.18)',
        line: { width: 0 },
        name: 'Gas P25–P75',
        type: 'scatter',
        hoverinfo: 'skip',
      },
      {
        x: [...dayIso, ...dayIso.slice().reverse()],
        y: [...summary.dailyHp.p75, ...summary.dailyHp.p25.slice().reverse()],
        fill: 'toself',
        fillcolor: 'rgba(59, 130, 246, 0.18)',
        line: { width: 0 },
        name: 'Heat pump P25–P75',
        type: 'scatter',
        hoverinfo: 'skip',
      },
      {
        x: dayIso,
        y: summary.dailyGas.p50,
        name: 'Gas median',
        line: { color: '#dc2626', width: 2 },
        type: 'scatter',
        hoverinfo: 'none',
      },
      {
        x: dayIso,
        y: summary.dailyHp.p50,
        name: 'Heat pump median',
        line: { color: '#2563eb', width: 2 },
        type: 'scatter',
        hoverinfo: 'none',
      },
      {
        x: dayIso,
        y: deltaDaily,
        name: 'Δ (Gas - HP)',
        line: { color: 'rgba(0,0,0,0)', width: 0 },
        mode: 'lines',
        showlegend: false,
        hoverinfo: 'none',
      },
    ];

    const layout = {
      title: {
        text: 'Daily cost profile (year 1) with uncertainty',
        font: { size: 15, color: '#334155', family: 'Inter, system-ui, sans-serif' },
        y: 1.07,
        pad: { t: 0, b: 2 },
        yanchor: 'top'
      },
      xaxis: {
        // title: { text: 'Month', font: { size: 12, color: '#64748b', family: 'Inter, system-ui, sans-serif' } },
        showgrid: false,
        linecolor: '#e2e8f0',
        tickfont: { family: "'JetBrains Mono', monospace", size: 11 },
        range: [dayIso[0], dayIso[TOTAL_DAYS - 1]],
        tickmode: 'array',
        tickvals: monthTickVals,
        ticktext: MONTH_NAMES,
        type: 'date',
        showspikes: true,
        spikemode: 'across',
        spikesnap: 'cursor',
        spikedash: 'dot',
        spikethickness: 1,
        spikecolor: 'rgba(0,0,0,0.9)'
      },
      yaxis: {
        title: { text: 'Daily cost (€)', font: { size: 12, color: '#64748b', family: 'Inter, system-ui, sans-serif' } },
        showgrid: true,
        gridwidth: 1,
        gridcolor: '#e2e8f0',
        linecolor: '#e2e8f0',
        tickfont: { family: "'JetBrains Mono', monospace", size: 11 },
        ticks: 'outside',
        ticklen: 5,
        tickwidth: 1,
        tickcolor: '#cbd5e1',
        ...(hasVisibleNegative
          ? {
              autorange: true,
              rangemode: 'tozero' as const
            }
          : {
              autorange: false,
              range: [0, nonNegativeUpper] as [number, number]
            })
      },
      font: { family: 'Inter, system-ui, sans-serif', color: '#475569', size: 11 },
      hoverlabel: {
        font: { family: 'Inter, system-ui, sans-serif', size: 11 },
        bgcolor: 'rgba(255,255,255,0.82)',
        bordercolor: 'rgba(255,255,255,0)'
      },
      showlegend: true,
      legend: {
        orientation: 'h',
        yanchor: 'top',
        y: .99,
        xanchor: 'right',
        x: 1,
        font: { size: 11, family: 'Inter, system-ui, sans-serif' },
        bgcolor: 'rgba(255,255,255,0.7)',
        borderwidth: 0
      },
      plot_bgcolor: 'rgba(255,255,255,0.5)',
      paper_bgcolor: 'transparent',
      margin: { t: 30, l: 48, r: 0, b: 28 },
      hovermode: 'x',
      hoverdistance: -1,
      spikedistance: -1,
    };

    const config = getPlotlyChartConfig((gd: any) => {
      Plotly.relayout(gd, {
        'xaxis.autorange': false,
        'xaxis.range[0]': dayIso[0],
        'xaxis.range[1]': dayIso[TOTAL_DAYS - 1],
        ...(hasVisibleNegative
          ? {
              'yaxis.autorange': true
            }
          : {
              'yaxis.autorange': false,
              'yaxis.range[0]': 0,
              'yaxis.range[1]': nonNegativeUpper
            })
      });
    });

    Promise.resolve(Plotly.react(monthlyChartEl, traces, layout, config)).then(() => {
      ensureMonthlyHoverHandler();
    });
  }
</script>

<style>
  .workspace {
    display: grid;
    grid-template-columns: minmax(300px, 400px) minmax(0, 1fr);
    gap: 1rem;
    align-items: start;
    font-family: 'Inter', system-ui, -apple-system, sans-serif;
    -webkit-font-smoothing: antialiased;
    -moz-osx-font-smoothing: grayscale;
  }

  @media (max-width: 1024px) {
    .workspace {
      grid-template-columns: 1fr;
    }
  }

  .left-panel,
  .right-panel {
    min-width: 0;
  }

  .left-panel {
    position: sticky;
    top: 0.5rem;
    max-height: calc(100vh - 1rem);
    overflow: auto;
    padding-right: 0.2rem;
    scrollbar-width: thin;
    scrollbar-color: #cbd5e1 transparent;
  }

  @media (max-width: 1024px) {
    .left-panel {
      position: static;
      max-height: none;
      overflow: visible;
      padding-right: 0;
    }
  }

  .page-header {
    margin: 0 0 0.45rem;
  }

  .page-header h2 {
    font-size: 1.45rem;
    font-weight: 700;
    margin: 0;
    margin-bottom: 0.1rem;
    letter-spacing: -0.03em;
    line-height: 1.2;
    background: linear-gradient(135deg, #0f766e 0%, #0ea5a4 45%, #22c55e 100%);
    -webkit-background-clip: text;
    -webkit-text-fill-color: transparent;
    background-clip: text;
  }

  .card {
    border: 1px solid rgba(0, 0, 0, 0.06);
    border-radius: 12px;
    padding: 0.55rem 0.7rem;
    background: rgba(255, 255, 255, 0.82);
    backdrop-filter: blur(16px);
    -webkit-backdrop-filter: blur(16px);
    box-shadow: 0 1px 2px rgba(0, 0, 0, 0.03), 0 3px 12px rgba(0, 0, 0, 0.04);
    margin-bottom: 0.45rem;
    transition: box-shadow 0.2s ease, border-color 0.2s ease;
  }

  .card:hover {
    box-shadow: 0 2px 4px rgba(0, 0, 0, 0.04), 0 6px 20px rgba(0, 0, 0, 0.06);
    border-color: rgba(0, 0, 0, 0.1);
  }

  .card h3 {
    font-size: 0.7rem;
    font-weight: 700;
    color: #64748b;
    margin: 0 0 0.3rem 0;
    letter-spacing: 0.06em;
    text-transform: uppercase;
  }

  .card h3.input-card-title {
    font-size: 0.72rem;
    font-weight: 700;
    color: #0f766e;
    letter-spacing: 0.015em;
    line-height: 1.3;
    text-transform: none;
  }

  .form-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(135px, 1fr));
    gap: 0.25rem 0.3rem;
  }

  .demand-mode-toggle {
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
    gap: 0.2rem;
    margin: 0.18rem 0 0.35rem;
  }

  .demand-mode-toggle button {
    padding: 0.26rem 0.35rem;
    min-height: auto;
    border-radius: 6px;
    border: 1px solid #cbd5e1;
    background: rgba(255, 255, 255, 0.9);
    color: #475569;
    box-shadow: none;
    font-size: 0.68rem;
    font-weight: 700;
    line-height: 1.15;
    letter-spacing: 0.01em;
  }

  .demand-mode-toggle button:hover:not(:disabled) {
    transform: none;
    box-shadow: none;
    filter: none;
    border-color: #94a3b8;
  }

  .demand-mode-toggle button.active {
    background: #e6fffb;
    border-color: #14b8a6;
    color: #0f766e;
  }

  label {
    font-size: 0.66rem;
    color: #64748b;
    display: flex;
    flex-direction: column;
    gap: 0.04rem;
    letter-spacing: 0.04em;
    text-transform: uppercase;
    font-weight: 600;
  }

  select {
    width: 100%;
    max-width: none;
    padding: 0.25rem 0.38rem;
    font-family: 'JetBrains Mono', ui-monospace, monospace;
    font-size: 0.78rem;
    line-height: 1.2;
    border-radius: 6px;
    border: 1px solid #e2e8f0;
    background: #fff;
    box-sizing: border-box;
    font-variant-numeric: tabular-nums;
    color: #1e293b;
    transition: border-color 0.15s ease, box-shadow 0.15s ease;
  }

  select:focus {
    outline: none;
    border-color: #818cf8;
    box-shadow: 0 0 0 2px rgba(129, 140, 248, 0.12);
  }

  .hint {
    font-size: 0.62rem;
    color: #94a3b8;
    margin-top: 0.04rem;
    line-height: 1.2;
    text-transform: none;
    font-weight: 500;
    letter-spacing: normal;
    font-style: italic;
  }

  button {
    padding: 0.38rem 0.8rem;
    background: linear-gradient(135deg, #2563eb, #1d4ed8);
    color: white;
    border: none;
    border-radius: 7px;
    cursor: pointer;
    font-family: 'Inter', system-ui, sans-serif;
    font-size: 0.78rem;
    font-weight: 600;
    line-height: 1.2;
    letter-spacing: 0.01em;
    transition: all 0.15s ease;
    box-shadow: 0 1px 3px rgba(29, 78, 216, 0.2);
  }

  button:hover:not(:disabled) {
    transform: translateY(-1px);
    box-shadow: 0 3px 10px rgba(29, 78, 216, 0.25);
    filter: brightness(1.05);
  }

  button:disabled {
    background: #94a3b8;
    cursor: not-allowed;
    box-shadow: none;
  }

  .status-banner {
    width: 100%;
    margin-bottom: 0.55rem;
  }

  .status-row {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 0.75rem;
    flex-wrap: wrap;
  }

  .status-controls {
    display: flex;
    align-items: end;
    gap: 0.45rem;
    flex-wrap: wrap;
  }

  .status-controls label {
    min-width: 160px;
  }

  .status-run-btn {
    min-height: 2.25rem;
    white-space: nowrap;
  }

  .error {
    color: #dc2626;
    font-size: 0.76rem;
    margin-top: 0.25rem;
    font-weight: 500;
    background: #fef2f2;
    padding: 0.28rem 0.45rem;
    border-radius: 5px;
    border: 1px solid #fecaca;
  }

  .note {
    font-size: 0.72rem;
    color: #94a3b8;
    margin: 0.12rem 0;
    line-height: 1.3;
  }

  .summary-grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(195px, 1fr));
    gap: 0.5rem;
    margin-top: 0;
  }

  .summary-grid .card {
    margin-bottom: 0;
    position: relative;
    overflow: hidden;
  }

  .summary-grid .card::before {
    content: '';
    position: absolute;
    top: 0;
    left: 0;
    right: 0;
    height: 1.5px;
    background: linear-gradient(90deg, #0f766e, #0ea5a4, #22c55e);
    opacity: 0.9;
    border-radius: 12px 12px 0 0;
  }

  .summary-grid .card strong {
    font-size: 0.72rem;
    font-weight: 700;
    color: #0f766e;
    letter-spacing: 0.015em;
    line-height: 1.3;
  }

  .card-title-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 0.35rem;
    margin-bottom: 0.08rem;
  }

  .horizon-toggle {
    display: inline-flex;
    gap: 0.18rem;
  }

  .horizon-toggle button {
    padding: 0.12rem 0.36rem;
    min-height: auto;
    border-radius: 5px;
    border: 1px solid #cbd5e1;
    background: rgba(255, 255, 255, 0.82);
    color: #475569;
    box-shadow: none;
    font-size: 0.62rem;
    font-weight: 600;
    line-height: 1.1;
  }

  .horizon-toggle button:hover:not(:disabled) {
    transform: none;
    box-shadow: none;
    filter: none;
    border-color: #94a3b8;
  }

  .horizon-toggle button.active {
    background: #e0e7ff;
    border-color: #818cf8;
    color: #3730a3;
  }

  .horizon-toggle button:disabled {
    opacity: 0.45;
    cursor: not-allowed;
    background: #f8fafc;
    color: #94a3b8;
  }

  .results-kpi {
    font-size: 1.15rem;
    line-height: 1.1;
    font-weight: 700;
    font-family: 'JetBrains Mono', ui-monospace, monospace;
    font-variant-numeric: tabular-nums;
    letter-spacing: -0.02em;
    margin: 0.15rem 0 0.08rem;
    color: #1e293b;
  }

  .mono-value {
    font-family: 'JetBrains Mono', ui-monospace, monospace;
    font-variant-numeric: tabular-nums;
  }

  .mono-align {
    white-space: pre;
  }

  .chart {
    height: 405px;
  }

  .chart-wrap {
    position: relative;
  }

  .custom-chart-tooltip {
    position: absolute;
    z-index: 6;
    pointer-events: none;
    background: rgba(255, 255, 255, 0.74);
    border: 1px solid #e2e8f0;
    border-radius: 0.5rem;
    box-shadow: 0 6px 20px rgba(15, 23, 42, 0.1);
    color: #1e293b;
    font-size: 0.78rem;
    line-height: 1.2;
    padding: 0.42rem 0.5rem;
    max-width: min(92%, 330px);
    white-space: normal;
  }

  .charts-grid {
    display: grid;
    grid-template-columns: 1fr;
    gap: 0.32rem;
    margin-top: 0.28rem;
  }

  @media (min-width: 1360px) {
    .charts-grid {
      grid-template-columns: repeat(2, minmax(0, 1fr));
    }
  }

  .chart-card {
    margin-top: 0;
    padding: 0.3rem 0.42rem;
    margin-bottom: 0;
  }

  .amount-positive {
    color: #16a34a;
  }

  .amount-negative {
    color: #dc2626;
  }

  .metric-row {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 0.35rem;
    margin: 0;
  }

  .metric-label {
    align-self: flex-start;
    line-height: 1.1;
    color: #0f766e;
    font-weight: 700;
  }

  .metric-stack {
    display: grid;
    row-gap: 0;
    margin: 0.12rem 0;
  }

  .note-spacer {
    visibility: hidden;
  }

  .metric-value {
    align-self: flex-start;
    margin: 0;
    text-align: right;
    font-size: 1.05rem;
  }

  .demand-summary {
    margin-top: 0.35rem;
    padding-top: 0.28rem;
    border-top: 1px solid #f1f5f9;
  }

  .demand-summary-grid {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 0.45rem;
  }

  .demand-summary-item {
    border: 1px solid #e2e8f0;
    border-radius: 8px;
    background: #ffffff;
    padding: 0.35rem 0.45rem;
  }

  .province-input-row {
    display: grid;
    grid-template-columns: minmax(0, 1fr) minmax(0, 1fr);
    gap: 0.35rem;
    margin-top: 0.18rem;
  }

  .province-field {
    grid-column: span 2;
  }

  .province-input-row select {
    min-width: 0;
  }

  .hint-inline {
    font-size: 0.62rem;
    color: #94a3b8;
    text-transform: none;
    letter-spacing: normal;
    font-style: italic;
    font-weight: 500;
    line-height: 1.2;
    text-align: left;
    border: 1px solid #e2e8f0;
    border-radius: 6px;
    background: #fff;
    padding: 0.25rem 0.38rem;
    display: flex;
    align-items: center;
    min-height: 2rem;
  }

  @media (max-width: 640px) {
    .province-field {
      grid-column: span 1;
    }

    .province-input-row {
      grid-template-columns: 1fr;
    }
  }

  .energy-assumptions-table-wrap {
    margin-bottom: 0.35rem;
    overflow-x: auto;
  }

  .energy-assumptions-table {
    width: 100%;
    table-layout: fixed;
    border-collapse: separate;
    border-spacing: 0;
    font-size: 0.72rem;
  }

  .energy-assumptions-table th,
  .energy-assumptions-table td {
    border-bottom: 1px solid #f1f5f9;
    padding: 0.24rem 0.18rem;
    vertical-align: middle;
    white-space: normal;
    overflow-wrap: anywhere;
    word-break: break-word;
  }

  .energy-assumptions-table th {
    font-size: 0.62rem;
    font-weight: 700;
    color: #94a3b8;
    letter-spacing: 0.05em;
    text-transform: uppercase;
    text-align: center;
  }

  .energy-assumptions-table td:first-child {
    font-weight: 600;
    color: #475569;
  }

  .energy-assumptions-table td :global(input) {
    max-width: none;
    width: 100%;
  }

  .system-assumptions-table-wrap {
    margin-bottom: 0.35rem;
    overflow-x: auto;
  }

  .system-assumptions-table {
    width: 100%;
    table-layout: fixed;
    border-collapse: separate;
    border-spacing: 0;
    font-size: 0.72rem;
  }

  .system-assumptions-table th,
  .system-assumptions-table td {
    border-bottom: 1px solid #f1f5f9;
    padding: 0.24rem 0.18rem;
    vertical-align: middle;
    white-space: normal;
    overflow-wrap: anywhere;
    word-break: break-word;
  }

  .system-assumptions-table th {
    font-size: 0.62rem;
    font-weight: 700;
    color: #94a3b8;
    letter-spacing: 0.05em;
    text-transform: uppercase;
    text-align: center;
  }

  .system-assumptions-table.capex-table td:first-child {
    font-weight: 600;
    color: #475569;
    width: 24%;
  }

  .system-assumptions-table.capex-table th:first-child {
    width: 24%;
  }

  .system-assumptions-table td :global(input) {
    max-width: none;
    width: 100%;
  }

  .efficiency-table thead tr:first-child th {
    text-align: center;
    font-size: 0.6rem;
    color: #64748b;
    letter-spacing: 0.06em;
  }

  .efficiency-table .boiler-separator {
    border-left: 1px solid #e2e8f0;
  }

  .solar-assumptions-table-wrap {
    margin-bottom: 0.35rem;
    overflow-x: auto;
  }

  .solar-assumptions-table {
    width: 100%;
    table-layout: fixed;
    border-collapse: separate;
    border-spacing: 0;
    font-size: 0.72rem;
  }

  .solar-assumptions-table th,
  .solar-assumptions-table td {
    border-bottom: 1px solid #f1f5f9;
    padding: 0.24rem 0.18rem;
    vertical-align: middle;
    white-space: normal;
    overflow-wrap: anywhere;
    word-break: break-word;
  }

  .solar-assumptions-table th {
    font-size: 0.62rem;
    font-weight: 700;
    color: #94a3b8;
    letter-spacing: 0.05em;
    text-transform: uppercase;
    text-align: center;
  }

  .solar-assumptions-table td:first-child {
    font-weight: 600;
    color: #475569;
  }

  .energy-assumptions-table th:first-child,
  .energy-assumptions-table td:first-child,
  .system-assumptions-table.capex-table th:first-child,
  .system-assumptions-table.capex-table td:first-child,
  .solar-assumptions-table th:first-child,
  .solar-assumptions-table td:first-child {
    width: 24%;
  }

  .solar-assumptions-table td :global(input) {
    max-width: none;
    width: 100%;
  }

</style>

<div class="page-header">
  <h2>Heat Pump vs Gas Planner (Netherlands)</h2>
  <p class="note">
    Probabilistic comparison with weather-driven COP. Daily temperatures are sampled per province (KNMI 1991-2020 normals)
    so cold spells naturally reduce heat-pump efficiency. Positive values mean the heat pump is cheaper (savings).
  </p>
</div>

<div class="workspace">
  <section class="left-panel">
    <div class="card">
      <h3 class="input-card-title">Home and Energy demand</h3>
      <div class="form-grid">
        <label class="province-field">
          <span>Province</span>
          <span class="province-input-row">
            <select bind:value={input.province}>
              {#each PROVINCES as prov}
                <option value={prov.key}>{prov.name}</option>
              {/each}
            </select>
            <span class="hint-inline">Used to determine the base temperature for heating degree days.</span>
          </span>
        </label>
        <label>
          Insulation quality
          <select bind:value={input.insulationQuality}>
            {#each insulationOptions as option}
              <option>{option.label}</option>
            {/each}
          </select>
        </label>
        <label>
          Radiators / supply temperature
          <select bind:value={input.supplyTemp}>
            {#each SUPPLY_TEMP_OPTIONS as opt}
              <option value={opt.value}>{opt.label}</option>
            {/each}
          </select>
        </label>
      </div>

      <div class="demand-mode-toggle" role="tablist" aria-label="Energy demand input mode">
        <button type="button" class:active={demandInputMode === 'estimate'} onclick={() => (demandInputMode = 'estimate')}>Estimate demand</button>
        <button type="button" class:active={demandInputMode === 'known-demand'} onclick={() => (demandInputMode = 'known-demand')}>I know demand (kWh)</button>
        <button type="button" class:active={demandInputMode === 'gas-consumption'} onclick={() => (demandInputMode = 'gas-consumption')}>Enter consumption (m³ of gas)</button>
      </div>

      <div class="form-grid">
        {#if demandInputMode === 'estimate'}
          <label>
            Dwelling type
            <select bind:value={input.dwellingType} onchange={handleDwellingChange}>
              <option>Apartment</option>
              <option>Row house</option>
              <option>Semi-detached</option>
              <option>Detached</option>
            </select>
          </label>
          <label>
            Floor area (m²)
            <FormattedNumberInput locale={outputNumberLocale} min="40" step="5" bind:value={input.floorArea} />
          </label>
          <label>
            Wind exposure
            <select bind:value={input.windExposure}>
              <option>Sheltered</option>
              <option>Normal</option>
              <option>Exposed</option>
            </select>
          </label>
          <label>
            Number of people
            <FormattedNumberInput locale={outputNumberLocale} min="1" max="8" step="1" bind:value={householdSize} />
          </label>
          <label>
            Estimated space heating (kWh/yr)
            <FormattedNumberInput locale={outputNumberLocale} value={estimatedSpaceDemand} readonly />
          </label>
          <label>
            Estimated hot water (kWh/yr)
            <FormattedNumberInput locale={outputNumberLocale} value={estimatedHotWaterDemand} readonly />
          </label>
        {:else if demandInputMode === 'known-demand'}
          <label>
            Space heating demand (kWh/yr)
            <FormattedNumberInput locale={outputNumberLocale} min="0" step="100" bind:value={knownSpaceHeatingDemand} />
          </label>
          <label>
            Hot water demand (kWh/yr)
            <FormattedNumberInput locale={outputNumberLocale} min="0" step="100" bind:value={knownHotWaterDemand} />
          </label>
        {:else}
          <label>
            Yearly gas consumption (m³/yr)
            <FormattedNumberInput locale={outputNumberLocale} min="0" step="50" bind:value={yearlyGasConsumptionM3} />
          </label>
          <label>
            Summer gas consumption (m³/month)
            <FormattedNumberInput locale={outputNumberLocale} min="0" step="5" bind:value={summerGasConsumptionM3PerMonth} />
          </label>
          <label>
            Inferred space heating (kWh/yr)
            <FormattedNumberInput locale={outputNumberLocale} value={inferredSpaceHeatingDemand} readonly />
          </label>
          <label>
            Inferred hot water (kWh/yr)
            <FormattedNumberInput locale={outputNumberLocale} value={inferredHotWaterDemand} readonly />
          </label>
        {/if}
      </div>
      <p class="hint">Space heating is weather-distributed by daily degree-days. In gas mode, summer usage estimates hot water share and the remainder is assigned to space heating.</p>

      <div class="form-grid">
        <label>
          Demand uncertainty due to behaviour (not weather related)
          <FormattedNumberInput locale={outputNumberLocale} min="0" max="50" step="1" bind:value={input.demandStdPct} isPercentage={true} />
        </label>
      </div>
      
      <div class="demand-summary">
        <div class="card-title-row">
          <strong>Output Yearly Demand</strong>
        </div>
        <div class="demand-summary-grid">
          <div class="demand-summary-item">
            <div class="note mono-value">Heating (kWh/yr)</div>
            <div class="results-kpi mono-value">{energyFormatter.format(input.annualHeatDemand)}</div>
          </div>
          <div class="demand-summary-item">
            <div class="note mono-value">Hot water (kWh/yr)</div>
            <div class="results-kpi mono-value">{energyFormatter.format(input.hotWaterDemand)}</div>
          </div>
        </div>
      </div>
    </div>

    <div class="card">
      <h3 class="input-card-title">Equipment Costs</h3>
      <div class="system-assumptions-table-wrap">
        <table class="system-assumptions-table capex-table mono-value">
          <thead>
            <tr>
              <th></th>
              <th>Installation (€)</th>
              <th>Subsidy (€)</th>
              <th>Uncertainty (€)</th>
              <th>Maintenance (€/yr)</th>
            </tr>
          </thead>
          <tbody>
            <tr>
              <td>Heat Pump</td>
              <td><FormattedNumberInput locale={outputNumberLocale} step="250" bind:value={input.heatPumpCapex} /></td>
              <td><FormattedNumberInput locale={outputNumberLocale} step="100" bind:value={input.retrofitGrant} /></td>
              <td><FormattedNumberInput locale={outputNumberLocale} step="100" bind:value={input.capexStd} /></td>
              <td><FormattedNumberInput locale={outputNumberLocale} step="10" bind:value={input.annualMaintenanceHp} /></td>
            </tr>
            <tr>
              <td>Boiler</td>
              <td><FormattedNumberInput locale={outputNumberLocale} step="100" bind:value={input.boilerReplacement} /></td>
              <td><FormattedNumberInput locale={outputNumberLocale} value={0} readonly /></td>
              <td><FormattedNumberInput locale={outputNumberLocale} step="50" bind:value={input.boilerCapexStd} /></td>
              <td><FormattedNumberInput locale={outputNumberLocale} step="10" bind:value={input.annualMaintenanceGas} /></td>
            </tr>
          </tbody>
        </table>
      </div>
    </div>

    <div class="card">
      <h3 class="input-card-title">Energy Price</h3>
      <div class="energy-assumptions-table-wrap">
        <table class="energy-assumptions-table mono-value">
          <thead>
            <tr>
              <th></th>
              <th>Price</th>
              <th>Annual increase (%)</th>
              <th>Volatility (%)</th>
            </tr>
          </thead>
          <tbody>
            <tr>
              <td>Gas (€/m³)</td>
              <td><FormattedNumberInput locale={outputNumberLocale} step="0.05" bind:value={input.gasPrice} /></td>
              <td><FormattedNumberInput locale={outputNumberLocale} step="0.5" bind:value={input.gasEscalationPct} isPercentage={true} /></td>
              <td><FormattedNumberInput locale={outputNumberLocale} step="1" bind:value={input.gasPriceVolatilityPct} isPercentage={true} /></td>
            </tr>
            <tr>
              <td>Electricity (€/kWh)</td>
              <td><FormattedNumberInput locale={outputNumberLocale} step="0.01" bind:value={input.electricityPrice} /></td>
              <td><FormattedNumberInput locale={outputNumberLocale} step="0.5" bind:value={input.electricityEscalationPct} isPercentage={true} /></td>
              <td><FormattedNumberInput locale={outputNumberLocale} step="1" bind:value={input.electricityPriceVolatilityPct} isPercentage={true} /></td>
            </tr>
          </tbody>
        </table>
      </div>
      <div class="solar-assumptions-table-wrap">
        <table class="solar-assumptions-table mono-value">
          <thead>
            <tr>
              <th></th>
              <th>Power (kWh/yr)</th>
              <th>Uncertainty (%)</th>
              <th>Eff. price (€/kWh)</th>
            </tr>
          </thead>
          <tbody>
            <tr>
              <td>Solar (Own)</td>
              <td><FormattedNumberInput locale={outputNumberLocale} step="100" min="0" bind:value={input.solarAvailableKwh} /></td>
              <td><FormattedNumberInput locale={outputNumberLocale} step="1" min="0" max="100" bind:value={input.solarAvailableStdPct} isPercentage={true} /></td>
              <td><FormattedNumberInput locale={outputNumberLocale} step="0.01" min="0" bind:value={input.solarEffectivePrice} /></td>
            </tr>
          </tbody>
        </table>
      </div>
    </div>

    <div class="card">
      <h3 class="input-card-title">Efficiency</h3>
      <div class="system-assumptions-table-wrap">
        <table class="system-assumptions-table efficiency-table mono-value">
          <thead>
            <tr>
              <th colspan="3">Heat Pump</th>
              <th class="boiler-separator">Boiler</th>
            </tr>
            <tr>
              <th>COP (A7/W35)</th>
              <th>Min COP</th>
              <th>Min temp (°C)</th>
              <th class="boiler-separator">Boiler efficiency</th>
            </tr>
          </thead>
          <tbody>
            <tr>
              <td><FormattedNumberInput locale={outputNumberLocale} step="0.1" bind:value={input.heatPumpCop} /></td>
              <td><FormattedNumberInput locale={outputNumberLocale} step="0.1" min="1" bind:value={input.minCop} /></td>
              <td><FormattedNumberInput locale={outputNumberLocale} step="1" bind:value={input.backupThreshold} /></td>
              <td class="boiler-separator"><FormattedNumberInput locale={outputNumberLocale} step="0.01" bind:value={input.gasBoilerEfficiency} /></td>
            </tr>
          </tbody>
        </table>
      </div>
    </div>

    <div class="card">
      <h3 class="input-card-title">CO₂</h3>
      <div class="system-assumptions-table-wrap">
        <table class="system-assumptions-table mono-value">
          <thead>
            <tr>
              <th>Grid (kg/kWh)</th>
              <th>Gas (kg/kWh)</th>
              <th>Carbon price (€/kg)</th>
            </tr>
          </thead>
          <tbody>
            <tr>
              <td><FormattedNumberInput locale={outputNumberLocale} step="0.01" bind:value={input.gridMixCo2} /></td>
              <td><FormattedNumberInput locale={outputNumberLocale} step="0.01" bind:value={input.gasCo2} /></td>
              <td><FormattedNumberInput locale={outputNumberLocale} step="0.01" bind:value={input.carbonPrice} /></td>
            </tr>
          </tbody>
        </table>
      </div>
    </div>

    <!-- <div class="card">
      <h3 class="input-card-title">Simulation</h3>
      <div class="form-grid">
        <label>
          Simulation horizon (years)
          <FormattedNumberInput locale={outputNumberLocale} min="5" step="1" bind:value={input.horizonYears} />
        </label>
      </div>
    </div> -->

    {#if errorMessage}
      <div class="error">{errorMessage}</div>
    {/if}
  </section>

  <section class="right-panel">
    <div class="card status-banner">
      <div class="status-row">
        <div>
          <strong>
            {#if running}
              Running full probabilistic sensitivity…
            {:else if resultStage === 'final'}
              Full probabilistic result is shown
            {:else}
              Approximate result (estimate) is shown
            {/if}
          </strong>
          <p class="note">
            {#if runStatusMessage}
              {runStatusMessage}
            {:else}
              Approximate result updates automatically whenever inputs change.
            {/if}
          </p>
        </div>
        <div class="status-controls">
          <label>
            Probabilistic runs
            <FormattedNumberInput locale={outputNumberLocale} min="200" step="100" bind:value={input.simulations} />
          </label>
          <button class="status-run-btn" onclick={runSimulation} disabled={running}>
            {#if running}
              Running…
            {:else}
              Run probabilistic full sensitivity
            {/if}
          </button>
        </div>
      </div>
    </div>

    {#if results}
      <div class="summary-grid">
        <div class="card">
          <div class="card-title-row">
            <strong>Total Costs</strong>
            <div class="horizon-toggle">
              <button type="button" class:active={selectedOutputHorizon === 10} onclick={() => (selectedOutputHorizon = 10)}>10 yr</button>
              <button type="button" class:active={selectedOutputHorizon === 15} onclick={() => (selectedOutputHorizon = 15)} disabled={!results.snapshots.y15}>15 yr</button>
              <button type="button" class:active={selectedOutputHorizon === 20} onclick={() => (selectedOutputHorizon = 20)} disabled={!results.snapshots.y20}>20 yr</button>
            </div>
          </div>
          <div class="results-kpi mono-value mono-align">
            Gas {alignedCostP50.gas}
          </div>
          <div class="note mono-value">
            P25/P75: {selectedSnapshot ? `${gasMoneyFormatter(selectedSnapshot.gasTotals.p25)} / ${gasMoneyFormatter(selectedSnapshot.gasTotals.p75)}` : 'N/A'}
          </div>
          <div class="results-kpi mono-value mono-align">
            HP  {alignedCostP50.hp}
          </div>
          <div class="note mono-value">
            P25/P75: {selectedSnapshot ? `${hpMoneyFormatter(selectedSnapshot.hpTotals.p25)} / ${hpMoneyFormatter(selectedSnapshot.hpTotals.p75)}` : 'N/A'}
          </div>
          <div class="results-kpi mono-value mono-align" class:amount-positive={(selectedSnapshot?.gasTotals.p50 ?? 0) - (selectedSnapshot?.hpTotals.p50 ?? 0) >= 0} class:amount-negative={(selectedSnapshot?.gasTotals.p50 ?? 0) - (selectedSnapshot?.hpTotals.p50 ?? 0) < 0}>
            Δ   {alignedCostP50.delta}
          </div>
          <div class="note mono-value">
            P25/P75: {selectedSnapshot ? `${gasMoneyFormatter(selectedSnapshot.gasTotals.p25 - selectedSnapshot.hpTotals.p25)} / ${gasMoneyFormatter(selectedSnapshot.gasTotals.p75 - selectedSnapshot.hpTotals.p75)}` : 'N/A'}
          </div>
        </div>
        
        <div class="card">
          <div class="card-title-row">
            <strong>Payback</strong>
            <div class="horizon-toggle">
              <button type="button" class:active={selectedOutputHorizon === 10} onclick={() => (selectedOutputHorizon = 10)}>10 yr</button>
              <button type="button" class:active={selectedOutputHorizon === 15} onclick={() => (selectedOutputHorizon = 15)} disabled={!results.snapshots.y15}>15 yr</button>
              <button type="button" class:active={selectedOutputHorizon === 20} onclick={() => (selectedOutputHorizon = 20)} disabled={!results.snapshots.y20}>20 yr</button>
            </div>
          </div>
          <div class="results-kpi mono-value" class:amount-positive={results.payback.p50 !== null} class:amount-negative={results.payback.p50 === null}>
            {formatNullableYearsWord(results.payback.p50)}
          </div>
          <div class="note mono-value">
            P25/P75: {formatNullableYear(results.payback.p25)} / {formatNullableYear(results.payback.p75)}
          </div>
          <div class="metric-stack">
            <div class="metric-row">
              <strong class="metric-label">Chance of Payout</strong>
              <div class="results-kpi mono-value metric-value" class:amount-positive={(selectedPayoutChance ?? 0) >= 0.5} class:amount-negative={(selectedPayoutChance ?? 0) < 0.5}>
                {selectedPayoutChance !== null ? percentFormatter.format(selectedPayoutChance) : 'N/A'}
              </div>
            </div>
            <div class="note mono-value note-spacer" aria-hidden="true">P25/P75: 0 / 0</div>
            <div class="metric-row">
              <strong class="metric-label">Return on Investment</strong>
              <div class="results-kpi mono-value metric-value" class:amount-positive={(selectedSnapshot?.roi.p50 ?? -1) >= 0} class:amount-negative={(selectedSnapshot?.roi.p50 ?? -1) < 0}>
                {selectedSnapshot ? percentFormatter.format(selectedSnapshot.roi.p50) : 'N/A'}
              </div>
            </div>
          </div>
          <div class="note mono-value">
            P25/P75: {selectedSnapshot ? `${percentFormatter.format(selectedSnapshot.roi.p25)} / ${percentFormatter.format(selectedSnapshot.roi.p75)}` : 'N/A'}
          </div>
          
        </div>
        <div class="card">
          <div class="card-title-row">
            <strong>CO₂ Savings</strong>
            <div class="horizon-toggle">
              <button type="button" class:active={selectedOutputHorizon === 10} onclick={() => (selectedOutputHorizon = 10)}>10 yr</button>
              <button type="button" class:active={selectedOutputHorizon === 15} onclick={() => (selectedOutputHorizon = 15)} disabled={!results.snapshots.y15}>15 yr</button>
              <button type="button" class:active={selectedOutputHorizon === 20} onclick={() => (selectedOutputHorizon = 20)} disabled={!results.snapshots.y20}>20 yr</button>
            </div>
          </div>
          <div class="results-kpi mono-value mono-align">
            Gas {alignedCo2P50.gas}
          </div>
          <div class="note mono-value">
            P25/P75: {selectedSnapshot ? `${co2FormatterCard(selectedSnapshot.emissionsGas.p25)} / ${co2FormatterCard(selectedSnapshot.emissionsGas.p75)}` : 'N/A'}
          </div>
          <div class="results-kpi mono-value mono-align">
            HP  {alignedCo2P50.hp}
          </div>
          <div class="note mono-value">
            P25/P75: {selectedSnapshot ? `${co2FormatterCard(selectedSnapshot.emissionsHp.p25)} / ${co2FormatterCard(selectedSnapshot.emissionsHp.p75)}` : 'N/A'}
          </div>
          <div class="results-kpi mono-value mono-align" class:amount-positive={(selectedSnapshot?.emissionsSavings.p50 ?? 0) >= 0} class:amount-negative={(selectedSnapshot?.emissionsSavings.p50 ?? 0) < 0}>
            Δ   {alignedCo2P50.delta}
          </div>
          <div class="note mono-value">
            P25/P75: {selectedSnapshot ? `${co2FormatterCard(selectedSnapshot.emissionsSavings.p25)} / ${co2FormatterCard(selectedSnapshot.emissionsSavings.p75)}` : 'N/A'}
          </div>
        </div>
        <div class="card">
          <div class="card-title-row">
            <strong>Energy Use</strong>
            <div class="horizon-toggle">
              <button type="button" class:active={selectedOutputHorizon === 10} onclick={() => (selectedOutputHorizon = 10)}>10 yr</button>
              <button type="button" class:active={selectedOutputHorizon === 15} onclick={() => (selectedOutputHorizon = 15)} disabled={!results.snapshots.y15}>15 yr</button>
              <button type="button" class:active={selectedOutputHorizon === 20} onclick={() => (selectedOutputHorizon = 20)} disabled={!results.snapshots.y20}>20 yr</button>
            </div>
          </div>
          <div class="results-kpi mono-value mono-align">
            Gas {alignedEnergyP50.gas}
          </div>
          <div class="note mono-value">
            P25/P75: {selectedSnapshot ? `${energyFormatterCard(selectedSnapshot.energyGas.p25)} / ${energyFormatterCard(selectedSnapshot.energyGas.p75)}` : 'N/A'}
          </div>
          <div class="results-kpi mono-value mono-align">
            HP  {alignedEnergyP50.hp}
          </div>
          <div class="note mono-value">
            P25/P75: {selectedSnapshot ? `${energyFormatterCard(selectedSnapshot.energyHp.p25)} / ${energyFormatterCard(selectedSnapshot.energyHp.p75)}` : 'N/A'}
          </div>
          <div class="results-kpi mono-value mono-align" class:amount-positive={((selectedSnapshot?.energyGas.p50 ?? 0) - (selectedSnapshot?.energyHp.p50 ?? 0)) >= 0} class:amount-negative={((selectedSnapshot?.energyGas.p50 ?? 0) - (selectedSnapshot?.energyHp.p50 ?? 0)) < 0}>
            Δ   {alignedEnergyP50.delta}
          </div>
          <div class="note mono-value">
            P25/P75: {selectedSnapshot ? `${energyFormatterCard(selectedSnapshot.energyGas.p25 - selectedSnapshot.energyHp.p25)} / ${energyFormatterCard(selectedSnapshot.energyGas.p75 - selectedSnapshot.energyHp.p75)}` : 'N/A'}
          </div>
        </div>
        <div class="card">
          <strong>Seasonal COP (SCOP)</strong>
          <div class="results-kpi mono-value">{results.scop.p50.toFixed(2)}</div>
          <div class="note mono-value">P25/P75: {results.scop.p25.toFixed(2)} / {results.scop.p75.toFixed(2)} — weather-weighted across all years.</div>
        </div>
      </div>
    {:else}
      <div class="card">
        <strong>Outputs</strong>
        <p class="note">Run comparison to populate cost, payback, emissions, and energy outputs.</p>
      </div>
    {/if}

    <div class="charts-grid">
      <div class="card chart-card">
        <div class="chart-wrap">
          <div class="chart" bind:this={chartEl}></div>
          {#if firstChartTooltipVisible}
            <div
              class="custom-chart-tooltip"
              bind:this={firstChartTooltipEl}
              style={`left:${firstChartTooltipLeft}px;top:${firstChartTooltipTop}px;`}
            >
              {@html firstChartTooltipHtml}
            </div>
          {/if}
        </div>
        <div class="note">Δ savings = gas total − heat pump total (discounted).</div>
      </div>

      <div class="card chart-card">
        <div class="chart-wrap">
          <div class="chart" bind:this={monthlyChartEl}></div>
          {#if secondChartTooltipVisible}
            <div
              class="custom-chart-tooltip"
              bind:this={secondChartTooltipEl}
              style={`left:${secondChartTooltipLeft}px;top:${secondChartTooltipTop}px;`}
            >
              {@html secondChartTooltipHtml}
            </div>
          {/if}
        </div>
        <div class="note">First-year daily cost profile incl. energy, carbon-price, and maintenance. Shaded areas show P25–P75 across simulations.</div>
      </div>
    </div>
  </section>
</div>
