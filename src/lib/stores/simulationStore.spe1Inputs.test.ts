import { describe, expect, it } from 'vitest';
import fs from 'fs';
import path from 'path';

const storePath = path.join(__dirname, 'parameterStore.svelte.ts');
const storeSource = fs.readFileSync(storePath, 'utf8');

describe('simulation store SPE1 exact-input wiring', () => {
    it('preserves scenario-provided PVT and SWOF/SGOF tables through the store pipeline', () => {
        expect(storeSource).toMatch(/pvtTableOverride = \$state<PvtRow\[\] \| undefined>\(undefined\);/);
        expect(storeSource).toMatch(/scalTables = \$state<ThreePhaseScalTables \| undefined>\(undefined\);/);
        expect(storeSource).toMatch(/this\.pvtTableOverride = clonePvtTable\(resolved\.pvtTable\);/);
        expect(storeSource).toMatch(/this\.scalTables = cloneScalTables\(resolved\.scalTables\);/);
        expect(storeSource).toMatch(/if \(this\.pvtTableOverride\?\.length\) \{/);
        expect(storeSource).toMatch(/pvtTable: this\.pvtTable,/);
        expect(storeSource).toMatch(/scalTables: this\.scalTables,/);
    });
});