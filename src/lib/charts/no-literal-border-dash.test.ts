/**
 * Guards the line-style policy: every dash pattern in the app comes from
 * curveStylePolicy.ts, so the three tiers (ResSim solid / analytical dashed /
 * additional reference dotted) cannot drift apart one call site at a time.
 *
 * This is the check that was missing while `[3,3]`, `[2,3]`, `[2,4]`, `[4,4]`
 * and `[6,4]` accumulated across the chart builders.
 */
import { describe, it, expect } from 'vitest';
import fs from 'fs';
import path from 'path';

const SRC_ROOT = path.join(__dirname, '..');
const POLICY_FILE = path.join(__dirname, 'curveStylePolicy.ts');

/** Recursively collect .ts/.svelte sources under src/lib, excluding tests. */
function collectSources(dir: string): string[] {
    const out: string[] = [];
    for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
        const full = path.join(dir, entry.name);
        if (entry.isDirectory()) {
            out.push(...collectSources(full));
            continue;
        }
        if (!/\.(ts|svelte)$/.test(entry.name)) continue;
        if (/\.test\.ts$/.test(entry.name)) continue;
        out.push(full);
    }
    return out;
}

describe('curve style policy', () => {
    it('is the only module that writes a literal dash array', () => {
        const offenders: string[] = [];
        for (const file of collectSources(SRC_ROOT)) {
            if (file === POLICY_FILE) continue;
            const src = fs.readFileSync(file, 'utf8');
            // `borderDash: [ ... ]` — a pattern spelled out instead of imported.
            const matches = src.match(/borderDash\s*:\s*\[\s*\d/g);
            if (matches) offenders.push(`${path.relative(SRC_ROOT, file)} (${matches.length})`);
        }
        expect(offenders).toEqual([]);
    });

    it('gives ResSim, analytical, and external references three distinct patterns', async () => {
        const { ANALYTICAL_DASH, REFERENCE_DASH, applyCurveTypeStyle } =
            await import('./curveStylePolicy');

        expect(applyCurveTypeStyle('simulation').borderDash).toBeUndefined();
        expect(applyCurveTypeStyle('analytical').borderDash).toEqual(ANALYTICAL_DASH);
        // Both external-reference tiers are dotted; colour tells them apart.
        expect(applyCurveTypeStyle('reference').borderDash).toEqual(REFERENCE_DASH);
        expect(applyCurveTypeStyle('reference-simulation').borderDash).toEqual(REFERENCE_DASH);
        expect(REFERENCE_DASH).not.toEqual(ANALYTICAL_DASH);
    });
});
