import type { PvtRow } from '../simulator-types';

export const PSI_PER_BAR = 14.5037738;
export const SCF_PER_BBL_TO_M3_PER_M3 = 0.1781076; // 1 bbl = 0.1589873 m3. 1 scf = 0.0283168 m3. (0.0283168 / 0.1589873) = 0.178107
export const M3_PER_M3_TO_SCF_PER_BBL = 1.0 / SCF_PER_BBL_TO_M3_PER_M3;

/**
 * Convert Celsius to Fahrenheit
 */
export function cToF(c: number): number {
    return c * 1.8 + 32.0;
}

/**
 * Convert Celsius to Rankine
 */
export function cToR(c: number): number {
    return c * 1.8 + 491.67;
}

/**
 * Standing (1947) Bubble Point Pressure
 * @param rs Solution GOR in scf/STB
 * @param sgGas Gas specific gravity (air=1)
 * @param api Oil API gravity
 * @param tempF Temperature in Fahrenheit
 * @returns Bubble point pressure in psia
 */
export function standingBubblePoint(rs: number, sgGas: number, api: number, tempF: number): number {
    return 18.0 * Math.pow(rs / sgGas, 0.83) * Math.pow(10, 0.00091 * tempF - 0.0125 * api);
}

/**
 * Standing (1947) Solution Gas-Oil Ratio
 * @param p Pressure in psia
 * @param sgGas Gas specific gravity
 * @param api Oil API gravity
 * @param tempF Temperature in Fahrenheit
 * @returns Rs in scf/STB
 */
export function standingRs(p: number, sgGas: number, api: number, tempF: number): number {
    return sgGas * Math.pow(p / (18.0 * Math.pow(10, 0.0125 * api - 0.00091 * tempF)), 1.0 / 0.83);
}

/**
 * Standing (1947) Oil Formation Volume Factor
 * @param rs Solution GOR in scf/STB
 * @param sgGas Gas specific gravity
 * @param sgOil Oil specific gravity
 * @param tempF Temperature in Fahrenheit
 * @returns Bo in bbl/STB (equivalent to m3/sm3)
 */
export function standingBo(rs: number, sgGas: number, sgOil: number, tempF: number): number {
    const factor = rs * Math.pow(sgGas / sgOil, 0.5) + 1.25 * tempF;
    return 0.9759 + 0.000120 * Math.pow(factor, 1.2);
}

/**
 * Beggs-Robinson (1975) Dead Oil Viscosity
 * @param api Oil API
 * @param tempF Temp in Fahrenheit
 * @returns Dead oil viscosity in cP
 */
export function beggsRobinsonDeadOilViscosity(api: number, tempF: number): number {
    const z = 3.0324 - 0.02023 * api - 1.163 * Math.log10(tempF);
    const A = Math.pow(10, z);
    return Math.pow(10, A) - 1.0;
}

/**
 * Beggs-Robinson (1975) Saturated Oil Viscosity
 * @param muOd Dead oil viscosity (cP)
 * @param rs Solution GOR (scf/STB)
 * @returns Saturated oil viscosity (cP)
 */
export function beggsRobinsonSaturatedOilViscosity(muOd: number, rs: number): number {
    const A = 10.715 * Math.pow(rs + 100.0, -0.515);
    const B = 5.44 * Math.pow(rs + 150.0, -0.338);
    return A * Math.pow(muOd, B);
}

/**
 * Vasquez-Beggs Undersaturated Oil Viscosity
 * @param muOb Viscosity at bubble point (cP)
 * @param p Pressure (psia)
 * @param pb Bubble point pressure (psia)
 * @returns Undersaturated oil viscosity (cP)
 */
export function vasquezBeggsUndersaturatedViscosity(muOb: number, p: number, pb: number): number {
    const m = 2.6 * Math.pow(p, 1.187) * Math.exp(-11.513 - 8.98e-5 * p);
    return muOb * Math.pow(p / pb, m);
}

/**
 * Sutton (1985) Pseudo-critical Gas Properties
 */
export function suttonPseudoCriticals(sgGas: number): { Tpc: number; Ppc: number } {
    return {
        Tpc: 169.2 + 349.5 * sgGas - 74.0 * sgGas * sgGas, // Rankine
        Ppc: 756.8 - 131.0 * sgGas - 3.6 * sgGas * sgGas   // psia
    };
}

/**
 * Hall-Yarborough (1973) Z-factor (Newton Raphson)
 * @param p Pressure in psia
 * @param tempR Temperature in Rankine
 * @param Tpc Pseudo-critical temp in Rankine
 * @param Ppc Pseudo-critical pressure in psia
 * @returns Z-factor (dimensionless)
 */
export function hallYarboroughZFactor(p: number, tempR: number, Tpc: number, Ppc: number): number {
    const Ppr = p / Ppc;
    const Tpr = tempR / Tpc;
    const t = 1.0 / Tpr;
    
    // Explicit low pressure approximation to seed NR or catch edge cases
    if (Ppr < 0.05) return 1.0;

    const A = 0.06125 * t * Math.exp(-1.2 * Math.pow(1.0 - t, 2.0));
    const B = t * (14.76 - 9.76 * t + 4.58 * t * t);
    const C = t * (90.7 - 242.2 * t + 42.4 * t * t);
    const D = 2.18 + 2.82 * t;

    let y = 0.001; // Initial guess for reduced density
    let iter = 0;
    const maxIter = 50;
    const tol = 1e-6;

    while (iter < maxIter) {
        const y2 = y * y;
        const y3 = y2 * y;
        const y4 = y3 * y;
        const oneMinusY = 1.0 - y;
        const oneMinusY3 = Math.pow(oneMinusY, 3.0);
        const oneMinusY4 = Math.pow(oneMinusY, 4.0);

        const fy = (y + y2 + y3 - y4) / oneMinusY3 - A * Ppr - B * y2 + C * Math.pow(y, D);
        const dfy = (1.0 + 4.0 * y + 4.0 * y2 - 4.0 * y3 + y4) / oneMinusY4 - 2.0 * B * y + C * D * Math.pow(y, D - 1.0);

        const dy = fy / dfy;
        y = y - dy;

        if (Math.abs(dy) < tol) {
            break;
        }
        iter++;
    }

    if (y > 0) {
        return A * Ppr / y;
    }
    return 1.0; // Fallback
}

/**
 * Lee-Gonzalez-Eakin Gas Viscosity
 * @param p Pressure in psia
 * @param z Z-factor
 * @param tempR Temperature in Rankine
 * @param sgGas Gas specific gravity
 * @returns Gas viscosity in cP
 */
export function leeGonzalezEakinViscosity(p: number, z: number, tempR: number, sgGas: number): number {
    const Mg = 28.96 * sgGas;
    const rho_g = (p * Mg) / (z * 10.732 * tempR); // lb/ft3
    const rho_g_gcc = rho_g * 0.01601846; // convert to g/cc
    
    const K = (9.4 + 0.02 * Mg) * Math.pow(tempR, 1.5) / (209.0 + 19.0 * Mg + tempR) * 1e-4;
    const X = 3.5 + 986.0 / tempR + 0.01 * Mg;
    const Y = 2.4 - 0.2 * X;
    
    return K * Math.exp(X * Math.pow(rho_g_gcc, Y));
}

/**
 * Generate a complete 1D Black-Oil PVT table mapping pressure to fluid properties.
 * Sweeps pressure from slightly above 0 to p_max_bar.
 * 
 * @param api Oil API gravity
 * @param sgGas Gas specific gravity (air=1)
 * @param tempC Reservoir temperature (Celsius)
 * @param pbBar Bubble point pressure (bar)
 * @param pMaxBar Maximum table pressure (bar)
 * @param points Number of points in the table
 * @returns Array of PvtRow ordered by pressure ascending
 */
export function generateBlackOilTable(
    api: number, 
    sgGas: number, 
    tempC: number, 
    pbBar: number, 
    pMaxBar: number,
    points: number = 30
): PvtRow[] {
    const tempF = cToF(tempC);
    const tempR = cToR(tempC);
    const pbPsia = pbBar * PSI_PER_BAR;
    const sgOil = 141.5 / (131.5 + api);
    const { Tpc, Ppc } = suttonPseudoCriticals(sgGas);

    const rsMaxScf = standingRs(pbPsia, sgGas, api, tempF);
    const rsMaxM3M3 = rsMaxScf * SCF_PER_BBL_TO_M3_PER_M3;
    const muOd = beggsRobinsonDeadOilViscosity(api, tempF);
    const boPb = standingBo(rsMaxScf, sgGas, sgOil, tempF);
    const muOPb = beggsRobinsonSaturatedOilViscosity(muOd, rsMaxScf);

    const table: PvtRow[] = [];
    
    // Ensure we capture bubble point precisely by defining a grid that includes it
    const pressuresBar: number[] = [];
    pressuresBar.push(1.0); // Never 0 to avoid singularities
    
    const dp = pMaxBar / (points - 1);
    for (let i = 1; i < points; i++) {
        let p = i * dp;
        // if we are close to pb, replace this point with pb precisely
        if (Math.abs(p - pbBar) < dp * 0.5) p = pbBar;
        if (!pressuresBar.includes(p)) pressuresBar.push(p);
    }
    // Force pb in if missed
    if (!pressuresBar.includes(pbBar)) pressuresBar.push(pbBar);
    pressuresBar.sort((a, b) => a - b);

    for (const pBar of pressuresBar) {
        const pPsia = pBar * PSI_PER_BAR;
        
        let rs_scf = 0;
        let bo = 1.0;
        let mu_o = muOd;

        if (pBar <= pbBar) {
            // Saturated
            rs_scf = standingRs(pPsia, sgGas, api, tempF);
            bo = standingBo(rs_scf, sgGas, sgOil, tempF);
            mu_o = beggsRobinsonSaturatedOilViscosity(muOd, rs_scf);
        } else {
            // Undersaturated
            rs_scf = rsMaxScf;
            // Co = typically around 1e-5. Using simplistic Vasquez-Beggs interpolation for viscosity
            mu_o = vasquezBeggsUndersaturatedViscosity(muOPb, pPsia, pbPsia);
            // We use simple compressibility for Bo in Rust, but we precompute the trend here
            // Vasquez-Beggs c_o correlation (optional, or just use 1e-5)
            // c_o = (-1433 + 5 * R_sb + 17.2 * T - 1180 * sg_g + 12.61 * API) / (P * 10^5) -> approx
            const c_o_bar = 1e-5; // standard oil field default
            bo = boPb * Math.exp(-c_o_bar * (pBar - pbBar));
        }

        const z = hallYarboroughZFactor(pPsia, tempR, Tpc, Ppc);
        const mu_g = leeGonzalezEakinViscosity(pPsia, z, tempR, sgGas);
        
        // Bg in m3/sm3. Bg = (Psc / Tsc) * (z * T / P)
        // Standard conditions for gas: 14.7 psia, 60F (519.67 R)
        const Psc = 14.7;
        const Tsc = 519.67;
        const bg_m3m3 = (Psc / Tsc) * (z * tempR / pPsia);

        table.push({
            p_bar: pBar,
            rs_m3m3: rs_scf * SCF_PER_BBL_TO_M3_PER_M3,
            bo_m3m3: bo,
            mu_o_cp: mu_o,
            bg_m3m3: bg_m3m3,
            mu_g_cp: mu_g
        });
    }

    return table;
}
