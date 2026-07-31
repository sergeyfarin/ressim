import { describe, it, expect } from 'vitest';
import fs from 'fs';
import path from 'path';

const cardSrc = fs.readFileSync(
    path.join(__dirname, '..', 'ui', 'cards', 'ThreeDViewCard.svelte'),
    'utf8',
);
const chartSrc = fs.readFileSync(path.join(__dirname, 'SpatialProfileChart.svelte'), 'utf8');
const navStoreSrc = fs.readFileSync(
    path.join(__dirname, '..', 'stores', 'navigationStore.svelte.ts'),
    'utf8',
);
const chartsDir = path.join(__dirname, '..', 'charts');

describe('spatial profile wiring', () => {
    it('is mounted in the 3D view card, not among the run-results charts', () => {
        // A run-results chart shows one number per report step across a whole
        // run; this shows one number per cell at one instant. Keeping it beside
        // the 3D view is the point of the component.
        expect(/import\s+SpatialProfileChart\s+from/.test(cardSrc)).toBe(true);
        expect(/<SpatialProfileChart/.test(cardSrc)).toBe(true);
        expect(fs.existsSync(path.join(chartsDir, 'SwProfileChart.svelte'))).toBe(false);
    });

    it('reads the grid at the selected timestep, not the run-final grid', () => {
        // selectedOutput3D.gridState follows the 3D timestep selector;
        // selectedOutputProfile.gridState is the final/live snapshot. Using the
        // latter would silently freeze the profile at the end of the run.
        expect(/gridState=\{selectedOutput3D\.gridState\}/.test(cardSrc)).toBe(true);
        expect(/gridState=\{selectedOutputProfile\.gridState\}/.test(cardSrc)).toBe(false);
        expect(navStoreSrc).toMatch(/const selectedSnapshot = currentIndex >= 0 \? history\[currentIndex\] : null/);
        expect(navStoreSrc).toMatch(/gridState: selectedSnapshot\?\.grid/);
    });

    it('shares the 3D property selector rather than hardcoding a property', () => {
        expect(/property=\{showProperty\}/.test(cardSrc)).toBe(true);
    });

    it('passes scenario-derived spatial-reference metadata into the profile', () => {
        expect(cardSrc).toMatch(/reference=\{selectedOutputProfile\.spatialReference\}/);
        expect(navStoreSrc).toMatch(/spatialReference: sweepGeometry === 'areal' \|\| sweepGeometry === 'both'/);
        expect(chartSrc).toMatch(/buildSweepDiagonalOverlay/);
    });

    it('remounts local axis state when the spatial reference geometry changes', () => {
        expect(cardSrc).toMatch(/\{#key selectedOutputProfile\.spatialReference\?\.kind === "sweep"/);
        expect(cardSrc).toMatch(/`sweep-\$\{selectedOutputProfile\.spatialReference\.geometry\}`/);
    });

    it('takes its snapshot time from the replay position when one is selected', () => {
        expect(/simTime=\{selectedOutput3D\.replayTime \?\? selectedOutputProfile\.simTime\}/.test(cardSrc))
            .toBe(true);
    });

    it('passes both well coordinates so areal profiles can follow the displacement path', () => {
        expect(cardSrc).toMatch(/injectorI=\{selectedOutputProfile\.injectorI\}/);
        expect(cardSrc).toMatch(/injectorJ=\{selectedOutputProfile\.injectorJ\}/);
        expect(cardSrc).toMatch(/producerI=\{selectedOutputProfile\.producerI\}/);
        expect(cardSrc).toMatch(/producerJ=\{selectedOutputProfile\.producerJ\}/);
    });

    it('hides the chart legend and offers column averaging for layered grids', () => {
        expect(chartSrc).toMatch(/display:\s*false/);
        expect(chartSrc).toMatch(/Column average/);
    });

    it('delegates extraction and front construction to the pure model', () => {
        expect(/from\s+['"]\.\/spatialProfileModel['"]/.test(chartSrc)).toBe(true);
        expect(/buildSpatialProfile/.test(chartSrc)).toBe(true);
        expect(/buildFloodFrontOverlay/.test(chartSrc)).toBe(true);
    });

    it('does not reimplement fractional-flow physics in the component', () => {
        // The old SwProfileChart carried its own k_rw / k_ro / fractionalFlow and
        // Welge tangent search, free to drift from analytical/fractionalFlow.ts.
        expect(/function\s+k_rw/.test(chartSrc)).toBe(false);
        expect(/function\s+k_ro/.test(chartSrc)).toBe(false);
        expect(/function\s+fractionalFlow/.test(chartSrc)).toBe(false);
        expect(/computeShockSw/.test(chartSrc)).toBe(false);
    });

    it('uses the shared chart helpers instead of touching datasets directly', () => {
        expect(/from\s+['"]\.\.\/charts\/chart-helpers['"]/.test(chartSrc)).toBe(true);
        expect(/applyThemeToChart/.test(chartSrc)).toBe(true);
    });
});
