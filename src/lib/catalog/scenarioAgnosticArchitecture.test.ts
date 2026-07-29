import { describe, expect, it } from 'vitest';
import fs from 'fs';
import path from 'path';
import { SCENARIOS } from './scenarios';

const sourceRoot = path.resolve(__dirname, '..', '..');

function listProductionFiles(dir: string): string[] {
    const files: string[] = [];
    for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
        const fullPath = path.join(dir, entry.name);
        if (entry.isDirectory()) {
            if (['pkg', 'target', 'node_modules'].includes(entry.name)) continue;
            files.push(...listProductionFiles(fullPath));
            continue;
        }
        if (/\.(ts|svelte)$/.test(entry.name) && !/\.test\.ts$/.test(entry.name)) {
            files.push(fullPath);
        }
    }
    return files;
}

function escapeRegex(value: string): string {
    return value.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
}

describe('scenario-agnostic frontend architecture', () => {
    it('keeps diagnostic-panel presentation declarative', () => {
        const chartSource = fs.readFileSync(
            path.join(sourceRoot, 'lib', 'charts', 'ReferenceComparisonChart.svelte'),
            'utf8',
        );
        expect(chartSource).not.toContain('isGasContext');
        expect(chartSource).toContain('...presentation.diagnostics');
    });

    it('recognizes live simulation curves when applying scenario-declared chart policies', () => {
        const chartSource = fs.readFileSync(
            path.join(sourceRoot, 'lib', 'charts', 'ReferenceComparisonChart.svelte'),
            'utf8',
        );
        expect(chartSource).toContain("curveKey?.endsWith('-sim')");
        expect(chartSource).toContain('suppressLeadingOutliers');
    });

    it('does not branch on canonical scenario keys outside scenario definitions', () => {
        const keyAlternation = SCENARIOS.map((scenario) => escapeRegex(scenario.key)).join('|');
        const keyComparison = new RegExp(
            `(?:scenarioKey|familyKey|caseKey|\\.key)\\s*(?:===|!==|==|!=)\\s*['\"](?:${keyAlternation})['\"]`,
            'g',
        );
        const reverseKeyComparison = new RegExp(
            `['\"](?:${keyAlternation})['\"]\\s*(?:===|!==|==|!=)\\s*(?:scenarioKey|familyKey|caseKey|[^\\n;]+\\.key)`,
            'g',
        );
        const violations: string[] = [];

        for (const file of listProductionFiles(sourceRoot)) {
            if (file.includes(`${path.sep}catalog${path.sep}scenarios${path.sep}`)) continue;
            const source = fs.readFileSync(file, 'utf8');
            if (keyComparison.test(source) || reverseKeyComparison.test(source)) {
                violations.push(path.relative(sourceRoot, file));
            }
            keyComparison.lastIndex = 0;
            reverseKeyComparison.lastIndex = 0;
        }

        expect(
            violations,
            `Move scenario-specific routing/presentation into the scenario definition: ${violations.join(', ')}`,
        ).toEqual([]);
    });

    it('keeps removed catalog experiments out of production source', () => {
        const removedKeys = ['wf_bl1d_opm', 'wf_tornado'];
        const violations: string[] = [];
        for (const file of listProductionFiles(sourceRoot)) {
            const source = fs.readFileSync(file, 'utf8');
            for (const key of removedKeys) {
                if (source.includes(key)) violations.push(`${path.relative(sourceRoot, file)}: ${key}`);
            }
        }
        expect(violations).toEqual([]);
    });
});
