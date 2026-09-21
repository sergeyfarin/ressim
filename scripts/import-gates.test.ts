/**
 * The import gates must see across a workspace package boundary.
 *
 * `check-import-cycles.mjs` is a CI gate and `measure-module-coupling.mjs` is the split's
 * progress metric. Both resolve import specifiers themselves, and both originally understood
 * only relative paths. The moment a module under `src/lib/` became a workspace package and its
 * consumers switched to `@ressim/<pkg>/<file>`, every cross-package edge vanished from both:
 * the coupling metric improved on its own, and — far worse — a value-level cycle spanning two
 * packages passed the gate in silence.
 *
 * That is the failure these tests exist to prevent, so they assert the bad case detects rather
 * than the good case passing: a gate that cannot fail is the thing being guarded against.
 *
 * They run the real scripts as subprocesses against a throwaway tree, because the scripts walk
 * `src` relative to the working directory. That keeps them honest about what CI actually runs —
 * no exported internals, no reimplementation of the resolver in the test.
 */
import { describe, expect, it, beforeAll, afterAll } from 'vitest';
import { execFileSync } from 'node:child_process';
import { mkdtempSync, mkdirSync, writeFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const CYCLES = fileURLToPath(new URL('./check-import-cycles.mjs', import.meta.url));
const COUPLING = fileURLToPath(new URL('./measure-module-coupling.mjs', import.meta.url));

let fixture: string;

/** Two packages importing each other's *values* by workspace name — a cycle across a boundary. */
function writeFixture(root: string, specA: string, specB: string) {
    for (const pkg of ['alpha', 'beta']) {
        mkdirSync(path.join(root, 'src', 'lib', pkg), { recursive: true });
    }
    writeFileSync(
        path.join(root, 'src', 'lib', 'alpha', 'index.ts'),
        `import { fromBeta } from '${specA}';\nexport const fromAlpha = fromBeta + 1;\n`,
    );
    writeFileSync(
        path.join(root, 'src', 'lib', 'beta', 'index.ts'),
        `import { fromAlpha } from '${specB}';\nexport const fromBeta = fromAlpha + 1;\n`,
    );
}

function run(script: string, cwd: string) {
    try {
        return { status: 0, out: execFileSync('node', [script], { cwd, encoding: 'utf8' }) };
    } catch (error) {
        const e = error as { status?: number; stdout?: string; stderr?: string };
        return { status: e.status ?? -1, out: `${e.stdout ?? ''}${e.stderr ?? ''}` };
    }
}

beforeAll(() => {
    fixture = mkdtempSync(path.join(tmpdir(), 'ressim-import-gates-'));
});

afterAll(() => {
    rmSync(fixture, { recursive: true, force: true });
});

describe('import gates across a workspace package boundary', () => {
    it('check-import-cycles fails on a cycle written with @ressim/ specifiers', () => {
        const root = path.join(fixture, 'named');
        writeFixture(root, '@ressim/beta/index', '@ressim/alpha/index');

        const { status, out } = run(CYCLES, root);

        expect(status, `the gate passed a real cross-package cycle:\n${out}`).toBe(1);
        expect(out).toContain('runtime import cycle');
    });

    it('finds the same cycle whether it is written by name or by relative path', () => {
        const named = path.join(fixture, 'same-named');
        const relative = path.join(fixture, 'same-relative');
        writeFixture(named, '@ressim/beta/index', '@ressim/alpha/index');
        writeFixture(relative, '../beta/index', '../alpha/index');

        // The specifier style is a spelling choice; it must not change the verdict.
        expect(run(CYCLES, named).status).toBe(run(CYCLES, relative).status);
    });

    it('counts a cross-package value edge in the coupling metric', () => {
        const root = path.join(fixture, 'metric');
        writeFixture(root, '@ressim/beta/index', '@ressim/alpha/index');

        const { out } = run(COUPLING, root);

        // Two value edges, and they are mutual — the metric must not report a clean tree.
        expect(out).toMatch(/value edges\s+2\b/);
        expect(out).toContain('MUTUAL value pairs: 1');
        expect(out).not.toContain('every area could be lifted');
    });

    it('still ignores genuinely external packages', () => {
        const root = path.join(fixture, 'external');
        writeFixture(root, 'chart.js', 'three');

        const { status, out } = run(CYCLES, root);

        expect(status).toBe(0);
        expect(run(COUPLING, root).out).toMatch(/value edges\s+0\b/);
        expect(out).toContain('No runtime import cycles');
    });
});
